//! Git diff view — the "click a changed file → see the diff" experience.
//!
//! Renders the unified diff produced by `git diff` with line numbers and
//! add/remove coloring (green = added, red = removed, blue = hunk headers),
//! plus a small toolbar with Open File / Discard Changes actions.

use gpui::{div, prelude::*, px, rgba, AnyElement, Context, FontWeight, IntoElement, SharedString};
use gpui_component::scroll::ScrollableElement as _;

use crate::actions::GitDiscardFile;
use crate::assets;
use crate::git::{parse_diff, DiffLine, DiffLineKind};
use crate::theme::Colors;
use crate::workspace::{DiffTab, Workspace};

/// Cap on rendered diff lines (huge generated diffs stay responsive).
const MAX_RENDERED_LINES: usize = 12_000;

/// Replace the alpha of an RGBA8 color.
fn with_alpha(color: u32, alpha: u8) -> u32 {
    (color & 0xFFFF_FF00) | u32::from(alpha)
}

pub(crate) fn render_diff_view(
    diff: &DiffTab,
    font_size: f32,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    let file_name = diff
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| diff.rel.clone());
    let path = diff.path.clone();

    let label = if diff.staged {
        format!("{file_name} — STAGED")
    } else {
        format!("{file_name} — WORKING TREE")
    };

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(rgba(t.editor_bg))
        .child(
            // Toolbar
            div()
                .h(px(38.0))
                .w_full()
                .px(px(14.0))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .bg(rgba(t.toolbar))
                .border_b_1()
                .border_color(rgba(t.border_variant))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgba(t.text))
                                .child(SharedString::from(file_name)),
                        )
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded_full()
                                .bg(rgba(if diff.staged { t.vc_added } else { t.vc_modified }))
                                .text_size(px(9.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgba(t.background))
                                .child(SharedString::from(label)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(tool_button("Open File", t, cx, {
                            let path = path.clone();
                            move |this, window, cx| {
                                this.open_file(path, window, cx);
                            }
                        }))
                        .child(tool_button("Discard Changes", t, cx, {
                            let path = path.clone();
                            move |_this, window, cx| {
                                window.dispatch_action(
                                    Box::new(GitDiscardFile { path }),
                                    cx,
                                );
                            }
                        })),
                ),
        )
        .child(diff_body(diff, font_size, t))
        .into_any_element()
}

fn tool_button(
    label: &'static str,
    t: &Colors,
    cx: &mut Context<Workspace>,
    on_click: impl Fn(&mut Workspace, &mut gpui::Window, &mut Context<Workspace>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("diff-tool-{label}")))
        .h(px(26.0))
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(t.element_bg))
        .border_1()
        .border_color(rgba(t.border))
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.element_hover)))
        .text_size(px(12.0))
        .text_color(rgba(t.text))
        .child(SharedString::from(label))
        .on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx)))
}

fn diff_body(diff: &DiffTab, font_size: f32, t: &Colors) -> impl IntoElement {
    let line_height = font_size * 1.45;

    let Some(text) = diff.text.as_ref() else {
        return div()
            .flex_1()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(rgba(t.text_muted))
            .child(SharedString::from("Loading diff…"))
            .into_any_element();
    };

    if let Some(error) = &diff.error {
        return div()
            .flex_1()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(rgba(t.vc_deleted))
            .child(SharedString::from(error.clone()))
            .into_any_element();
    }

    if text.trim().is_empty() {
        return div()
            .flex_1()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(rgba(t.text_muted))
            .child(SharedString::from("No changes"))
            .into_any_element();
    }

    let lines = parse_diff(text);
    let truncated = lines.len() > MAX_RENDERED_LINES;
    let iter = lines.into_iter().take(MAX_RENDERED_LINES);

    let mut list = div()
        .id("diff-scroll")
        .flex_1()
        .w_full()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .font_family(assets::MONO_FONT)
        .text_size(px(font_size))
        .overflow_y_scrollbar();

    for line in iter {
        list = list.child(diff_row(&line, line_height, font_size, t));
    }
    if truncated {
        list = list.child(
            div()
                .w_full()
                .h(px(line_height))
                .px(px(12.0))
                .flex()
                .items_center()
                .text_size(px(12.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(format!(
                    "… diff truncated at {MAX_RENDERED_LINES} lines"
                ))),
        );
    }
    list.into_any_element()
}

fn diff_row(line: &DiffLine, line_height: f32, font_size: f32, t: &Colors) -> impl IntoElement {
    let (bg, fg) = match line.kind {
        DiffLineKind::Add => (Some(with_alpha(t.vc_added, 0x24)), t.vc_added),
        DiffLineKind::Remove => (Some(with_alpha(t.vc_deleted, 0x24)), t.vc_deleted),
        DiffLineKind::Hunk => (Some(with_alpha(t.icon_accent, 0x12)), t.text_accent),
        DiffLineKind::Meta => (None, t.text_muted),
        DiffLineKind::NoNewline => (None, t.text_muted),
        DiffLineKind::Context => (None, t.editor_fg),
    };

    div()
        .w_full()
        .h(px(line_height))
        .flex()
        .flex_row()
        .when_some(bg, |d, bg| d.bg(rgba(bg)))
        // Old-line number gutter
        .child(
            div()
                .w(px(font_size * 3.2))
                .flex_none()
                .flex()
                .items_center()
                .justify_end()
                .pr(px(6.0))
                .text_size(px(font_size * 0.82))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(
                    line.old_no.map(|n| n.to_string()).unwrap_or_default(),
                )),
        )
        // New-line number gutter
        .child(
            div()
                .w(px(font_size * 3.2))
                .flex_none()
                .flex()
                .items_center()
                .justify_end()
                .pr(px(6.0))
                .text_size(px(font_size * 0.82))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(
                    line.new_no.map(|n| n.to_string()).unwrap_or_default(),
                )),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .px(px(6.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(font_size))
                .text_color(rgba(fg))
                .child(SharedString::from(line.text.clone())),
        )
}
