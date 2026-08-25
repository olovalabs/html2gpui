//! Explorer panel: the scrollable folder tree with file icons.

use std::path::PathBuf;

use gpui::{
    div, prelude::*, px, rgba, AnyElement, Context, FontWeight, ScrollDelta, ScrollWheelEvent,
    SharedString,
};

use crate::file_icons;
use crate::fs_tree::TreeNode;
use crate::theme::Colors;
use crate::ui::common::{icon_img, panel_header};
use crate::workspace::Workspace;

pub(crate) fn render_tree(
    nodes: &[TreeNode],
    open: Option<&PathBuf>,
    scroll_y: f32,
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
        .on_scroll_wheel(cx.listener(move |v, event: &ScrollWheelEvent, _, cx| {
            let delta = match event.delta {
                ScrollDelta::Lines(l) => l.y * 22.0,
                ScrollDelta::Pixels(p) => {
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
        list = list.child(tree_row(node, depth, open, t, cx));
    }
    col.child(list).into_any_element()
}

fn tree_row(
    node: &TreeNode,
    depth: usize,
    open: Option<&PathBuf>,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
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
    row
}
