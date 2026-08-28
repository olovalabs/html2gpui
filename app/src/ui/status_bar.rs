//! Bottom status bar: transient messages on the left, theme/encoding info
//! on the right.

use gpui::{div, prelude::*, px, rgba, svg, IntoElement, SharedString};

use crate::theme::Colors;

pub(crate) fn render_status_bar(
    status: &str,
    theme_name: &str,
    t: &Colors,
) -> impl IntoElement {
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
        .text_size(px(12.5))
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
                .child(
                    svg()
                        .path("ui_icons/settings-gear_tint.svg")
                        .w(px(13.0))
                        .h(px(13.0))
                        .text_color(rgba(t.text)),
                )
                .child(SharedString::from(theme_name))
                .child(SharedString::from("UTF-8"))
                .child(SharedString::from("Spaces: 4")),
        )
}
