//! Bottom terminal panel placeholder (not yet wired to a shell).

use gpui::{div, prelude::*, px, rgba, FontWeight, IntoElement, SharedString};

use crate::assets::MONO_FONT;
use crate::theme::Colors;

pub(crate) fn render_terminal(t: &Colors) -> impl IntoElement {
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
