//! User-intent actions ([`gpui::Action`]) dispatched by menus, the title bar
//! and keyboard shortcuts. They decouple "what the user asked for" from the
//! widget that observed the input.

use gpui::actions;

actions!(
    editor,
    [
        Save,
        Quit,
        NewFile,
        OpenFile,
        OpenFolder,
        ShowExplorer,
        ShowSearch,
        ShowGit,
        ShowExtensions,
        ToggleSidebar,
        ToggleTerminal,
        About
    ]
);

/// Select a theme by index into `theme::all`. Payload action (not bound to
/// any keymap), dispatched from the View → Theme submenu.
#[derive(Clone, Copy, Debug, Default, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct SelectTheme {
    pub ix: usize,
}
