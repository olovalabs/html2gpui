//! Integrated Terminal powered by `gpui-terminal` (Alacritty + PTY engine).
//!
//! Uses `gpui-terminal` and `portable-pty` for production-grade terminal
//! emulation with full TUI support (opencode, vim, htop, lazygit), 24-bit
//! RGB truecolor, smooth scrolling, selection, and automatic PTY resizing.

use std::path::Path;
use std::sync::{Arc, Mutex};

use gpui::{
    div, prelude::*, px, rgba, svg, Context, Edges, Entity, FocusHandle, IntoElement, Window,
};
use gpui_terminal::{ColorPalette, TerminalConfig, TerminalView};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::assets::MONO_FONT;
use crate::theme::Colors;
use crate::workspace::Workspace;

pub struct Terminal {
    pub view: Entity<TerminalView>,
    /// Shell display name shown on the tab (e.g. "PowerShell 1").
    pub name: String,
}

impl Terminal {
    pub fn new(
        root_dir: Option<&Path>,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("Failed to open PTY");

        let (shell_cmd, _shell_name) = if cfg!(windows) {
            ("powershell.exe", "PowerShell")
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
            let name = if shell.ends_with("zsh") {
                "zsh"
            } else if shell.ends_with("fish") {
                "fish"
            } else {
                "bash"
            };
            (shell.leak() as &str, name)
        };

        let mut cmd = CommandBuilder::new(shell_cmd);
        if let Some(dir) = root_dir {
            cmd.cwd(dir);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let _child = pair.slave.spawn_command(cmd).expect("spawn shell failed");

        let writer = pair.master.take_writer().expect("take writer failed");
        let reader = pair.master.try_clone_reader().expect("clone reader failed");
        let pty_master = Arc::new(Mutex::new(pair.master));

        let config = TerminalConfig {
            font_family: MONO_FONT.into(),
            font_size: px(13.5),
            cols: 80,
            rows: 24,
            scrollback: 10000,
            line_height_multiplier: 1.2,
            padding: Edges::all(px(6.0)),
            colors: ColorPalette::default(),
        };

        let pty_for_resize = pty_master.clone();
        let resize_callback = move |cols: usize, rows: usize| {
            if let Ok(master) = pty_for_resize.lock() {
                let _ = master.resize(PtySize {
                    cols: cols as u16,
                    rows: rows as u16,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            }
        };

        let view = cx.new(|cx| {
            TerminalView::new(writer, reader, config, cx)
                .with_resize_callback(resize_callback)
        });

        view.read(cx).focus_handle().focus(window);

        Self {
            view,
            name,
        }
    }

    pub fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.view.read(cx).focus_handle().clone()
    }
}

pub fn render_terminal_panel(
    tabs: &[Entity<Terminal>],
    active: usize,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    // Guard: only render the body when at least one tab exists. The caller
    // hides the panel when the last tab is closed, so this is defensive.
    let active_view = tabs
        .get(active)
        .map(|term| term.read(cx).view.clone())
        .or_else(|| tabs.first().map(|term| term.read(cx).view.clone()));

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(rgba(t.terminal_bg))
        .border_t_1()
        .border_color(rgba(t.border_variant))
        // Terminal tab strip (VS Code style): one tab per shell, a [+] to add
        // a new terminal, and a chevron to collapse/hide the panel.
        .child(render_terminal_tab_bar(tabs, active, t, cx))
        // Embedded Terminal View from gpui-terminal (active tab only).
        .child(
            div()
                .flex_1()
                .w_full()
                .min_h(px(0.0))
                .overflow_hidden()
                .when_some(active_view, |d, view| d.child(view)),
        )
}

/// Terminal tab strip at the top of the panel.
fn render_terminal_tab_bar(
    tabs: &[Entity<Terminal>],
    active: usize,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let tabs_root = div()
        .id("terminal-tab-bar")
        .h(px(28.0))
        .w_full()
        .px(px(6.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .bg(rgba(t.toolbar))
        .border_b_1()
        .border_color(rgba(t.border_variant))
        .child(
            // Terminal icon + label anchoring the strip.
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .mr(px(4.0))
                .child(
                    svg()
                        .path("ui_icons/terminal_tint.svg")
                        .w(px(13.0))
                        .h(px(13.0))
                        .text_color(rgba(t.icon)),
                ),
        );

    let with_tabs = tabs_root.children(tabs.iter().enumerate().map(|(idx, _)| {
        let is_active = idx == active;
        let name = tabs[idx].read(cx).name.clone();
        render_terminal_tab(name, idx, is_active, t, cx)
    }));

    with_tabs
        .child(div().flex_1())
        .child(render_new_terminal_button(t, cx))
        .child(render_hide_panel_button(t, cx))
}

/// A single terminal tab with its own close (×) button. Clicking the tab
/// activates it; clicking × kills that shell (VS Code).
fn render_terminal_tab(
    name: String,
    index: usize,
    is_active: bool,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let bg = if is_active { t.tab_active_bg } else { t.tab_inactive_bg };
    let fg = if is_active { t.tab_active_fg } else { t.tab_inactive_fg };

    div()
        .id(("terminal-tab", index))
        .h(px(20.0))
        .pl(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .rounded(px(4.0))
        .bg(rgba(bg))
        .cursor_pointer()
        .text_size(px(11.5))
        .text_color(rgba(fg))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.activate_terminal(index, window, cx);
        }))
        .hover(|h| if is_active { h } else { h.bg(rgba(t.element_hover)) })
        .child(div().child(name))
        .child(
            div()
                .id(("terminal-tab-close", index))
                .w(px(16.0))
                .h(px(16.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.0))
                .cursor_pointer()
                .opacity(0.6)
                .hover(|h| h.opacity(1.0).bg(rgba(t.element_hover)))
                .child(div().text_size(px(11.0)).text_color(rgba(fg)).child("×"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.close_terminal(index, window, cx);
                })),
        )
}

/// "+" button that spawns a new terminal tab.
fn render_new_terminal_button(t: &Colors, cx: &mut Context<Workspace>) -> impl IntoElement {
    div()
        .id("term-new-btn")
        .w(px(22.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.element_hover)))
        .text_size(px(15.0))
        .text_color(rgba(t.text_muted))
        .child("+")
        .on_click(cx.listener(|this, _, window, cx| {
            this.new_terminal(window, cx);
        }))
}

/// Chevron button that hides the panel while keeping all shell sessions alive
/// (equivalent to the Ctrl+` toggle, but mouse driven).
fn render_hide_panel_button(t: &Colors, cx: &mut Context<Workspace>) -> impl IntoElement {
    div()
        .id("term-hide-btn")
        .w(px(22.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.element_hover)))
        .child(
            svg()
                .path("ui_icons/chevron-down_tint.svg")
                .w(px(14.0))
                .h(px(14.0))
                .text_color(rgba(t.text_muted)),
        )
        .on_click(cx.listener(|this, _, window, cx| {
            this.hide_terminal(window, cx);
        }))
}
