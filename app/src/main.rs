use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    actions, div, img, prelude::*, px, rgba, size, svg, App, Application, Bounds, Context,
    Entity, FontWeight, KeyBinding, SharedString, Window, WindowBounds, WindowDecorations,
    WindowOptions,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    input::{Copy, Cut, Input, InputEvent, InputState, Paste, Redo, SelectAll, TabSize, Undo},
    menu::{DropdownMenu as _, PopupMenu},
    Root, TitleBar,
};
use rust_embed::RustEmbed;

mod file_icons;
mod theme;

/// Select a theme by index into [`theme::all`]. Payload action (not bound to
/// any keymap), dispatched from the View → Theme submenu.
#[derive(Clone, Copy, Debug, Default, PartialEq, gpui::Action)]
#[action(no_json)]
struct SelectTheme {
    ix: usize,
}

#[derive(RustEmbed)]
#[folder = "assets/"]
struct AppAssets;

struct CombinedAssets;

impl gpui::AssetSource for CombinedAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let clean = path
            .trim_start_matches("icons/")
            .trim_start_matches("assets/")
            .trim_start_matches('/');
        if let Some(file) = AppAssets::get(clean) {
            return Ok(Some(file.data));
        }
        if let Some(file) = AppAssets::get(path) {
            return Ok(Some(file.data));
        }
        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        let mut list = Vec::new();
        for file in AppAssets::iter() {
            list.push(gpui::SharedString::from(file.to_string()));
        }
        if let Ok(other) = gpui_component_assets::Assets.list(path) {
            list.extend(other);
        }
        Ok(list)
    }
}

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

use theme::Colors;

/// Zed's shipped fonts (`assets/fonts` in zed-industries/zed):
/// IBM Plex Sans for the UI, Lilex for code buffers.
pub const SANS_FONT: &str = "IBM Plex Sans";
pub const MONO_FONT: &str = "Lilex";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Activity {
    Explorer,
    Search,
    Git,
    Extensions,
}

#[derive(Clone)]
struct TreeNode {
    name: String,
    path: PathBuf,
    is_dir: bool,
    expanded: bool,
    children: Vec<TreeNode>,
}

struct Workspace {
    /// None until the user opens a folder (VS Code-style start state).
    root: Option<PathBuf>,
    tree: Vec<TreeNode>,
    open: Option<PathBuf>,
    /// Buffer was created via "New File" and has no path yet.
    untitled: bool,
    dirty: bool,
    status: String,
    tree_scroll: f32,
    activity: Activity,
    show_sidebar: bool,
    show_terminal: bool,
    theme_ix: usize,
    editor: Entity<InputState>,
}

impl Workspace {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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

    fn theme(&self) -> &'static theme::Theme {
        let themes = theme::all();
        &themes[self.theme_ix.min(themes.len() - 1)]
    }

    fn apply_theme(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
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
        sync_component_fonts(cx);
        self.status = format!("Theme: {}", th.name);
        cx.notify();
    }

    fn load_root(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.root = Some(path.clone());
        self.tree = load_dir(&path);
        self.status = format!("Opened folder {}", display_name(&path));
        cx.notify();
    }

    fn open_folder_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.load_root(path, cx);
        }
    }

    fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.open_file(path, window, cx);
        }
    }

    fn new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    fn toggle_activity(&mut self, activity: Activity, cx: &mut Context<Self>) {
        if self.show_sidebar && self.activity == activity {
            self.show_sidebar = false;
        } else {
            self.show_sidebar = true;
            self.activity = activity;
        }
        self.status = match self.activity {
            Activity::Explorer => "EXPLORER".into(),
            Activity::Search => "SEARCH".into(),
            Activity::Git => "SOURCE CONTROL".into(),
            Activity::Extensions => "EXTENSIONS".into(),
        };
        cx.notify();
    }

    fn set_activity_explicit(&mut self, activity: Activity, cx: &mut Context<Self>) {
        self.show_sidebar = true;
        self.activity = activity;
        self.status = match activity {
            Activity::Explorer => "EXPLORER".into(),
            Activity::Search => "SEARCH".into(),
            Activity::Git => "SOURCE CONTROL".into(),
            Activity::Extensions => "EXTENSIONS".into(),
        };
        cx.notify();
    }

    fn language_for(path: &Path) -> Option<&'static str> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        match name {
            "Dockerfile" | "Containerfile" => return Some("dockerfile"),
            "Makefile" => return Some("make"),
            _ => {}
        }
        Some(match ext.as_str() {
            "rs" => "rust",
            "js" | "mjs" | "cjs" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "py" => "python",
            "html" | "htm" => "html",
            "css" => "css",
            "json" => "json",
            "toml" => "toml",
            "yaml" | "yml" => "yaml",
            "md" | "markdown" => "markdown",
            "go" => "go",
            "c" | "h" => "c",
            "cpp" | "cc" | "hpp" => "cpp",
            "java" => "java",
            "rb" => "ruby",
            "sh" | "bash" => "bash",
            "php" => "php",
            "swift" => "swift",
            "kt" => "kotlin",
            "scala" => "scala",
            "lua" => "lua",
            "zig" => "zig",
            "cs" => "c-sharp",
            "sql" => "sql",
            "r" => "r",
            "xml" => "xml",
            _ => return None,
        })
    }

    /// The language server binary Zed uses by default for this language.
    fn lsp_binary_for(lang: &str) -> Option<&'static str> {
        Some(match lang {
            "rust" => "rust-analyzer",
            "go" => "gopls",
            "python" => "basedpyright-langserver",
            "c" | "cpp" => "clangd",
            "javascript" | "typescript" => "typescript-language-server",
            "lua" => "lua-language-server",
            "zig" => "zls",
            "ruby" => "ruby-lsp",
            "java" => "jdtls",
            "php" => "intelephense",
            "bash" => "bash-language-server",
            "yaml" => "yaml-language-server",
            _ => return None,
        })
    }

    /// Human-readable LSP availability for the status bar.
    fn lsp_status(path: &Path) -> String {
        let Some(lang) = Self::language_for(path) else {
            return "plain text".into();
        };
        match Self::lsp_binary_for(lang) {
            Some(bin) if Self::binary_on_path(bin) => format!("{lang} · LSP ready ({bin})"),
            Some(bin) => format!("{lang} · LSP server '{bin}' not found on PATH"),
            None => format!("{lang} · no default LSP"),
        }
    }

    fn binary_on_path(binary: &str) -> bool {
        let exe = if cfg!(windows) && !binary.ends_with(".exe") {
            format!("{binary}.exe")
        } else {
            binary.to_string()
        };
        std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths)
                    .any(|dir| dir.join(binary).is_file() || dir.join(&exe).is_file())
            })
            .unwrap_or(false)
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
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
                let lang = if highlight {
                    Self::language_for(&path).unwrap_or("text")
                } else {
                    "text"
                };
                let text = String::from_utf8_lossy(&bytes).into_owned();
                self.editor.update(cx, |state, cx| {
                    state.set_indent_guides(false, window, cx);
                    state.set_highlighter(lang, cx);
                    state.set_value(text, window, cx);
                });
                self.open = Some(path.clone());
                self.untitled = false;
                self.dirty = false;
                self.status = if highlight {
                    format!("{} · {}", path.display(), Self::lsp_status(&path))
                } else {
                    format!(
                        "{}  (plain — highlight off for large files)",
                        path.display()
                    )
                };
            }
            Err(e) => self.status = format!("open failed: {e}"),
        }
        cx.notify();
    }

    fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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
                let lang = Self::language_for(&path).unwrap_or("text");
                self.editor.update(cx, |state, cx| {
                    state.set_highlighter(lang, cx);
                });
                self.status = format!("Saved {} · {}", path.display(), Self::lsp_status(&path));
            }
            Err(e) => self.status = format!("save failed: {e}"),
        }
        cx.notify();
    }

    fn toggle_dir(&mut self, path: &Path, cx: &mut Context<Self>) {
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

    fn quit(&mut self, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn welcome_visible(&self) -> bool {
        self.open.is_none() && !self.untitled && self.root.is_none()
    }

    fn about(&mut self, cx: &mut Context<Self>) {
        self.status = format!("gpui editor — {} (Zed theme system)", self.theme().name);
        cx.notify();
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let th = self.theme();
        let t = th.colors;
        let untitled = self.untitled;
        let welcome = self.welcome_visible();
        let title = match (&self.open, &self.root) {
            (Some(p), _) => {
                let star = if self.dirty { " ●" } else { "" };
                let name = display_name(p);
                match self.root.as_deref().map(display_name) {
                    Some(folder) => format!("{name}{star} — {folder}"),
                    None => format!("{name}{star}"),
                }
            }
            (_, Some(r)) => display_name(r),
            _ if untitled => {
                let star = if self.dirty { " ●" } else { "" };
                format!("untitled{star} — gpui editor")
            }
            _ => "gpui editor".to_string(),
        };
        let tree = &self.tree;
        let open = self.open.clone();
        let tree_scroll = self.tree_scroll;
        let status = self.status.clone();
        let editor = self.editor.clone();
        let activity = self.activity;
        let root_opt = self.root.clone();
        let show_sidebar = self.show_sidebar;
        let show_terminal = self.show_terminal;
        let theme_ix = self.theme_ix;
        let theme_name = th.name.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(SANS_FONT)
            .bg(rgba(t.background))
            .text_color(rgba(t.text))
            .text_size(px(13.0))
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save(window, cx)))
            .on_action(cx.listener(|this, _: &Quit, _, cx| this.quit(cx)))
            .on_action(cx.listener(|this, _: &ShowExplorer, _, cx| {
                this.set_activity_explicit(Activity::Explorer, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowSearch, _, cx| {
                this.set_activity_explicit(Activity::Search, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowGit, _, cx| {
                this.set_activity_explicit(Activity::Git, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowExtensions, _, cx| {
                this.set_activity_explicit(Activity::Extensions, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.show_sidebar = !this.show_sidebar;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, _, cx| {
                this.show_terminal = !this.show_terminal;
                this.status = if this.show_terminal {
                    "Terminal".into()
                } else {
                    "Terminal hidden".into()
                };
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NewFile, window, cx| this.new_file(window, cx)))
            .on_action(cx.listener(|this, _: &OpenFile, window, cx| {
                this.open_file_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFolder, window, cx| {
                this.open_folder_dialog(window, cx);
            }))
            .on_action(cx.listener(
                |this, action: &SelectTheme, window, cx| {
                    this.apply_theme(action.ix, window, cx);
                },
            ))
            .on_action(cx.listener(|this, _: &About, _, cx| this.about(cx)))
            .child(render_titlebar(&title, &t, theme_ix, &theme_name))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .child(render_activity_bar(activity, show_sidebar, &t, cx))
                    .when(show_sidebar, |d| {
                        d.child(match activity {
                            Activity::Explorer => match &root_opt {
                                Some(root) => render_tree(
                                    tree,
                                    open.as_ref(),
                                    tree_scroll,
                                    &display_name(root),
                                    &t,
                                    cx,
                                ),
                                None => render_no_folder_panel(&t, cx),
                            },
                            Activity::Search => render_search_panel(&t),
                            Activity::Git => render_git_panel(&t),
                            Activity::Extensions => render_extensions_panel(&t),
                        })
                    })
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .bg(rgba(t.editor_bg))
                            .when(welcome, |d| d.child(render_welcome(&t, cx)))
                            .when(!welcome, |d| {
                                d.child(
                                    div()
                                        .flex_1()
                                        .min_h(px(0.0))
                                        .overflow_hidden()
                                        .child(
                                            Input::new(&editor)
                                                .h_full()
                                                .appearance(false)
                                                .bordered(false),
                                        ),
                                )
                            })
                            .when(show_terminal, |d| d.child(render_terminal(&t))),
                    ),
            )
            .child(
                div()
                    .h(px(26.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(10.0))
                    .bg(rgba(t.status_bar))
                    .border_t_1()
                    .border_color(rgba(t.border_variant))
                    .text_size(px(12.0))
                    .text_color(rgba(t.text))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(SharedString::from(status)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(svg()
                                .path("ui_icons/settings.svg")
                                .w(px(13.0))
                                .h(px(13.0))
                                .text_color(rgba(t.text)))
                            .child(SharedString::from(theme_name))
                            .child(SharedString::from("UTF-8"))
                            .child(SharedString::from("Spaces: 4")),
                    ),
            )
    }
}

fn render_titlebar(
    title: &str,
    t: &Colors,
    theme_ix: usize,
    _theme_name: &str,
) -> impl IntoElement {
    TitleBar::new()
        .bg(rgba(t.title_bar))
        .border_color(rgba(t.border))
        .child(
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_row()
                .items_center()
                .text_size(px(13.0))
                .child(menu_btn("m-file", "File", t, |menu, _, _| {
                    menu.menu("New File", Box::new(NewFile))
                        .separator()
                        .menu("Open File…", Box::new(OpenFile))
                        .menu("Open Folder…", Box::new(OpenFolder))
                        .separator()
                        .menu("Save", Box::new(Save))
                        .separator()
                        .menu("Exit", Box::new(Quit))
                }))
                .child(menu_btn("m-edit", "Edit", t, |menu, _, _| {
                    menu.menu("Undo", Box::new(Undo))
                        .menu("Redo", Box::new(Redo))
                        .separator()
                        .menu("Cut", Box::new(Cut))
                        .menu("Copy", Box::new(Copy))
                        .menu("Paste", Box::new(Paste))
                        .separator()
                        .menu("Select All", Box::new(SelectAll))
                }))
                .child(menu_btn("m-view", "View", t, move |menu, window, cx| {
                    menu.menu("Explorer", Box::new(ShowExplorer))
                        .menu("Search", Box::new(ShowSearch))
                        .menu("Source Control", Box::new(ShowGit))
                        .menu("Extensions", Box::new(ShowExtensions))
                        .separator()
                        .submenu("Theme", window, cx, move |menu, _, _| {
                            let mut m = menu;
                            for (i, th) in theme::all().iter().enumerate() {
                                m = m.menu_with_check(
                                    th.name.clone(),
                                    i == theme_ix,
                                    Box::new(SelectTheme { ix: i }),
                                );
                            }
                            m
                        })
                        .separator()
                        .menu("Toggle Primary Side Bar", Box::new(ToggleSidebar))
                        .menu("Toggle Terminal", Box::new(ToggleTerminal))
                }))
                .child(menu_btn("m-term", "Terminal", t, |menu, _, _| {
                    menu.menu("New Terminal", Box::new(ToggleTerminal))
                }))
                .child(menu_btn("m-help", "Help", t, |menu, _, _| {
                    menu.menu("About", Box::new(About))
                }))
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.0))
                        .text_color(rgba(t.text))
                        .child(SharedString::from(title.to_string())),
                ),
        )
}

fn menu_btn(
    id: &'static str,
    label: &'static str,
    t: &Colors,
    build: impl Fn(
            PopupMenu,
            &mut Window,
            &mut Context<PopupMenu>,
        ) -> PopupMenu
        + 'static,
) -> impl IntoElement {
    Button::new(id)
        .ghost()
        .compact()
        .label(label)
        .text_color(rgba(t.text))
        .dropdown_menu(build)
}

/// Renders an embedded SVG asset at a fixed size, preserving its own colors.
/// (`img()` rasterizes full-color; `svg()` would tint everything one color.)
fn icon_img(path: &'static str, size: f32) -> impl IntoElement {
    div()
        .flex_none()
        .w(px(size))
        .h(px(size))
        .child(img(path).w(px(size)).h(px(size)))
}

fn panel_header(label: &'static str, t: &Colors) -> gpui::Div {
    div()
        .h(px(35.0))
        .px(px(16.0))
        .flex()
        .items_center()
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgba(t.text_muted))
        .child(SharedString::from(label))
}

/// VS Code-style start screen shown when no folder is open and no file is
/// being edited.
fn render_welcome(t: &Colors, cx: &mut Context<Workspace>) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.0))
        .bg(rgba(t.editor_bg))
        .child(
            div()
                .text_size(px(42.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgba(t.text))
                .child(SharedString::from("gpui editor")),
        )
        .child(
            div()
                .text_size(px(14.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(
                    "Native code editor · tree-sitter highlighting · Zed themes",
                )),
        )
        .child(div().h(px(16.0)))
        .child(welcome_button("Open Folder", true, t, cx.listener(
            |this, _, window, cx| this.open_folder_dialog(window, cx),
        )))
        .child(welcome_button("Open File", false, t, cx.listener(
            |this, _, window, cx| this.open_file_dialog(window, cx),
        )))
        .child(welcome_button("New File", false, t, cx.listener(
            |this, _, window, cx| this.new_file(window, cx),
        )))
        .child(div().h(px(8.0)))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(
                    "Ctrl+N new file · Ctrl+O open file · Ctrl+S save · Ctrl+F search",
                )),
        )
}

fn welcome_button(
    label: &'static str,
    primary: bool,
    t: &Colors,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("wb-{label}")))
        .w(px(220.0))
        .h(px(34.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .cursor_pointer()
        .text_size(px(13.0))
        .when(primary, |d| {
            d.bg(rgba(t.border_focused))
                .text_color(rgba(t.background))
                .hover(|s| s.bg(rgba(t.icon_accent)))
        })
        .when(!primary, |d| {
            d.bg(rgba(t.element_bg))
                .border_1()
                .border_color(rgba(t.border))
                .text_color(rgba(t.text))
                .hover(|s| s.bg(rgba(t.element_hover)))
        })
        .child(SharedString::from(label))
        .on_click(on_click)
}

/// Explorer placeholder when the app started without a folder.
fn render_no_folder_panel(t: &Colors, cx: &mut Context<Workspace>) -> gpui::AnyElement {
    div()
        .w(px(260.0))
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(10.0))
        .pt(px(48.0))
        .bg(rgba(t.panel))
        .border_r_1()
        .border_color(rgba(t.border_variant))
        .child(panel_header("EXPLORER", t))
        .child(
            div()
                .px(px(12.0))
                .text_size(px(12.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from("No folder opened")),
        )
        .child(welcome_button("Open Folder", true, t, cx.listener(
            |this, _, window, cx| this.open_folder_dialog(window, cx),
        )))
        .into_any_element()
}

fn render_terminal(t: &Colors) -> impl IntoElement {
    div()
        .h(px(160.0))
        .w_full()
        .flex()
        .flex_col()
        .bg(rgba(t.terminal_bg))
        .border_t_1()
        .border_color(rgba(t.border_variant))
        .child(
            div()
                .h(px(28.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .bg(rgba(t.toolbar))
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgba(t.text_muted))
                .child(SharedString::from("TERMINAL")),
        )
        .child(
            div()
                .flex_1()
                .p(px(10.0))
                .font_family(MONO_FONT)
                .text_size(px(12.0))
                .text_color(rgba(t.editor_fg))
                .child(SharedString::from(
                    "Terminal panel — shell is not connected yet. View → Toggle Terminal to hide.",
                )),
        )
}

fn render_activity_bar(
    activity: Activity,
    show_sidebar: bool,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    div()
        .w(px(48.0))
        .h_full()
        .flex()
        .flex_col()
        .justify_between()
        .bg(rgba(t.panel))
        .border_r_1()
        .border_color(rgba(t.border_variant))
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .child(activity_icon(
                    "act-explorer",
                    "ui_icons/file_tree.svg",
                    show_sidebar && activity == Activity::Explorer,
                    Activity::Explorer,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-search",
                    "ui_icons/magnifying_glass.svg",
                    show_sidebar && activity == Activity::Search,
                    Activity::Search,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-git",
                    "ui_icons/git_branch.svg",
                    show_sidebar && activity == Activity::Git,
                    Activity::Git,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-ext",
                    "ui_icons/blocks.svg",
                    show_sidebar && activity == Activity::Extensions,
                    Activity::Extensions,
                    t,
                    cx,
                )),
        )
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .child(activity_static_icon(
                    "act-settings",
                    "ui_icons/settings.svg",
                    t,
                    move |this, _, window, cx| {
                        // Settings cycles to the next theme for now.
                        let next = (this.theme_ix + 1) % theme::all().len();
                        this.apply_theme(next, window, cx);
                    },
                    cx,
                )),
        )
}

fn activity_icon(
    id: &'static str,
    svg_path: &'static str,
    selected: bool,
    which: Activity,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    div()
        .id(SharedString::from(id))
        .w_full()
        .h(px(48.0))
        .flex()
        .items_center()
        .justify_center()
        .relative()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)))
        .when(selected, |d| d.bg(rgba(t.ghost_active)))
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(6.0))
                .h(px(36.0))
                .w(px(2.0))
                .bg(rgba(if selected { t.icon_accent } else { t.panel })),
        )
        .child(
            svg()
                .path(svg_path)
                .w(px(22.0))
                .h(px(22.0))
                .text_color(rgba(if selected { t.icon_accent } else { t.icon_muted })),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_activity(which, cx)))
}

fn activity_static_icon(
    id: &'static str,
    svg_path: &'static str,
    t: &Colors,
    action: impl Fn(&mut Workspace, &gpui::ClickEvent, &mut Window, &mut Context<Workspace>) + 'static,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    div()
        .id(SharedString::from(id))
        .w_full()
        .h(px(48.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(rgba(t.icon_muted))
        .hover(|s| s.bg(rgba(t.ghost_hover)).text_color(rgba(t.text)))
        .child(
            svg()
                .path(svg_path)
                .w(px(22.0))
                .h(px(22.0)),
        )
        .on_click(cx.listener(action))
}

fn mock_input(text: &'static str, h: f32, t: &Colors) -> gpui::Div {
    div()
        .h(px(h))
        .px(px(8.0))
        .flex()
        .items_center()
        .bg(rgba(t.element_bg))
        .border_1()
        .border_color(rgba(t.border))
        .rounded(px(4.0))
        .text_size(px(12.0))
        .text_color(rgba(t.text_muted))
        .child(SharedString::from(text))
}

fn render_search_panel(t: &Colors) -> gpui::AnyElement {
    div()
        .w(px(260.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(rgba(t.panel))
        .border_r_1()
        .border_color(rgba(t.border_variant))
        .child(panel_header("SEARCH", t))
        .child(
            div()
                .px(px(12.0))
                .py(px(8.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(mock_input("Search (files, symbols)", 26.0, t))
                .child(mock_input("Replace", 26.0, t))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(search_filter_badge("Aa", t))
                        .child(search_filter_badge("Ab", t))
                        .child(search_filter_badge(".*", t)),
                ),
        )
        .child(
            div()
                .px(px(16.0))
                .pt(px(16.0))
                .text_size(px(12.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from("Press Ctrl+F inside the editor to search active file")),
        )
        .into_any_element()
}

fn search_filter_badge(label: &'static str, t: &Colors) -> impl IntoElement {
    div()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(3.0))
        .bg(rgba(t.element_active))
        .hover(|s| s.bg(rgba(t.border_focused)))
        .cursor_pointer()
        .text_size(px(11.0))
        .text_color(rgba(t.text))
        .child(SharedString::from(label))
}

fn section_strip(label: &'static str, t: &Colors) -> gpui::Div {
    div()
        .h(px(22.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .justify_between()
        .bg(rgba(t.surface))
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgba(t.text))
        .child(SharedString::from(label))
}

fn render_git_panel(t: &Colors) -> gpui::AnyElement {
    div()
        .w(px(260.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(rgba(t.panel))
        .border_r_1()
        .border_color(rgba(t.border_variant))
        .overflow_hidden()
        .child(panel_header("SOURCE CONTROL", t))
        .child(
            div()
                .px(px(12.0))
                .py(px(8.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(mock_input("Message (Ctrl+Enter to commit)", 52.0, t))
                .child(
                    div()
                        .h(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgba(t.border_focused))
                        .hover(|s| s.bg(rgba(t.icon_accent)))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .text_color(rgba(t.background))
                        .child(SharedString::from("✓ Commit")),
                ),
        )
        .child(section_strip("CHANGES", t).child(
            div()
                .px(px(6.0))
                .rounded_full()
                .bg(rgba(t.element_active))
                .text_size(px(10.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from("2")),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .child(git_change_row("Cargo.toml", "M", t.vc_modified, t))
                .child(git_change_row("app/src/main.rs", "M", t.vc_modified, t)),
        )
        .child(
            section_strip("COMMITS / TIMELINE", t),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(commit_row("Optimize editor performance", "e1cec16", "2h ago", t))
                .child(commit_row("Replace HTML compiler with native GPUI", "2a8930e", "1d ago", t))
                .child(commit_row("Update compiler and templates", "c3bbae9", "3d ago", t)),
        )
        .into_any_element()
}

fn git_change_row(filename: &'static str, status_letter: &'static str, color: u32, t: &Colors) -> impl IntoElement {
    div()
        .h(px(22.0))
        .px(px(16.0))
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(t.text))
                .child(SharedString::from(filename)),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgba(color))
                .child(SharedString::from(status_letter)),
        )
}

fn commit_row(message: &'static str, hash: &'static str, time: &'static str, t: &Colors) -> impl IntoElement {
    div()
        .p(px(8.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .border_b_1()
        .border_color(rgba(t.border_variant))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(t.text))
                .child(SharedString::from(message)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(hash))
                .child(SharedString::from(time)),
        )
}

fn render_extensions_panel(t: &Colors) -> gpui::AnyElement {
    div()
        .w(px(260.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(rgba(t.panel))
        .border_r_1()
        .border_color(rgba(t.border_variant))
        .child(panel_header("EXTENSIONS", t))
        .child(
            div()
                .px(px(12.0))
                .py(px(8.0))
                .child(mock_input("Search Extensions in Marketplace", 26.0, t)),
        )
        .child(section_strip("INSTALLED", t))
        .child(
            div()
                .flex()
                .flex_col()
                .child(extension_item(
                    "Rust Analyzer",
                    "rust-lang.rust-analyzer",
                    "Rust language support",
                    true,
                    t,
                ))
                .child(extension_item(
                    "Tree-sitter Syntax",
                    "gpui.treesitter",
                    "High performance syntax coloring",
                    true,
                    t,
                ))
                .child(extension_item(
                    "HTML to GPUI Preview",
                    "olova.html2gpui",
                    "Live GPUI element previewer",
                    true,
                    t,
                )),
        )
        .into_any_element()
}

fn extension_item(
    name: &'static str,
    author: &'static str,
    desc: &'static str,
    installed: bool,
    t: &Colors,
) -> impl IntoElement {
    div()
        .p(px(8.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .border_b_1()
        .border_color(rgba(t.border_variant))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgba(t.text))
                        .child(SharedString::from(name)),
                )
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(1.0))
                        .rounded(px(3.0))
                        .bg(rgba(if installed { t.element_active } else { t.border_focused }))
                        .text_size(px(10.0))
                        .text_color(rgba(if installed { t.text_muted } else { t.background }))
                        .child(SharedString::from(if installed { "Installed" } else { "Install" })),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(author)),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(desc)),
        )
}

fn render_tree(
    nodes: &[TreeNode],
    open: Option<&PathBuf>,
    scroll_y: f32,
    folder: &str,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> gpui::AnyElement {
    let mut rows: Vec<(&TreeNode, usize)> = Vec::new();
    fn walk<'a>(nodes: &'a [TreeNode], depth: usize, out: &mut Vec<(&'a TreeNode, usize)>) {
        for n in nodes {
            out.push((n, depth));
            if n.is_dir && n.expanded {
                walk(&n.children, depth + 1, out);
            }
        }
    }
    walk(nodes, 0, &mut rows);

    let mut col = div()
        .w(px(260.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(rgba(t.panel))
        .border_r_1()
        .border_color(rgba(t.border_variant))
        .overflow_hidden()
        .on_scroll_wheel(cx.listener(move |v, event: &gpui::ScrollWheelEvent, _, cx| {
            let delta = match event.delta {
                gpui::ScrollDelta::Lines(l) => l.y * 22.0,
                gpui::ScrollDelta::Pixels(p) => {
                    let f: f32 = p.y.into();
                    f
                }
            };
            v.tree_scroll = (v.tree_scroll + delta).min(0.0);
            cx.notify();
        }));

    col = col
        .child(panel_header("EXPLORER", t))
        .child(
            div()
                .h(px(22.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgba(t.text))
                .child(icon_img(file_icons::FOLDER_EXPANDED, 14.0))
                .child(SharedString::from(folder.to_uppercase())),
        );

    let mut list = div().flex().flex_col().mt(px(scroll_y));
    for (i, (node, depth)) in rows.into_iter().enumerate() {
        if i > 400 {
            break;
        }
        const ICON_SIZE: f32 = 16.0;
        const CHEVRON_SIZE: f32 = 12.0;
        let selected = open.is_some_and(|p| p == &node.path);
        let path = node.path.clone();
        let is_dir = node.is_dir;
        let expanded = node.expanded;
        let name = node.name.clone();
        let pad = 8.0 + depth as f32 * 12.0;
        let mut row = div()
            .id(SharedString::from(format!("t{}", path.display())))
            .w_full()
            .h(px(22.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .pl(px(pad))
            .pr(px(8.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(t.ghost_hover)));
        if selected {
            row = row.bg(rgba(t.element_selected));
        }
        row = if is_dir {
            let (chevron, folder_icon) = if expanded {
                (
                    file_icons::CHEVRON_EXPANDED,
                    file_icons::FOLDER_EXPANDED,
                )
            } else {
                (
                    file_icons::CHEVRON_COLLAPSED,
                    file_icons::FOLDER_COLLAPSED,
                )
            };
            row.child(icon_img(chevron, CHEVRON_SIZE))
                .child(icon_img(folder_icon, ICON_SIZE))
        } else {
            let icon = file_icons::icon_for(&node.path);
            // align files under folder names (indent past the chevron column)
            row.child(div().flex_none().w(px(CHEVRON_SIZE)))
                .child(icon_img(icon, ICON_SIZE))
        };
        row = row
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(SharedString::from(name)),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                if is_dir {
                    this.toggle_dir(&path, cx);
                } else {
                    this.open_file(path.clone(), window, cx);
                }
            }));
        list = list.child(row);
    }
    col.child(list).into_any_element()
}

const SKIP: &[&str] = &["target", "node_modules", "dist", ".git"];

fn load_dir(dir: &Path) -> Vec<TreeNode> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| {
        let p = e.path();
        (!p.is_dir(), e.file_name())
    });
    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || SKIP.iter().any(|s| *s == name) {
            continue;
        }
        let is_dir = path.is_dir();
        out.push(TreeNode {
            name,
            path,
            is_dir,
            expanded: false,
            children: Vec::new(),
        });
    }
    out
}

fn display_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

/// Register every embedded TTF under `assets/fonts/` with GPUI's text
/// system — same approach as Zed's `load_embedded_fonts`.
fn load_embedded_fonts(cx: &App) {
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = AppAssets::iter()
        .filter(|p| p.starts_with("fonts/") && p.ends_with(".ttf"))
        .filter_map(|p| AppAssets::get(&p).map(|f| f.data))
        .collect();
    let count = fonts.len();
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        eprintln!("failed to register embedded fonts: {e}");
    } else {
        println!("registered {count} embedded font files");
    }
}

/// Point the widget library at Zed's fonts (called after every
/// `Theme::change`, which restores families from its built-in config).
fn sync_component_fonts(cx: &mut App) {
    let theme = gpui_component::Theme::global_mut(cx);
    theme.font_family = SANS_FONT.into();
    theme.mono_font_family = MONO_FONT.into();
    theme.font_size = px(14.0);
    theme.mono_font_size = px(13.0);
}

fn main() {
    Application::new()
        .with_assets(CombinedAssets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            cx.bind_keys([
                KeyBinding::new("ctrl-s", Save, None),
                KeyBinding::new("cmd-s", Save, None),
                KeyBinding::new("ctrl-n", NewFile, None),
                KeyBinding::new("cmd-n", NewFile, None),
                KeyBinding::new("ctrl-o", OpenFile, None),
                KeyBinding::new("cmd-o", OpenFile, None),
                KeyBinding::new("ctrl-`", ToggleTerminal, None),
                KeyBinding::new("ctrl-b", ToggleSidebar, None),
                KeyBinding::new("ctrl-shift-e", ShowExplorer, None),
                KeyBinding::new("ctrl-shift-f", ShowSearch, None),
                KeyBinding::new("ctrl-shift-g", ShowGit, None),
                KeyBinding::new("ctrl-shift-x", ShowExtensions, None),
            ]);

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
