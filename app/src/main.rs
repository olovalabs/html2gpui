use std::path::{Path, PathBuf};

use gpui::{
    actions, div, prelude::*, px, rgb, size, App, Application, Bounds, Context,
    Entity, FontWeight, KeyBinding, SharedString, Window, WindowBounds, WindowDecorations,
    WindowOptions,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    input::{Copy, Cut, Input, InputEvent, InputState, Paste, Redo, SelectAll, TabSize, Undo},
    menu::DropdownMenu as _,
    Icon, IconName, Root, TitleBar,
};

actions!(
    editor,
    [
        Save,
        Quit,
        ShowExplorer,
        ShowGit,
        ShowExtensions,
        ToggleSidebar,
        ToggleTerminal,
        About
    ]
);

const BG: u32 = 0x1e1e1e;
const ACTIVITY: u32 = 0x333333;
const SIDEBAR: u32 = 0x252526;
const SIDEBAR_HOVER: u32 = 0x2a2d2e;
const ACTIVE: u32 = 0x37373d;
const TITLE: u32 = 0x3c3c3c;
const STATUS: u32 = 0x007acc;
const TEXT: u32 = 0xcccccc;
const MUTED: u32 = 0x858585;
const ICON: u32 = 0x858585;
const ICON_ON: u32 = 0xffffff;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Activity {
    Explorer,
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
    root: PathBuf,
    tree: Vec<TreeNode>,
    open: Option<PathBuf>,
    dirty: bool,
    status: String,
    tree_scroll: f32,
    activity: Activity,
    show_sidebar: bool,
    show_terminal: bool,
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

        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            tree: load_dir(&root),
            root,
            open: None,
            dirty: false,
            status: "Explorer · Ctrl+S save · Ctrl+F search".into(),
            tree_scroll: 0.0,
            activity: Activity::Explorer,
            show_sidebar: true,
            show_terminal: false,
            editor,
        }
    }
    
  

    fn set_activity(&mut self, activity: Activity, cx: &mut Context<Self>) {
        self.activity = activity;
        self.status = match activity {
            Activity::Explorer => "EXPLORER".into(),
            Activity::Git => "Source Control (coming soon)".into(),
            Activity::Extensions => "Extensions (coming soon)".into(),
        };
        cx.notify();
    }

    fn language_for(path: &Path) -> Option<&'static str> {
        Some(match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
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
            _ => return None,
        })
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
                self.dirty = false;
                self.status = if highlight {
                    path.display().to_string()
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
        let Some(path) = self.open.clone() else {
            self.status = "No file open".into();
            cx.notify();
            return;
        };
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

    fn about(&mut self, cx: &mut Context<Self>) {
        self.status = "gpui editor — File / Edit / View / Terminal / Help".into();
        cx.notify();
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match &self.open {
            Some(p) => {
                let star = if self.dirty { " ●" } else { "" };
                format!("{}{star} — {}", display_name(p), display_name(&self.root))
            }
            None => display_name(&self.root),
        };
        let tree = &self.tree;
        let open = self.open.clone();
        let tree_scroll = self.tree_scroll;
        let status = self.status.clone();
        let editor = self.editor.clone();
        let activity = self.activity;
        let folder = display_name(&self.root);
        let show_sidebar = self.show_sidebar;
        let show_terminal = self.show_terminal;

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .text_size(px(13.0))
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save(window, cx)))
            .on_action(cx.listener(|this, _: &Quit, _, cx| this.quit(cx)))
            .on_action(cx.listener(|this, _: &ShowExplorer, _, cx| {
                this.show_sidebar = true;
                this.set_activity(Activity::Explorer, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowGit, _, cx| {
                this.show_sidebar = true;
                this.set_activity(Activity::Git, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowExtensions, _, cx| {
                this.show_sidebar = true;
                this.set_activity(Activity::Extensions, cx);
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
            .on_action(cx.listener(|this, _: &About, _, cx| this.about(cx)))
            .child(render_titlebar(&title))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .child(render_activity_bar(activity, cx))
                    .when(show_sidebar, |d| {
                        d.child(match activity {
                            Activity::Explorer => {
                                render_tree(tree, open.as_ref(), tree_scroll, &folder, cx)
                            }
                            Activity::Git => placeholder_side(
                                "SOURCE CONTROL",
                                "Git view is not connected yet.",
                            ),
                            Activity::Extensions => placeholder_side(
                                "EXTENSIONS",
                                "Extensions view is not connected yet.",
                            ),
                        })
                    })
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .bg(rgb(BG))
                            .child(
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
                            .when(show_terminal, |d| d.child(render_terminal())),
                    ),
            )
            .child(
                div()
                    .h(px(22.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(10.0))
                    .bg(rgb(STATUS))
                    .text_color(rgb(0xffffff))
                    .text_size(px(12.0))
                    .child(SharedString::from(status)),
            )
    }
}

fn render_titlebar(title: &str) -> impl IntoElement {
    TitleBar::new().bg(rgb(TITLE)).border_color(rgb(0x2d2d2d)).child(
        div()
            .w_full()
            .h_full()
            .flex()
            .flex_row()
            .items_center()
            .text_size(px(13.0))
            .child(menu_btn("m-file", "File", |menu, _, _| {
                menu.menu("Save", Box::new(Save))
                    .separator()
                    .menu("Exit", Box::new(Quit))
            }))
            .child(menu_btn("m-edit", "Edit", |menu, _, _| {
                menu.menu("Undo", Box::new(Undo))
                    .menu("Redo", Box::new(Redo))
                    .separator()
                    .menu("Cut", Box::new(Cut))
                    .menu("Copy", Box::new(Copy))
                    .menu("Paste", Box::new(Paste))
                    .separator()
                    .menu("Select All", Box::new(SelectAll))
            }))
            .child(menu_btn("m-view", "View", |menu, _, _| {
                menu.menu("Explorer", Box::new(ShowExplorer))
                    .menu("Source Control", Box::new(ShowGit))
                    .menu("Extensions", Box::new(ShowExtensions))
                    .separator()
                    .menu("Toggle Primary Side Bar", Box::new(ToggleSidebar))
                    .menu("Toggle Terminal", Box::new(ToggleTerminal))
            }))
            .child(menu_btn("m-term", "Terminal", |menu, _, _| {
                menu.menu("New Terminal", Box::new(ToggleTerminal))
            }))
            .child(menu_btn("m-help", "Help", |menu, _, _| {
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
                    .text_color(rgb(TEXT))
                    .child(SharedString::from(title.to_string())),
            ),
    )
}

fn menu_btn(
    id: &'static str,
    label: &'static str,
    build: impl Fn(
            gpui_component::menu::PopupMenu,
            &mut Window,
            &mut Context<gpui_component::menu::PopupMenu>,
        ) -> gpui_component::menu::PopupMenu
        + 'static,
) -> impl IntoElement {
    Button::new(id)
        .ghost()
        .compact()
        .label(label)
        .text_color(rgb(TEXT))
        .dropdown_menu(build)
}

fn render_terminal() -> impl IntoElement {
    div()
        .h(px(160.0))
        .w_full()
        .flex()
        .flex_col()
        .bg(rgb(0x1e1e1e))
        .border_t_1()
        .border_color(rgb(0x3c3c3c))
        .child(
            div()
                .h(px(28.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .bg(rgb(SIDEBAR))
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MUTED))
                .child(SharedString::from("TERMINAL")),
        )
        .child(
            div()
                .flex_1()
                .p(px(10.0))
                .text_size(px(12.0))
                .text_color(rgb(0xcccccc))
                .child(SharedString::from(
                    "Terminal panel — shell is not connected yet. View → Toggle Terminal to hide.",
                )),
        )
}

fn render_activity_bar(activity: Activity, cx: &mut Context<Workspace>) -> impl IntoElement {
    div()
        .w(px(48.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(ACTIVITY))
        .child(activity_icon(
            "act-explorer",
            IconName::Folder,
            activity == Activity::Explorer,
            Activity::Explorer,
            cx,
        ))
        .child(activity_icon(
            "act-git",
            IconName::GitHub,
            activity == Activity::Git,
            Activity::Git,
            cx,
        ))
        .child(activity_icon(
            "act-ext",
            IconName::LayoutDashboard,
            activity == Activity::Extensions,
            Activity::Extensions,
            cx,
        ))
}

fn activity_icon(
    id: &'static str,
    icon: IconName,
    selected: bool,
    which: Activity,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let color = if selected { ICON_ON } else { ICON };
    div()
        .id(SharedString::from(id))
        .w_full()
        .h(px(48.0))
        .flex()
        .items_center()
        .justify_center()
        .relative()
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x2c2c2c)))
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(10.0))
                .h(px(28.0))
                .w(px(2.0))
                .bg(rgb(if selected { ICON_ON } else { ACTIVITY })),
        )
        .child(Icon::new(icon).size(px(22.0)).text_color(rgb(color)))
        .on_click(cx.listener(move |this, _, _, cx| this.set_activity(which, cx)))
}

fn placeholder_side(title: &'static str, body: &'static str) -> gpui::AnyElement {
    div()
        .w(px(260.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(SIDEBAR))
        .child(
            div()
                .h(px(35.0))
                .px(px(16.0))
                .flex()
                .items_center()
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MUTED))
                .child(SharedString::from(title)),
        )
        .child(
            div()
                .px(px(16.0))
                .pt(px(8.0))
                .text_size(px(13.0))
                .text_color(rgb(MUTED))
                .child(SharedString::from(body)),
        )
        .into_any_element()
}

fn render_tree(
    nodes: &[TreeNode],
    open: Option<&PathBuf>,
    scroll_y: f32,
    folder: &str,
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
        .bg(rgb(SIDEBAR))
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
        .child(
            div()
                .h(px(35.0))
                .px(px(16.0))
                .flex()
                .items_center()
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(MUTED))
                .child(SharedString::from("EXPLORER")),
        )
        .child(
            div()
                .h(px(22.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(TEXT))
                .child(SharedString::from(format!("▾ {}", folder.to_uppercase()))),
        );

    let mut list = div().flex().flex_col().mt(px(scroll_y));
    for (i, (node, depth)) in rows.into_iter().enumerate() {
        if i > 400 {
            break;
        }
        let selected = open.is_some_and(|p| p == &node.path);
        let path = node.path.clone();
        let is_dir = node.is_dir;
        let pad = 8.0 + depth as f32 * 12.0;
        let label = if is_dir {
            format!("{} {}", if node.expanded { "▾" } else { "▸" }, node.name)
        } else {
            format!("    {}", node.name)
        };
        let mut row = div()
            .id(SharedString::from(format!("t{}", path.display())))
            .w_full()
            .h(px(22.0))
            .flex()
            .items_center()
            .pl(px(pad))
            .pr(px(8.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(SIDEBAR_HOVER)));
        if selected {
            row = row.bg(rgb(ACTIVE));
        }
        row = row
            .child(SharedString::from(label))
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

fn main() {
    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.bind_keys([
            KeyBinding::new("ctrl-s", Save, None),
            KeyBinding::new("cmd-s", Save, None),
            KeyBinding::new("ctrl-`", ToggleTerminal, None),
            KeyBinding::new("ctrl-b", ToggleSidebar, None),
            KeyBinding::new("ctrl-shift-e", ShowExplorer, None),
            KeyBinding::new("ctrl-shift-g", ShowGit, None),
            KeyBinding::new("ctrl-shift-x", ShowExtensions, None),
        ]);

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
