//! VS Code-style activity bar: the 48px icon rail on the far left that
//! switches the active sidebar panel.

use gpui::{div, prelude::*, px, rgba, svg, Context, IntoElement, SharedString, Window};

use crate::theme;
use crate::workspace::{Activity, Workspace};

pub(crate) fn render_activity_bar(
    activity: Activity,
    show_sidebar: bool,
    t: &crate::theme::Colors,
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
            div().w_full().flex().flex_col().child(activity_static_icon(
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
    t: &crate::theme::Colors,
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
    t: &crate::theme::Colors,
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
        .child(svg().path(svg_path).w(px(22.0)).h(px(22.0)))
        .on_click(cx.listener(action))
}
