//! User-intent actions ([`gpui::Action`]) dispatched by menus, the title bar
//! and keyboard shortcuts. They decouple "what the user asked for" from the
//! widget that observed the input.

use std::path::PathBuf;

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
        About,
        ExplorerRefresh,
        ExplorerCollapseAll,
        CloseTab,
        NextTab,
        PrevTab,
        CloseActiveTab,
        IncreaseFontSize,
        DecreaseFontSize,
        ResetFontSize,
        CopyDiagnostic
    ]
);

/// Select a specific tab by index (used by tab bar clicks)
#[derive(Clone, Copy, Debug, Default, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct SwitchTab {
    pub index: usize,
}

/// Close a specific tab by index (used by tab close button clicks)
#[derive(Clone, Copy, Debug, Default, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct CloseTabAt {
    pub index: usize,
}

/// Select a theme by index into `theme::all`. Payload action (not bound to
/// any keymap), dispatched from the View → Theme submenu.
#[derive(Clone, Copy, Debug, Default, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct SelectTheme {
    pub ix: usize,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct ExplorerNewFile {
    pub parent: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct ExplorerNewFolder {
    pub parent: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct ExplorerRevealInFinder {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct ExplorerCopyPath {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct ExplorerCopyRelativePath {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct ExplorerRename {
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(no_json)]
pub struct ExplorerDelete {
    pub path: PathBuf,
}
