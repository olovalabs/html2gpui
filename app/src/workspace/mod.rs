//! Application state. The single [`Workspace`] entity owns the open buffer,
//! the explorer tree and all UI flags; this module holds the struct plus the
//! commands that mutate it. Rendering lives in [`render`].

mod render;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState, TabSize};

use crate::fs_tree::{collapse_all, display_name, load_dir, reload_dir_preserving, TreeNode};
use crate::lang;
use crate::theme;

/// Represents a single open tab with its own editor buffer
#[derive(Clone)]
pub struct OpenTab {
    pub path: Option<PathBuf>,
    pub editor: Entity<InputState>,
    pub dirty: bool,
    pub untitled: bool,
    /// True if this is a preview tab (will be replaced when opening another file)
    /// VS Code behavior: clicking a file opens it in preview mode,
    /// editing promotes it to a permanent tab
    pub preview: bool,
}

/// Which sidebar panel is active.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Activity {
    Explorer,
    Search,
    Git,
    Extensions,
}

impl Activity {
    /// Label shown in the status bar when this panel is selected.
    pub(crate) fn status_label(self) -> &'static str {
        match self {
            Activity::Explorer => "EXPLORER",
            Activity::Search => "SEARCH",
            Activity::Git => "SOURCE CONTROL",
            Activity::Extensions => "EXTENSIONS",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CreatingKind {
    File,
    Folder,
}

#[derive(Clone)]
pub(crate) struct InlineCreating {
    pub(crate) kind: CreatingKind,
    pub(crate) parent_dir: PathBuf,
    pub(crate) input: Entity<InputState>,
}

pub(crate) struct Workspace {
    /// None until the user opens a folder (VS Code-style start state).
    pub(crate) root: Option<PathBuf>,
    pub(crate) tree: Vec<TreeNode>,
    /// Currently selected path in the explorer.
    pub(crate) selected_path: Option<PathBuf>,
    pub(crate) explorer_section_expanded: bool,
    pub(crate) inline_creating: Option<InlineCreating>,
    /// A file to open once the next render cycle runs.
    pub(crate) pending_open: Option<PathBuf>,
    pub(crate) status: String,
    pub(crate) activity: Activity,
    pub(crate) show_sidebar: bool,
    pub(crate) show_terminal: bool,
    pub(crate) theme_ix: usize,
    /// Open tabs
    pub(crate) tabs: Vec<OpenTab>,
    /// Index of the currently active tab
    pub(crate) active_tab: usize,
}

impl Workspace {
    pub(crate) fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        // Don't create any tabs on startup - only when files are opened

        // Start with NO project loaded — the welcome screen offers
        // Open Folder / Open File / New File (VS Code-style).
        Self {
            root: None,
            tree: Vec::new(),
            selected_path: None,
            explorer_section_expanded: true,
            inline_creating: None,
            pending_open: None,
            status: "Welcome — open a folder or create a file to begin".into(),
            activity: Activity::Explorer,
            show_sidebar: true,
            show_terminal: false,
            theme_ix: theme::default_index(),
            tabs: Vec::new(),
            active_tab: 0,
        }
    }

    pub(crate) fn theme(&self) -> &'static theme::Theme {
        let themes = theme::all();
        &themes[self.theme_ix.min(themes.len() - 1)]
    }

    /// Window/title-bar text: "file ● — folder", "folder" or the app name.
    pub(crate) fn title(&self) -> String {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            if let Some(path) = &tab.path {
                let star = if tab.dirty { " ●" } else { "" };
                let name = display_name(path);
                match &self.root {
                    Some(root) => format!("{name}{star} — {}", display_name(root)),
                    None => format!("{name}{star}"),
                }
            } else if tab.untitled {
                let star = if tab.dirty { " ●" } else { "" };
                format!("untitled{star} — gpui editor")
            } else {
                "gpui editor".to_string()
            }
        } else {
            "gpui editor".to_string()
        }
    }

    /// Returns true if welcome screen should be shown
    pub(crate) fn welcome_visible(&self) -> bool {
        // Show welcome when no files are open
        self.tabs.is_empty()
    }

    /// Get the active tab's editor
    pub fn active_editor(&self) -> Option<&Entity<InputState>> {
        self.tabs.get(self.active_tab).map(|t| &t.editor)
    }

    /// Get the active tab's file path
    pub fn active_path(&self) -> Option<&PathBuf> {
        self.tabs.get(self.active_tab)?.path.as_ref()
    }

    /// Check if active tab has unsaved changes
    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        self.tabs.get(self.active_tab).map(|t| t.dirty).unwrap_or(false)
    }

    // -- commands -----------------------------------------------------------

    pub(crate) fn apply_theme(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let themes = theme::all();
        let Some(th) = themes.get(ix) else {
            return;
        };
        self.theme_ix = ix;
        // Keep the widget library (inputs, menus, scrollbars…) in sync.
        let mode = if th.appearance == "light" {
            gpui_component::ThemeMode::Light
        } else {
            gpui_component::ThemeMode::Dark
        };
        gpui_component::Theme::change(mode, Some(window), cx);
        // Paint tree-sitter captures with this theme's exact syntax palette.
        gpui_component::Theme::global_mut(cx).highlight_theme = Arc::new(th.highlight_theme());
        // Re-apply Zed fonts — Theme::change resets families from its config.
        crate::assets::sync_component_fonts(cx);
        self.status = format!("Theme: {}", th.name);
        cx.notify();
    }

    pub(crate) fn load_root(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.root = Some(path.clone());
        self.tree = load_dir(&path);
        self.explorer_section_expanded = true;
        self.selected_path = None;
        self.inline_creating = None;
        self.status = format!("Opened folder {}", display_name(&path));
        cx.notify();
    }

    pub(crate) fn open_folder_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.load_root(path, cx);
        }
    }

    pub(crate) fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.open_file(path, window, cx);
        }
    }

    pub(crate) fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Create a new untitled tab
        let editor = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("text")
                .line_number(true)
                .indent_guides(false)
                .soft_wrap(false)
                .searchable(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .placeholder("Start typing...")
        });

        // Subscribe to change events for the new editor
        cx.subscribe(&editor, move |this, _state, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(tab) = this.tabs.get_mut(this.active_tab) {
                    if !tab.dirty {
                        tab.dirty = true;
                        cx.notify();
                    }
                }
            }
        })
        .detach();

        self.tabs.push(OpenTab {
            path: None,
            editor,
            dirty: false,
            untitled: true,
            preview: false, // New file tabs are permanent
        });
        self.active_tab = self.tabs.len() - 1;
        self.status = "Untitled file — Ctrl+S to save".into();
        cx.notify();
    }

    pub(crate) fn toggle_activity(&mut self, activity: Activity, cx: &mut Context<Self>) {
        if self.show_sidebar && self.activity == activity {
            self.show_sidebar = false;
        } else {
            self.show_sidebar = true;
            self.activity = activity;
        }
        self.status = self.activity.status_label().into();
        cx.notify();
    }

    pub(crate) fn set_activity_explicit(&mut self, activity: Activity, cx: &mut Context<Self>) {
        self.show_sidebar = true;
        self.activity = activity;
        self.status = activity.status_label().into();
        cx.notify();
    }

    pub(crate) fn open_file(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_path = Some(path.clone());

        // Check if file is already open in a tab
        if let Some(idx) = self.tabs.iter().position(|t| t.path.as_ref() == Some(&path)) {
            // File already open - switch to its tab and promote preview to permanent
            self.active_tab = idx;
            // Remove preview status - it's now a permanent tab
            if let Some(tab) = self.tabs.get_mut(idx) {
                tab.preview = false;
            }
            cx.notify();
            return;
        }

        // VS Code behavior: Check if current active tab is a preview tab
        // If yes, REPLACE it with the new file (don't add a new tab)
        // If no, ADD a new tab
        let replace_preview = if let Some(active_idx) = self.tabs.get(self.active_tab) {
            // Active tab is preview AND not dirty AND is the only tab or current view
            active_idx.preview && !active_idx.dirty
        } else {
            false
        };

        // Read file content
        match std::fs::read(&path) {
            Ok(bytes) => {
                if bytes.len() > 8_000_000 {
                    self.status = format!("{} is too large (>8MB)", display_name(&path));
                    cx.notify();
                    return;
                }
                if bytes.iter().take(8000).any(|&b| b == 0) {
                    self.status = format!("{} looks binary", display_name(&path));
                    cx.notify();
                    return;
                }
                let newline_count = bytes.iter().filter(|&&b| b == b'\n').count();
                let highlight = bytes.len() <= 400_000 && newline_count <= 8_000;
                let lang_id = if highlight {
                    lang::language_for(&path).unwrap_or("text")
                } else {
                    "text"
                };
                let text = String::from_utf8_lossy(&bytes).into_owned();

                // Create a new editor for this tab
                let editor = cx.new(|cx| {
                    let mut state = InputState::new(window, cx)
                        .code_editor(lang_id)
                        .line_number(true)
                        .indent_guides(false)
                        .soft_wrap(false)
                        .searchable(true)
                        .tab_size(TabSize {
                            tab_size: 4,
                            hard_tabs: false,
                        });
                    state.set_value(text, window, cx);
                    state
                });

                // Subscribe to change events - also handles promoting preview to permanent
                cx.subscribe(&editor, move |this, _state, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        if let Some(tab) = this.tabs.get_mut(this.active_tab) {
                            if !tab.dirty {
                                tab.dirty = true;
                            }
                            // VS Code: editing a preview tab promotes it to permanent
                            if tab.preview {
                                tab.preview = false;
                            }
                            cx.notify();
                        }
                    }
                })
                .detach();

                if replace_preview {
                    // REPLACE the current preview tab with the new file
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        tab.path = Some(path.clone());
                        tab.dirty = false;
                        tab.untitled = false;
                        tab.preview = true; // New file is still in preview mode
                        tab.editor = editor;
                    }
                } else {
                    // ADD a new tab in preview mode (VS Code style)
                    self.tabs.push(OpenTab {
                        path: Some(path.clone()),
                        editor,
                        dirty: false,
                        untitled: false,
                        preview: true, // New tabs start as preview
                    });
                    self.active_tab = self.tabs.len() - 1;
                }
                self.status = if highlight {
                    format!("{} · {}", path.display(), lang::lsp_status(&path))
                } else {
                    format!("{}  (plain — highlight off for large files)", path.display())
                };
            }
            Err(e) => self.status = format!("open failed: {e}"),
        }
        cx.notify();
    }

    pub(crate) fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Get active tab
        let tab = match self.active_tab_mut() {
            Some(t) => t,
            None => {
                self.status = "No file open".into();
                cx.notify();
                return;
            }
        };

        // If untitled, fall back to Save As
        if tab.path.is_none() {
            self.save_as(cx);
            return;
        }

        // Save the file
        let path = tab.path.clone().unwrap();
        let text = tab.editor.read(cx).value().to_string();
        match std::fs::write(&path, text.as_bytes()) {
            Ok(()) => {
                tab.dirty = false;
                self.status = format!("Saved {}", display_name(&path));
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
        cx.notify();
    }

    fn save_as(&mut self, cx: &mut Context<Self>) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("untitled.txt")
            .save_file()
        else {
            return;
        };

        // Get text first before mutable borrow
        let active_idx = self.active_tab;
        let text = {
            let tab = match self.tabs.get(active_idx) {
                Some(t) => t,
                None => return,
            };
            tab.editor.read(cx).value().to_string()
        };

        match std::fs::write(&path, text.as_bytes()) {
            Ok(()) => {
                let lang_id = lang::language_for(&path).unwrap_or("text");
                // Now update tab
                if let Some(tab) = self.tabs.get_mut(active_idx) {
                    tab.path = Some(path.clone());
                    tab.untitled = false;
                    tab.dirty = false;
                    tab.editor.update(cx, |state, cx| {
                        state.set_highlighter(lang_id, cx);
                    });
                }
                self.selected_path = Some(path.clone());
                if let Some(root) = &self.root {
                    self.tree = reload_dir_preserving(root, &self.tree);
                }
                self.status = format!("Saved {} · {}", path.display(), lang::lsp_status(&path));
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
        cx.notify();
    }

    /// Get mutable reference to the active tab
    fn active_tab_mut(&mut self) -> Option<&mut OpenTab> {
        self.tabs.get_mut(self.active_tab)
    }

    pub(crate) fn toggle_dir(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.selected_path = Some(path.to_path_buf());
        fn rec(nodes: &mut [TreeNode], path: &Path) -> bool {
            for n in nodes {
                if n.path == path {
                    n.expanded = !n.expanded;
                    if n.expanded && n.children.is_empty() {
                        n.children = load_dir(&n.path);
                    }
                    return true;
                }
                if rec(&mut n.children, path) {
                    return true;
                }
            }
            false
        }
        rec(&mut self.tree, path);
        cx.notify();
    }

    pub(crate) fn refresh_explorer(&mut self, cx: &mut Context<Self>) {
        if let Some(root) = &self.root {
            self.tree = reload_dir_preserving(root, &self.tree);
            self.status = "Explorer refreshed".into();
            cx.notify();
        }
    }

    pub(crate) fn collapse_all_folders(&mut self, cx: &mut Context<Self>) {
        collapse_all(&mut self.tree);
        self.status = "Collapsed all folders".into();
        cx.notify();
    }

    pub(crate) fn toggle_explorer_section(&mut self, cx: &mut Context<Self>) {
        self.explorer_section_expanded = !self.explorer_section_expanded;
        cx.notify();
    }

    pub(crate) fn start_inline_create(
        &mut self,
        kind: CreatingKind,
        parent: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target_dir = parent
            .or_else(|| {
                self.selected_path.as_ref().map(|p| {
                    if p.is_dir() {
                        p.clone()
                    } else {
                        p.parent().unwrap_or(p).to_path_buf()
                    }
                })
            })
            .or_else(|| self.root.clone());

        let Some(dir) = target_dir else {
            if kind == CreatingKind::File {
                self.new_file(window, cx);
            }
            return;
        };

        // Expand parent directory so the new item is visible in the tree
        if Some(&dir) != self.root.as_ref() {
            fn expand_path(nodes: &mut [TreeNode], target: &Path) {
                for n in nodes {
                    if target.starts_with(&n.path) {
                        n.expanded = true;
                        if n.children.is_empty() {
                            n.children = load_dir(&n.path);
                        }
                        expand_path(&mut n.children, target);
                    }
                }
            }
            expand_path(&mut self.tree, &dir);
        }
        self.explorer_section_expanded = true;

        let input = cx.new(|cx| {
            let state = InputState::new(window, cx)
                .code_editor("text")
                .line_number(false);
            state.focus(window, cx);
            state
        });

        cx.subscribe(&input, |this, _state, event: &InputEvent, cx| {
            match event {
                InputEvent::PressEnter { .. } => {
                    this.confirm_inline_create(cx);
                }
                InputEvent::Blur => {
                    this.cancel_inline_create(cx);
                }
                _ => {}
            }
        })
        .detach();

        self.inline_creating = Some(InlineCreating {
            kind,
            parent_dir: dir,
            input,
        });
        cx.notify();
    }

    pub(crate) fn confirm_inline_create(&mut self, cx: &mut Context<Self>) {
        let Some(creating) = self.inline_creating.take() else {
            return;
        };
        let raw_name = creating.input.read(cx).value().to_string();
        let name = raw_name.trim();
        if name.is_empty() {
            cx.notify();
            return;
        }

        let target_path = creating.parent_dir.join(name);

        match creating.kind {
            CreatingKind::File => {
                if let Some(parent) = target_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Ok(()) = std::fs::write(&target_path, b"") {
                    if let Some(root) = &self.root {
                        self.tree = reload_dir_preserving(root, &self.tree);
                    }
                    self.selected_path = Some(target_path.clone());
                    self.pending_open = Some(target_path.clone());
                    self.status = format!("Created {}", display_name(&target_path));
                }
            }
            CreatingKind::Folder => {
                if let Ok(()) = std::fs::create_dir_all(&target_path) {
                    if let Some(root) = &self.root {
                        self.tree = reload_dir_preserving(root, &self.tree);
                    }
                    self.selected_path = Some(target_path.clone());
                    self.status = format!("Created folder {}", display_name(&target_path));
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn cancel_inline_create(&mut self, cx: &mut Context<Self>) {
        if self.inline_creating.is_some() {
            self.inline_creating = None;
            cx.notify();
        }
    }

    pub(crate) fn reveal_in_explorer(&mut self, path: &Path, cx: &mut Context<Self>) {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(path.parent().unwrap_or(path))
                .spawn();
        }
        self.status = format!("Revealed {}", display_name(path));
        cx.notify();
    }

    pub(crate) fn copy_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.to_string_lossy().to_string()));
        self.status = format!("Copied path: {}", path.display());
        cx.notify();
    }

    pub(crate) fn copy_relative_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        let rel = if let Some(root) = &self.root {
            path.strip_prefix(root).unwrap_or(path)
        } else {
            path
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(rel.to_string_lossy().to_string()));
        self.status = format!("Copied relative path: {}", rel.display());
        cx.notify();
    }

    pub(crate) fn delete_entry(&mut self, path: &Path, cx: &mut Context<Self>) {
        let is_dir = path.is_dir();
        let res = if is_dir {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        if res.is_ok() {
            // Close any tabs that have this file open
            self.tabs.retain(|t| t.path.as_ref() != Some(&path.to_path_buf()));
            // Adjust active_tab if needed
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len().saturating_sub(1);
            }
            if self.selected_path.as_ref() == Some(&path.to_path_buf()) {
                self.selected_path = None;
            }
            if let Some(root) = &self.root {
                self.tree = reload_dir_preserving(root, &self.tree);
            }
            self.status = format!("Deleted {}", display_name(path));
        } else if let Err(e) = res {
            self.status = format!("Failed to delete: {e}");
        }
        cx.notify();
    }

    pub(crate) fn rename_entry(&mut self, path: &Path, cx: &mut Context<Self>) {
        if let Some(parent) = path.parent() {
            let name = display_name(path);
            if let Some(new_path) = rfd::FileDialog::new()
                .set_directory(parent)
                .set_file_name(&name)
                .save_file()
            {
                if std::fs::rename(path, &new_path).is_ok() {
                    // Update any tabs that have this file open
                    for tab in &mut self.tabs {
                        if tab.path.as_ref() == Some(&path.to_path_buf()) {
                            tab.path = Some(new_path.clone());
                        }
                    }
                    if self.selected_path.as_ref() == Some(&path.to_path_buf()) {
                        self.selected_path = Some(new_path.clone());
                    }
                    if let Some(root) = &self.root {
                        self.tree = reload_dir_preserving(root, &self.tree);
                    }
                    self.status = format!("Renamed to {}", display_name(&new_path));
                }
            }
        }
        cx.notify();
    }

    // Tab management methods

    /// Switch to a specific tab by index
    #[allow(dead_code)]
    pub(crate) fn switch_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    /// Close a tab by index
    pub(crate) fn close_tab(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        // Don't close if only one tab and it's clean (just show welcome)
        if self.tabs.len() == 1 {
            self.tabs.remove(0);
            self.active_tab = 0;
            cx.notify();
            return;
        }

        // Remove the tab
        self.tabs.remove(index);

        // Adjust active tab index
        if index <= self.active_tab && self.active_tab > 0 {
            self.active_tab -= 1;
        }
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len().saturating_sub(1);
        }

        cx.notify();
    }

    /// Switch to a specific tab by index (called from tab bar click)
    pub(crate) fn switch_tab_to(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            self.active_tab = index;
            // Clicking a tab also promotes it from preview to permanent (VS Code behavior)
            if let Some(tab) = self.tabs.get_mut(index) {
                tab.preview = false;
            }
            cx.notify();
        }
    }

    /// Close a tab at a specific index (called from tab close button click)
    pub(crate) fn close_tab_at_index(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.tabs.len() {
            // Don't close if only one tab - just show welcome
            if self.tabs.len() == 1 {
                self.tabs.remove(0);
                self.active_tab = 0;
            } else {
                // Remove the tab
                self.tabs.remove(index);
                // Adjust active tab index
                if index <= self.active_tab && self.active_tab > 0 {
                    self.active_tab -= 1;
                }
                if self.active_tab >= self.tabs.len() {
                    self.active_tab = self.tabs.len().saturating_sub(1);
                }
            }
            cx.notify();
        }
    }

    /// Action handlers for tab operations

    pub(crate) fn handle_close_tab(&mut self, _: &crate::actions::CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tab(self.active_tab, window, cx);
    }

    pub(crate) fn handle_next_tab(&mut self, _: &crate::actions::NextTab, _window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
            cx.notify();
        }
    }

    pub(crate) fn handle_prev_tab(&mut self, _: &crate::actions::PrevTab, _window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
            cx.notify();
        }
    }

    /// Switch to a specific tab by index (called when tab is clicked)
    pub(crate) fn handle_switch_tab(&mut self, action: &crate::actions::SwitchTab, _window: &mut Window, cx: &mut Context<Self>) {
        if action.index < self.tabs.len() {
            self.active_tab = action.index;
            cx.notify();
        }
    }

    /// Close a specific tab by index (called when close button is clicked)
    pub(crate) fn handle_close_tab_at(&mut self, action: &crate::actions::CloseTabAt, window: &mut Window, cx: &mut Context<Self>) {
        if action.index < self.tabs.len() {
            self.close_tab(action.index, window, cx);
        }
    }

    pub(crate) fn quit(&mut self, cx: &mut Context<Self>) {
        cx.quit();
    }

    pub(crate) fn about(&mut self, cx: &mut Context<Self>) {
        self.status = format!("gpui editor — {} (Zed theme system)", self.theme().name);
        cx.notify();
    }
}
