//! Application entry point.
//!
//! Boots GPUI, registers keybindings/themes/fonts, then opens the single
//! window hosting [`workspace::Workspace`]. All state and behavior live in
//! the other modules — keep this file thin.

mod actions;
mod assets;
mod file_icons;
mod fs_tree;
mod git;
mod lang;
mod lsp;
mod terminal;
mod theme;
mod ui;
mod workspace;

use std::sync::Arc;

use gpui::{
    px, size, App, AppContext, Application, Bounds, KeyBinding, WindowBounds, WindowDecorations,
    WindowOptions,
};
use gpui_component::{Root, TitleBar};

use actions::*;
use assets::{load_embedded_fonts, sync_component_fonts, CombinedAssets};
use workspace::Workspace;

fn main() {
    Application::new()
        .with_assets(CombinedAssets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            // Windows/Linux keymap style (like Zed's default-linux.json):
            // `ctrl` modifiers only. On Windows GPUI maps `cmd` to the
            // Windows key, which produced wrong "Win+X" menu labels.
            cx.bind_keys([
                KeyBinding::new("ctrl-s", Save, None),
                KeyBinding::new("ctrl-n", NewFile, None),
                KeyBinding::new("ctrl-o", OpenFile, None),
                KeyBinding::new("ctrl-,", OpenSettings, None),
                KeyBinding::new("ctrl-`", ToggleTerminal, None),
                KeyBinding::new("ctrl-j", ToggleTerminal, None),
                KeyBinding::new("ctrl-shift-`", NewTerminal, None),
                KeyBinding::new("ctrl-b", ToggleSidebar, None),
                KeyBinding::new("ctrl-shift-e", ShowExplorer, None),
                KeyBinding::new("ctrl-shift-f", ShowSearch, None),
                KeyBinding::new("ctrl-shift-g", ShowGit, None),
                KeyBinding::new("ctrl-shift-x", ShowExtensions, None),
                // Tab keybindings
                KeyBinding::new("ctrl-w", CloseTab, None),
                KeyBinding::new("ctrl-tab", NextTab, None),
                KeyBinding::new("ctrl-shift-tab", PrevTab, None),
                // Font zoom keybindings (Zed-compatible)
                KeyBinding::new("ctrl-=", IncreaseFontSize, None),
                KeyBinding::new("ctrl-+", IncreaseFontSize, None),
                KeyBinding::new("ctrl-shift-+", IncreaseFontSize, None),
                KeyBinding::new("ctrl-shift-=", IncreaseFontSize, None),
                KeyBinding::new("ctrl--", DecreaseFontSize, None),
                KeyBinding::new("ctrl-_", DecreaseFontSize, None),
                KeyBinding::new("ctrl-0", ResetFontSize, None),
                KeyBinding::new("ctrl-alt-c", CopyDiagnostic, None),
                // Format the current document with its language server
                // (VS Code / Zed compatible).
                KeyBinding::new("shift-alt-f", FormatDocument, Some("Workspace")),
            ]);
            // Go to definition (F12, like VS Code). The editor library
            // resolves definitions with a modifier-hover first and jumps on
            // this action.
            cx.bind_keys([KeyBinding::new(
                "f12",
                gpui_component::input::GoToDefinition,
                Some("Input"),
            )]);

            // Start on GitHub Dark and paint tree-sitter captures with the
            // theme's exact syntax palette from day one.
            gpui_component::Theme::change(gpui_component::ThemeMode::Dark, None, cx);
            let default_theme = &theme::all()[theme::default_index()];
            gpui_component::Theme::global_mut(cx).highlight_theme =
                Arc::new(default_theme.highlight_theme());

            // Zed's fonts: IBM Plex Sans (UI) + Lilex (code).
            load_embedded_fonts(cx);
            sync_component_fonts(cx);

            let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitleBar::title_bar_options()),
                    window_decorations: Some(WindowDecorations::Client),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| Workspace::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
