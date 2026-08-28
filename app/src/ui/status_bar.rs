//! Bottom status bar: transient messages on the left; git branch + change
//! count, active language/LSP state, theme and encoding on the right.

use gpui::{div, prelude::*, px, rgba, svg, FontWeight, IntoElement, SharedString};

use crate::theme::Colors;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_status_bar(
    status: &str,
    theme_name: &str,
    git_branch: Option<&str>,
    git_changes: usize,
    lang: Option<&str>,
    lsp_ready: Option<bool>,
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
                .child(SharedString::from(status.to_string())),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                // Git: branch + change count (hidden outside a repository).
                .when_some(git_branch, |bar, branch| {
                    bar.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                svg()
                                    .path("ui_icons/git_branch.svg")
                                    .w(px(13.0))
                                    .h(px(13.0))
                                    .text_color(rgba(t.text)),
                            )
                            .child(SharedString::from(branch.to_string()))
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgba(if git_changes > 0 {
                                        t.vc_modified
                                    } else {
                                        t.text_muted
                                    }))
                                    .child(SharedString::from(git_changes.to_string())),
                            ),
                    )
                })
                // Active file language + LSP connection state.
                .when_some(lang, |bar, lang| {
                    let ready = lsp_ready.unwrap_or(false);
                    let (dot, dot_color) = if ready {
                        ("●", t.vc_added)
                    } else {
                        ("○", t.text_muted)
                    };
                    bar.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(rgba(dot_color))
                                    .child(SharedString::from(dot)),
                            )
                            .child(SharedString::from(lang.to_string())),
                    )
                })
                .child(
                    svg()
                        .path("ui_icons/settings-gear_tint.svg")
                        .w(px(13.0))
                        .h(px(13.0))
                        .text_color(rgba(t.text)),
                )
                .child(SharedString::from(theme_name.to_string()))
                .child(SharedString::from("UTF-8"))
                .child(SharedString::from("Spaces: 4")),
        )
}
