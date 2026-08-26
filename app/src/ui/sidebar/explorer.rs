//! Explorer panel: Zed/VS Code-style minimalist folder tree with continuous
//! vertical tree lines (indent guides), file icons, inline creation, and context menus.

use std::path::{Path, PathBuf};

use gpui::{
    div, prelude::*, px, rgba, svg, AnyElement, Context, FontWeight, IntoElement, SharedString,
};
use gpui_component::{
    input::Input, menu::ContextMenuExt, scroll::ScrollableElement as _, Sizable,
};

use crate::actions::{
    ExplorerCollapseAll, ExplorerCopyPath, ExplorerCopyRelativePath, ExplorerDelete,
    ExplorerNewFile, ExplorerNewFolder, ExplorerRefresh, ExplorerRename, ExplorerRevealInFinder,
    OpenFolder,
};
use crate::file_icons;
use crate::fs_tree::TreeNode;
use crate::theme::Colors;
use crate::ui::common::icon_img;
use crate::workspace::{CreatingKind, InlineCreating, Workspace};

const INDENT_STEP: f32 = 14.0;
const BASE_PAD: f32 = 12.0;
const ROW_HEIGHT: f32 = 24.0;
const ICON_SIZE: f32 = 16.0;

pub(crate) fn render_tree(
    nodes: &[TreeNode],
    root_path: Option<&Path>,
    open: Option<&PathBuf>,
    selected_path: Option<&PathBuf>,
    section_expanded: bool,
    inline_creating: Option<&InlineCreating>,
    folder: &str,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    // Flatten the tree into visible rows (expanded dirs only).
    let mut rows: Vec<(&TreeNode, usize)> = Vec::new();
    fn walk<'a>(nodes: &'a [TreeNode], depth: usize, out: &mut Vec<(&'a TreeNode, usize)>) {
        for n in nodes {
            out.push((n, depth));
            if n.is_dir && n.expanded {
                walk(&n.children, depth + 1, out);
            }
        }
    }
    if section_expanded {
        walk(nodes, 0, &mut rows);
    }

    let mut col = div()
        .w(px(260.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(rgba(t.panel))
        .border_r_1()
        .border_color(rgba(t.border_variant))
        .overflow_hidden();

    // Clean, minimalist root project header (e.g. `[folder] foodime`)
    let root_folder_icon = if section_expanded {
        file_icons::FOLDER_EXPANDED
    } else {
        file_icons::FOLDER_COLLAPSED
    };
    let root_chevron = if section_expanded {
        "ui_icons/chevron-down_tint.svg"
    } else {
        "ui_icons/chevron-right_tint.svg"
    };

    let root_path_buf = root_path.map(|p| p.to_path_buf());
    let r_clone = root_path_buf.clone();

    col = col.child(
        div()
            .id("exp-root-header")
            .h(px(28.0))
            .px(px(BASE_PAD))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .hover(|s| s.bg(rgba(t.ghost_hover)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_explorer_section(cx);
            }))
            .context_menu(move |menu, _window, _cx| {
                let p1 = r_clone.clone();
                let p2 = r_clone.clone();
                let p3 = r_clone.clone();
                let p4 = r_clone.clone();
                menu.menu("New File…", Box::new(ExplorerNewFile { parent: p1 }))
                    .menu("New Folder…", Box::new(ExplorerNewFolder { parent: p2 }))
                    .separator()
                    .menu("Refresh Explorer", Box::new(ExplorerRefresh))
                    .menu("Collapse All Folders", Box::new(ExplorerCollapseAll))
                    .separator()
                    .when(p3.is_some(), |m| {
                        m.menu(
                            "Reveal in File Explorer",
                            Box::new(ExplorerRevealInFinder {
                                path: p3.unwrap(),
                            }),
                        )
                    })
                    .when(p4.is_some(), |m| {
                        m.menu(
                            "Copy Path",
                            Box::new(ExplorerCopyPath {
                                path: p4.unwrap(),
                            }),
                        )
                    })
                    .separator()
                    .menu("Open Folder…", Box::new(OpenFolder))
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_none()
                            .child(
                                svg()
                                    .path(root_chevron)
                                    .w(px(10.0))
                                    .h(px(10.0))
                                    .text_color(rgba(t.icon_muted)),
                            ),
                    )
                    .child(icon_img(root_folder_icon, ICON_SIZE))
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgba(t.text))
                            .child(SharedString::from(folder.to_lowercase())),
                    ),
            ),
    );

    // Tree rows list with Zed-style vertical tree lines
    if section_expanded {
        let mut list = div()
            .id("explorer-tree-scroll")
            .flex_1()
            .w_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_y_scrollbar();

        // Determine where the inline create row should appear
        let inline_pos: Option<(usize, usize)> = if let Some(creating) = inline_creating {
            if root_path.is_some_and(|r| r == creating.parent_dir) {
                Some((0, 0))
            } else {
                if let Some((idx, (_, parent_depth))) = rows
                    .iter()
                    .enumerate()
                    .find(|(_, (n, _))| n.path == creating.parent_dir)
                {
                    Some((idx + 1, parent_depth + 1))
                } else {
                    Some((0, 0))
                }
            }
        } else {
            None
        };

        let mut inserted_inline = false;
        for (idx, (node, depth)) in rows.into_iter().enumerate() {
            if let Some((inline_idx, inline_depth)) = inline_pos {
                if !inserted_inline && inline_idx == idx {
                    list = list.child(inline_create_row(
                        inline_creating.unwrap(),
                        inline_depth,
                        t,
                        cx,
                    ));
                    inserted_inline = true;
                }
            }
            list = list.child(tree_row(node, depth, open, selected_path, t, cx));
        }

        if let Some((_, inline_depth)) = inline_pos {
            if !inserted_inline {
                list = list.child(inline_create_row(
                    inline_creating.unwrap(),
                    inline_depth,
                    t,
                    cx,
                ));
            }
        }

        col = col.child(list);
    }

    col.into_any_element()
}

fn inline_create_row(
    creating: &InlineCreating,
    depth: usize,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let pad = BASE_PAD + depth as f32 * INDENT_STEP;

    let mut row = div()
        .id("inline-create-row")
        .w_full()
        .h(px(ROW_HEIGHT))
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
            if event.keystroke.key == "escape" {
                this.cancel_inline_create(cx);
            } else if event.keystroke.key == "enter" {
                this.confirm_inline_create(cx);
            }
        }));

    // Zed-style vertical tree lines (indent guides)
    for d in 0..depth {
        let guide_x = BASE_PAD + d as f32 * INDENT_STEP + 3.0;
        row = row.child(
            div()
                .absolute()
                .left(px(guide_x))
                .top(px(0.0))
                .bottom(px(0.0))
                .w(px(1.0))
                .bg(rgba(0xffffff1e)),
        );
    }

    let icon = match creating.kind {
        CreatingKind::File => "file_icons/default_file.svg",
        CreatingKind::Folder => file_icons::FOLDER_COLLAPSED,
    };

    let content = div()
        .w_full()
        .h_full()
        .flex()
        .flex_row()
        .items_center()
        .pl(px(pad))
        .pr(px(10.0))
        .child(
            // Chevron spacer so icon aligns with folder/file icons
            div().w(px(14.0)).h(px(14.0)).flex_none(),
        )
        .child(div().w(px(4.0)).flex_none())
        .child(icon_img(icon, ICON_SIZE))
        .child(div().w(px(6.0)).flex_none())
        .child(
            div()
                .flex_1()
                .h(px(22.0))
                .flex()
                .items_center()
                .bg(rgba(t.background))
                .border_1()
                .border_color(rgba(t.border_focused))
                .rounded(px(4.0))
                .px(px(2.0))
                .child(
                    Input::new(&creating.input)
                        .xsmall()
                        .text_size(px(13.5))
                        .appearance(false)
                        .bordered(false),
                ),
        );

    row.child(content)
}

fn tree_row(
    node: &TreeNode,
    depth: usize,
    open: Option<&PathBuf>,
    selected_path: Option<&PathBuf>,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let is_open = open.is_some_and(|p| p == &node.path);
    let is_selected = selected_path.is_some_and(|p| p == &node.path) || is_open;
    let path = node.path.clone();
    let is_dir = node.is_dir;
    let expanded = node.expanded;
    let name = node.name.clone();
    let pad = BASE_PAD + depth as f32 * INDENT_STEP;

    let mut row = div()
        .id(SharedString::from(format!("t{}", path.display())))
        .w_full()
        .h(px(ROW_HEIGHT))
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)));

    if is_selected {
        row = row.bg(rgba(t.element_selected));
    }

    // Zed-style vertical tree lines (indent guides) for each ancestor depth level
    for d in 0..depth {
        let guide_x = BASE_PAD + d as f32 * INDENT_STEP + 3.0;
        row = row.child(
            div()
                .absolute()
                .left(px(guide_x))
                .top(px(0.0))
                .bottom(px(0.0))
                .w(px(1.0))
                .bg(rgba(0xffffff1e)),
        );
    }

    let icon_path = if is_dir {
        file_icons::folder_icon_for(&node.path, expanded)
    } else {
        file_icons::icon_for(&node.path)
    };

    let text_color = if is_selected {
        0xffffffff
    } else {
        t.text
    };

    let chevron_element = if is_dir {
        let chev_path = if expanded {
            "ui_icons/chevron-down_tint.svg"
        } else {
            "ui_icons/chevron-right_tint.svg"
        };
        div()
            .w(px(14.0))
            .h(px(14.0))
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .child(
                svg()
                    .path(chev_path)
                    .w(px(10.0))
                    .h(px(10.0))
                    .text_color(rgba(t.icon_muted)),
            )
    } else {
        div().w(px(14.0)).h(px(14.0)).flex_none()
    };

    let content = div()
        .w_full()
        .h_full()
        .flex()
        .flex_row()
        .items_center()
        .pl(px(pad))
        .pr(px(8.0))
        .child(chevron_element)
        .child(div().w(px(4.0)).flex_none())
        .child(icon_img(icon_path, ICON_SIZE))
        .child(div().w(px(6.0)).flex_none())
        .child(
            div()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .text_size(px(13.5))
                .text_color(rgba(text_color))
                .child(SharedString::from(name)),
        );

    let path_click = path.clone();
    row = row
        .child(content)
        .on_click(cx.listener(move |this, _, window, cx| {
            if is_dir {
                this.toggle_dir(&path_click, cx);
            } else {
                this.open_file(path_click.clone(), window, cx);
            }
        }));

    // Right-click context menu (Zed / VS Code File Explorer Context Menu)
    let path_c1 = path.clone();
    let path_c2 = path.clone();
    let path_c3 = path.clone();
    let path_c4 = path.clone();
    let path_c5 = path.clone();
    let parent_for_new = if is_dir {
        Some(path.clone())
    } else {
        path.parent().map(|p| p.to_path_buf())
    };

    row.context_menu(move |menu, _window, _cx| {
        let p_new1 = parent_for_new.clone();
        let p_new2 = parent_for_new.clone();
        menu.menu("New File…", Box::new(ExplorerNewFile { parent: p_new1 }))
            .menu("New Folder…", Box::new(ExplorerNewFolder { parent: p_new2 }))
            .separator()
            .menu("Reveal in File Explorer", Box::new(ExplorerRevealInFinder { path: path_c1.clone() }))
            .separator()
            .menu("Copy Path", Box::new(ExplorerCopyPath { path: path_c2.clone() }))
            .menu("Copy Relative Path", Box::new(ExplorerCopyRelativePath { path: path_c3.clone() }))
            .separator()
            .menu("Rename…", Box::new(ExplorerRename { path: path_c4.clone() }))
            .menu("Delete", Box::new(ExplorerDelete { path: path_c5.clone() }))
    })
}
