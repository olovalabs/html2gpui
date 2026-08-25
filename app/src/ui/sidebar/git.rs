//! Source Control panel placeholder: commit box plus mock changes/history.

use gpui::{div, prelude::*, px, rgba, AnyElement, Div, FontWeight, IntoElement, SharedString};

use crate::theme::Colors;
use crate::ui::common::{mock_input, panel_header, section_strip};

pub(crate) fn render_git_panel(t: &Colors) -> AnyElement {
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
        .child(commit_box(t))
        .child(
            section_strip("CHANGES", t).child(change_count_badge("2", t)),
        )
        .child(
            div().flex().flex_col().child(
                change_row("Cargo.toml", "M", t.vc_modified, t),
            ),
        )
        .child(
            div().flex().flex_col().child(
                change_row("app/src/main.rs", "M", t.vc_modified, t),
            ),
        )
        .child(section_strip("COMMITS / TIMELINE", t))
        .child(
            div()
                .flex()
                .flex_col()
                .child(commit_row("Optimize editor performance", "e1cec16", "2h ago", t))
                .child(commit_row(
                    "Replace HTML compiler with native GPUI",
                    "2a8930e",
                    "1d ago",
                    t,
                ))
                .child(commit_row("Update compiler and templates", "c3bbae9", "3d ago", t)),
        )
        .into_any_element()
}

fn commit_box(t: &Colors) -> Div {
    div()
        .px(px(12.0))
        .py(px(8.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(mock_input("Message (Ctrl+Enter to commit)", 52.0, t))
        .child(commit_button(t))
}

fn commit_button(t: &Colors) -> impl IntoElement {
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
        .child(SharedString::from("✓ Commit"))
}

fn change_count_badge(count: &'static str, t: &Colors) -> Div {
    div()
        .px(px(6.0))
        .rounded_full()
        .bg(rgba(t.element_active))
        .text_size(px(10.0))
        .text_color(rgba(t.text_muted))
        .child(SharedString::from(count))
}

/// One pending file in the CHANGES list ("M" = modified).
fn change_row(
    filename: &'static str,
    status_letter: &'static str,
    color: u32,
    t: &Colors,
) -> impl IntoElement {
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

/// One entry of the COMMITS / TIMELINE list.
fn commit_row(message: &'static str, hash: &'static str, time: &'static str, t: &Colors) -> Div {
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
