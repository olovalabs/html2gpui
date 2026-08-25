//! Application state. The single [`Workspace`] entity owns the open buffer,
//! the explorer tree and all UI flags; this module holds the struct plus the
//! commands that mutate it. Rendering lives in [`render`].

mod render;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState, TabSize};

use crate::fs_tree::{display_name, load_dir, TreeNode};
use crate::lang;
use crate::theme;

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

pub(crate) struct Workspace {
    /// None until the user opens a folder (VS Code-style start state).
    pub(crate) root: Option<PathBuf>,
    pub(crate) tree: Vec<TreeNode>,
    pub(crate) open: Option<PathBuf>,
    /// Buffer was created via "New File" and has no path yet.
    pub(crate) untitled: bool,
    pub(crate) dirty: bool,
    pub(crate) status: String,
    pub(crate) tree_scroll: f32,
    pub(crate) activity: Activity,
    pub(crate) show_sidebar: bool,
    pub(crate) show_terminal: bool,
    pub(crate) theme_ix: usize,
    pub(crate) editor: Entity<InputState>,
}

impl Workspace {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
                .placeholder("Open a file from the explorer")
        });

        cx.subscribe(&editor, |this, _state, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) && this.open.is_some() && !this.dirty {
                this.dirty = true;
                cx.notify();
            }
        })
        .detach();

        // Start with NO project loaded — the welcome screen offers
        // Open Folder / Open File / New File (VS Code-style).
        Self {
            root: None,
            tree: Vec::new(),
            open: None,
            untitled: false,
            dirty: false,
            status: "Welcome — open a folder or create a file to begin".into(),
            tree_scroll: 0.0,
            activity: Activity::Explorer,
            show_sidebar: true,
            show_terminal: false,
            theme_ix: theme::default_index(),
            editor,
        }
    }

    pub(crate) fn theme(&self) -> &'static theme::Theme {
        let themes = theme::all();
        &themes[self.theme_ix.min(themes.len() - 1)]
    }

    /// Window/title-bar text: "file ● — folder", "folder" or the app name.
    pub(crate) fn title(&self) -> String {
        match (&self.open, &self.root) {
            (Some(p), _) => {
                let star = if self.dirty { " ●" } else { "" };
                let name = display_name(p);
                match self.root.as_deref().map(display_name) {
                    Some(folder) => format!("{name}{star} — {folder}"),
                    None => format!("{name}{star}"),
                }
            }
            (_, Some(r)) => display_name(r),
            _ if self.untitled => {
                let star = if self.dirty { " ●" } else { "" };
                format!("untitled{star} — gpui editor")
            }
            _ => "gpui editor".to_string(),
        }
    }

    pub(crate) fn welcome_visible(&self) -> bool {
        self.open.is_none() && !self.untitled && self.root.is_none()
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
        self.editor.update(cx, |state, cx| {
            state.set_highlighter("text", cx);
            state.set_value("", window, cx);
        });
        self.open = None;
        self.untitled = true;
        self.dirty = false;
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
                self.editor.update(cx, |state, cx| {
                    state.set_indent_guides(false, window, cx);
                    state.set_highlighter(lang_id, cx);
                    state.set_value(text, window, cx);
                });
                self.open = Some(path.clone());
                self.untitled = false;
                self.dirty = false;
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
        // Untitled buffer → fall back to Save As.
        if self.open.is_none() {
            if !self.untitled {
                self.status = "No file open".into();
                cx.notify();
                return;
            }
            self.save_as(cx);
            return;
        }
        let path = self.open.clone().unwrap();
        let text = self.editor.read(cx).value().to_string();
        match std::fs::write(&path, text.as_bytes()) {
            Ok(()) => {
                self.dirty = false;
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
        let text = self.editor.read(cx).value().to_string();
        match std::fs::write(&path, text.as_bytes()) {
            Ok(()) => {
                self.open = Some(path.clone());
                self.untitled = false;
                self.dirty = false;
                let lang_id = lang::language_for(&path).unwrap_or("text");
                self.editor.update(cx, |state, cx| {
                    state.set_highlighter(lang_id, cx);
                });
                self.status = format!("Saved {} · {}", path.display(), lang::lsp_status(&path));
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
        cx.notify();
    }

    pub(crate) fn toggle_dir(&mut self, path: &Path, cx: &mut Context<Self>) {
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

    pub(crate) fn quit(&mut self, cx: &mut Context<Self>) {
        cx.quit();
    }

    pub(crate) fn about(&mut self, cx: &mut Context<Self>) {
        self.status = format!("gpui editor — {} (Zed theme system)", self.theme().name);
        cx.notify();
    }
}
