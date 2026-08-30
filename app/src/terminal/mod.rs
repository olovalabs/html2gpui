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
use gpui_terminal::{TerminalConfig, TerminalView};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::assets::MONO_FONT;
use crate::theme::Colors;
use crate::workspace::Workspace;

pub struct Terminal {
    pub view: Entity<TerminalView>,
    /// Shell display name shown on the tab (e.g. "PowerShell 1").
    pub name: String,
}



struct SharedWriter(Arc<Mutex<Box<dyn std::io::Write + Send>>>);

impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self
            .0
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "lock poisoned"))?;
        guard.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "lock poisoned"))?;
        guard.flush()
    }
}

#[cfg(windows)]
fn is_capslock_on() -> bool {
    extern "system" {
        fn GetKeyState(nVirtKey: i32) -> i16;
    }
    const VK_CAPITAL: i32 = 0x14;
    unsafe { (GetKeyState(VK_CAPITAL) & 1) != 0 }
}

#[cfg(not(windows))]
fn is_capslock_on() -> bool {
    false
}

/// Convert a GPUI keystroke to terminal escape sequence bytes with complete support
/// for Shift and Caps Lock capitalization, shifted symbols, and control keys.
pub fn terminal_keystroke_to_bytes(keystroke: &gpui::Keystroke) -> Option<Vec<u8>> {
    // 1. Special and navigation keys
    match keystroke.key.as_str() {
        "space" => {
            if keystroke.modifiers.control {
                return Some(b"\x00".to_vec());
            }
            return Some(b" ".to_vec());
        }
        "enter" => return Some(b"\r".to_vec()),
        "escape" => return Some(b"\x1b".to_vec()),
        "backspace" => return Some(b"\x7f".to_vec()),
        "tab" => {
            if keystroke.modifiers.shift {
                return Some(b"\x1b[Z".to_vec());
            }
            return Some(b"\t".to_vec());
        }
        "up" => return Some(b"\x1b[A".to_vec()),
        "down" => return Some(b"\x1b[B".to_vec()),
        "right" => return Some(b"\x1b[C".to_vec()),
        "left" => return Some(b"\x1b[D".to_vec()),
        "home" => return Some(b"\x1b[H".to_vec()),
        "end" => return Some(b"\x1b[F".to_vec()),
        "pageup" => return Some(b"\x1b[5~".to_vec()),
        "pagedown" => return Some(b"\x1b[6~".to_vec()),
        "insert" => return Some(b"\x1b[2~".to_vec()),
        "delete" => return Some(b"\x1b[3~".to_vec()),
        "f1" => return Some(b"\x1bOP".to_vec()),
        "f2" => return Some(b"\x1bOQ".to_vec()),
        "f3" => return Some(b"\x1bOR".to_vec()),
        "f4" => return Some(b"\x1bOS".to_vec()),
        "f5" => return Some(b"\x1b[15~".to_vec()),
        "f6" => return Some(b"\x1b[17~".to_vec()),
        "f7" => return Some(b"\x1b[18~".to_vec()),
        "f8" => return Some(b"\x1b[19~".to_vec()),
        "f9" => return Some(b"\x1b[20~".to_vec()),
        "f10" => return Some(b"\x1b[21~".to_vec()),
        "f11" => return Some(b"\x1b[23~".to_vec()),
        "f12" => return Some(b"\x1b[24~".to_vec()),
        _ => {}
    }

    // 2. Control combinations
    if keystroke.modifiers.control {
        let key = keystroke.key.as_str();
        if key.len() == 1 {
            let ch = key.chars().next().unwrap();
            if ch.is_ascii_alphabetic() {
                let upper = ch.to_ascii_uppercase();
                let ctrl_char = (upper as u8) - b'@';
                return Some(vec![ctrl_char]);
            }
            match ch {
                '[' => return Some(b"\x1b".to_vec()),
                '\\' => return Some(b"\x1c".to_vec()),
                ']' => return Some(b"\x1d".to_vec()),
                '^' => return Some(b"\x1e".to_vec()),
                '_' => return Some(b"\x1f".to_vec()),
                '?' => return Some(b"\x7f".to_vec()),
                _ => {}
            }
        }
    }

    // 3. Alt combinations
    if keystroke.modifiers.alt {
        let key = keystroke.key.as_str();
        if key.len() == 1 {
            let ch = key.chars().next().unwrap();
            if ch.is_ascii() {
                return Some(vec![b'\x1b', ch as u8]);
            }
        }
    }

    // 4. Regular printable characters: handle Shift, CapsLock, symbols, letters
    let is_caps = is_capslock_on();
    // Shift XOR CapsLock determines if letter should be capitalized
    let should_uppercase = keystroke.modifiers.shift ^ is_caps;

    // Check if key is a single ASCII letter
    let key = keystroke.key.as_str();
    if key.len() == 1 {
        let ch = key.chars().next().unwrap();
        if ch.is_ascii_alphabetic() {
            let out_char = if should_uppercase {
                ch.to_ascii_uppercase()
            } else {
                ch.to_ascii_lowercase()
            };
            return Some(vec![out_char as u8]);
        }
    }

    // If key_char is available (from IME or platform event)
    if !keystroke.modifiers.control && !keystroke.modifiers.alt {
        if let Some(key_char) = &keystroke.key_char {
            if key_char.len() == 1 {
                let ch = key_char.chars().next().unwrap();
                if ch.is_ascii_alphabetic() {
                    let out_char = if should_uppercase {
                        ch.to_ascii_uppercase()
                    } else {
                        ch.to_ascii_lowercase()
                    };
                    return Some(vec![out_char as u8]);
                }
            }
            return Some(key_char.as_bytes().to_vec());
        }
    }

    // Fallback for shifted US keyboard layout symbols
    if key.len() == 1 {
        let ch = key.chars().next().unwrap();
        if keystroke.modifiers.shift {
            let shifted = match ch {
                '1' => '!',
                '2' => '@',
                '3' => '#',
                '4' => '$',
                '5' => '%',
                '6' => '^',
                '7' => '&',
                '8' => '*',
                '9' => '(',
                '0' => ')',
                '-' => '_',
                '=' => '+',
                '[' => '{',
                ']' => '}',
                '\\' => '|',
                ';' => ':',
                '\'' => '"',
                ',' => '<',
                '.' => '>',
                '/' => '?',
                '`' => '~',
                other => other,
            };
            return Some(vec![shifted as u8]);
        }
        if ch.is_ascii() {
            return Some(vec![ch as u8]);
        }
        return Some(key.as_bytes().to_vec());
    }

    None
}

impl Terminal {
    pub fn new(
        root_dir: Option<&Path>,
        name: String,
        palette: gpui_terminal::ColorPalette,
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
            colors: palette,
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

        let pty_writer = Arc::new(Mutex::new(writer));
        let pty_writer_for_input = pty_writer.clone();

        let view = cx.new(|cx| {
            TerminalView::new(SharedWriter(pty_writer), reader, config, cx)
                .with_resize_callback(resize_callback)
                .with_key_handler(move |event| {
                    if let Some(bytes) = terminal_keystroke_to_bytes(&event.keystroke) {
                        if let Ok(mut writer) = pty_writer_for_input.lock() {
                            use std::io::Write;
                            let _ = writer.write_all(&bytes);
                            let _ = writer.flush();
                        }
                        return true;
                    }
                    false
                })
        });

        view.read(cx).focus_handle().focus(window);

        Self {
            view,
            name,
        }
    }

    /// Dynamically update the terminal color palette when the active theme changes.
    pub fn set_theme(&mut self, palette: &gpui_terminal::ColorPalette, cx: &mut Context<Self>) {
        self.view.update(cx, |view, cx| {
            let mut config = view.config().clone();
            config.colors = palette.clone();
            view.update_config(config, cx);
        });
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
                .bg(rgba(t.terminal_bg))
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Keystroke;

    #[test]
    fn test_capital_letters_with_shift() {
        let keystroke = Keystroke::parse("shift-a").unwrap();
        let bytes = terminal_keystroke_to_bytes(&keystroke);
        assert_eq!(bytes, Some(b"A".to_vec()));

        let keystroke_z = Keystroke::parse("shift-z").unwrap();
        let bytes_z = terminal_keystroke_to_bytes(&keystroke_z);
        assert_eq!(bytes_z, Some(b"Z".to_vec()));
    }

    #[test]
    fn test_lowercase_letters() {
        let keystroke = Keystroke::parse("a").unwrap();
        let bytes = terminal_keystroke_to_bytes(&keystroke);
        assert_eq!(bytes, Some(b"a".to_vec()));

        let keystroke_z = Keystroke::parse("z").unwrap();
        let bytes_z = terminal_keystroke_to_bytes(&keystroke_z);
        assert_eq!(bytes_z, Some(b"z".to_vec()));
    }

    #[test]
    fn test_shifted_symbols() {
        let k1 = Keystroke::parse("shift-1").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&k1), Some(b"!".to_vec()));

        let k_dash = Keystroke::parse("shift--").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&k_dash), Some(b"_".to_vec()));
    }

    #[test]
    fn test_ctrl_combinations() {
        let ctrl_c = Keystroke::parse("ctrl-c").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&ctrl_c), Some(vec![0x03]));

        let ctrl_a = Keystroke::parse("ctrl-a").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&ctrl_a), Some(vec![0x01]));

        let ctrl_z = Keystroke::parse("ctrl-z").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&ctrl_z), Some(vec![0x1a]));
    }

    #[test]
    fn test_special_keys() {
        let enter = Keystroke::parse("enter").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&enter), Some(b"\r".to_vec()));

        let backspace = Keystroke::parse("backspace").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&backspace), Some(b"\x7f".to_vec()));

        let tab = Keystroke::parse("tab").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&tab), Some(b"\t".to_vec()));

        let shift_tab = Keystroke::parse("shift-tab").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&shift_tab), Some(b"\x1b[Z".to_vec()));

        let up = Keystroke::parse("up").unwrap();
        assert_eq!(terminal_keystroke_to_bytes(&up), Some(b"\x1b[A".to_vec()));
    }
}

