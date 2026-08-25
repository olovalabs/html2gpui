//! Explorer panel: VS Code-style folder tree with file icons, indent guides,
//! action buttons, bounded scrolling, and context menus.

use std::path::PathBuf;

use gpui::{
    div, prelude::*, px, rgba, svg, AnyElement, Context, FontWeight, IntoElement, SharedString,
    Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    menu::{ContextMenuExt, DropdownMenu as _},
    scroll::ScrollableElement as _,
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
use crate::workspace::Workspace;

pub(crate) fn render_tree(
    nodes: &[TreeNode],
    open: Option<&PathBuf>,
    selected_path: Option<&PathBuf>,
    section_expanded: bool,
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

    // 1. Top EXPLORER title row with '...' action menu
    col = col.child(
        div()
            .h(px(35.0))
            .px(px(16.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgba(t.text_muted))
                    .child("EXPLORER"),
            )
            .child(
                Button::new("exp-top-more")
                    .ghost()
                    .compact()
                    .text_color(rgba(t.icon_muted))
                    .child(
                        svg()
                            .path("ui_icons/more.svg")
                            .w(px(14.0))
                            .h(px(14.0)),
                    )
                    .dropdown_menu(|menu, _, _| {
                        menu.menu("Refresh Explorer", Box::new(ExplorerRefresh))
                            .menu("Collapse Folders in Explorer", Box::new(ExplorerCollapseAll))
                            .separator()
                            .menu("Open Folder…", Box::new(OpenFolder))
                    }),
            ),
    );

    // 2. Collapsible root project section header (e.g. `v SERA`) with action icons
    let section_chevron = if section_expanded {
        file_icons::CHEVRON_EXPANDED
    } else {
        file_icons::CHEVRON_COLLAPSED
    };

    col = col.child(
        div()
            .id("exp-section-header")
            .h(px(22.0))
            .px(px(8.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .hover(|s| s.bg(rgba(t.ghost_hover)))
            .child(
                div()
                    .id("exp-section-toggle")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_explorer_section(cx);
                    }))
                    .child(
                        svg()
                            .path(section_chevron)
                            .w(px(10.0))
                            .h(px(10.0))
                            .text_color(rgba(t.icon_muted)),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgba(t.text))
                            .child(SharedString::from(folder.to_uppercase())),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .child(header_action_btn(
                        "exp-btn-new-file",
                        "ui_icons/new_file.svg",
                        t,
                        |this, _, window, cx| this.new_file_in_dir(None, window, cx),
                        cx,
                    ))
                    .child(header_action_btn(
                        "exp-btn-new-folder",
                        "ui_icons/new_folder.svg",
                        t,
                        |this, _, _, cx| this.new_folder_in_dir(None, cx),
                        cx,
                    ))
                    .child(header_action_btn(
                        "exp-btn-refresh",
                        "ui_icons/refresh.svg",
                        t,
                        |this, _, _, cx| this.refresh_explorer(cx),
                        cx,
                    ))
                    .child(header_action_btn(
                        "exp-btn-collapse",
                        "ui_icons/collapse_all.svg",
                        t,
                        |this, _, _, cx| this.collapse_all_folders(cx),
                        cx,
                    )),
            ),
    );

    // 3. Tree rows list with bounded VS Code-style scrolling
    if section_expanded {
        let mut list = div()
            .id("explorer-tree-scroll")
            .flex_1()
            .w_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_y_scrollbar();

        for (node, depth) in rows {
            list = list.child(tree_row(node, depth, open, selected_path, t, cx));
        }
        col = col.child(list);
    }

    col.into_any_element()
}

fn header_action_btn(
    id: &'static str,
    icon_path: &'static str,
    t: &Colors,
    on_click: impl Fn(&mut Workspace, &gpui::ClickEvent, &mut Window, &mut Context<Workspace>) + 'static,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    div()
        .id(SharedString::from(id))
        .w(px(20.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.0))
        .cursor_pointer()
        .text_color(rgba(t.icon_muted))
        .hover(|s| s.bg(rgba(t.ghost_hover)).text_color(rgba(t.text)))
        .child(
            svg()
                .path(icon_path)
                .w(px(14.0))
                .h(px(14.0)),
        )
        .on_click(cx.listener(on_click))
}

fn tree_row(
    node: &TreeNode,
    depth: usize,
    open: Option<&PathBuf>,
    selected_path: Option<&PathBuf>,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    const ICON_SIZE: f32 = 16.0;
    const CHEVRON_SIZE: f32 = 10.0;
    const ROW_HEIGHT: f32 = 22.0;

    let is_open = open.is_some_and(|p| p == &node.path);
    let is_selected = selected_path.is_some_and(|p| p == &node.path) || is_open;
    let path = node.path.clone();
    let is_dir = node.is_dir;
    let expanded = node.expanded;
    let name = node.name.clone();
    let pad = 8.0 + depth as f32 * 12.0;

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

    // Render subtle VS Code-style indent guides for each ancestor depth level
    for d in 0..depth {
        let guide_x = 8.0 + d as f32 * 12.0 + 5.0;
        row = row.child(
            div()
                .absolute()
                .left(px(guide_x))
                .top(px(0.0))
                .bottom(px(0.0))
                .w(px(1.0))
                .bg(rgba(0xffffff10)),
        );
    }

    // Row inner content: [Twistie/Spacer] [Icon] [Name]
    let mut content = div()
        .w_full()
        .h_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(pad))
        .pr(px(8.0));

    if is_dir {
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
        content = content
            .child(
                div()
                    .flex_none()
                    .w(px(14.0))
                    .h(px(14.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(chevron)
                            .w(px(CHEVRON_SIZE))
                            .h(px(CHEVRON_SIZE))
                            .text_color(rgba(t.icon_muted)),
                    ),
            )
            .child(icon_img(folder_icon, ICON_SIZE));
    } else {
        let icon = file_icons::icon_for(&node.path);
        // Align file icon with folder icons (blank 14px spacer instead of twistie)
        content = content
            .child(
                div()
                    .flex_none()
                    .w(px(14.0))
                    .h(px(14.0)),
            )
            .child(icon_img(icon, ICON_SIZE));
    }

    let text_color = if is_selected {
        0xffffffff
    } else {
        t.text
    };

    content = content.child(
        div()
            .min_w(px(0.0))
            .overflow_hidden()
            .text_ellipsis()
            .text_size(px(13.0))
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

    // Right-click context menu (VS Code Explorer Context Menu)
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
