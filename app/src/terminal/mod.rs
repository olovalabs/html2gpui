//! Integrated Terminal powered by `gpui-terminal` (Alacritty + PTY engine).
//!
//! Uses `gpui-terminal` and `portable-pty` for production-grade terminal
//! emulation with full TUI support (opencode, vim, htop, lazygit), 24-bit
//! RGB truecolor, smooth scrolling, selection, and automatic PTY resizing.

use std::path::Path;
use std::sync::{Arc, Mutex};

use gpui::{
    div, prelude::*, px, rgba, svg, Context, Edges, Entity, FocusHandle, FontWeight, IntoElement,
    Window,
};
use gpui_terminal::{ColorPalette, TerminalConfig, TerminalView};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::assets::MONO_FONT;
use crate::theme::Colors;
use crate::workspace::Workspace;

pub struct Terminal {
    pub view: Entity<TerminalView>,
    pub shell_name: String,
}

impl Terminal {
    pub fn new(root_dir: Option<&Path>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("Failed to open PTY");

        let (shell_cmd, shell_name) = if cfg!(windows) {
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
            shell_name: shell_name.to_string(),
        }
    }

    pub fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.view.read(cx).focus_handle().clone()
    }
}

pub fn render_terminal(
    terminal_entity: &Entity<Terminal>,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let term = terminal_entity.read(cx);
    let shell_name = term.shell_name.clone();
    let term_view = term.view.clone();

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(rgba(t.terminal_bg))
        .border_t_1()
        .border_color(rgba(t.border_variant))
        .child(
            // Terminal Toolbar
            div()
                .h(px(28.0))
                .w_full()
                .px(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .bg(rgba(t.toolbar))
                .border_b_1()
                .border_color(rgba(t.border_variant))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            svg()
                                .path("ui_icons/terminal_tint.svg")
                                .w(px(13.0))
                                .h(px(13.0))
                                .text_color(rgba(t.icon)),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgba(t.text))
                                .child(format!("TERMINAL ({shell_name})")),
                        ),
                )
                .child(
                    div()
                        .id("term-close-btn")
                        .w(px(20.0))
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(t.element_hover)))
                        .text_size(px(12.0))
                        .text_color(rgba(t.text_muted))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.show_terminal = false;
                            this.status = "Terminal hidden".into();
                            cx.notify();
                        })),
                ),
        )
        .child(
            // Embedded Terminal View from gpui-terminal
            div()
                .flex_1()
                .w_full()
                .min_h(px(0.0))
                .overflow_hidden()
                .child(term_view),
        )
}
