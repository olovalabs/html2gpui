//! VS Code-style activity bar: modern floating rail on the far left
//! themed dynamically with active theme tokens.

use gpui::{div, prelude::*, px, rgba, svg, Context, FontWeight, IntoElement, SharedString, Window};

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
        .py(px(6.0))
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(3.0))
                .child(activity_icon(
                    "act-explorer",
                    "ui_icons/files_tint.svg",
                    show_sidebar && activity == Activity::Explorer,
                    None,
                    Activity::Explorer,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-search",
                    "ui_icons/search_tint.svg",
                    show_sidebar && activity == Activity::Search,
                    None,
                    Activity::Search,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-git",
                    "ui_icons/source-control_tint.svg",
                    show_sidebar && activity == Activity::Git,
                    Some("3"),
                    Activity::Git,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-ext",
                    "ui_icons/extensions_tint.svg",
                    show_sidebar && activity == Activity::Extensions,
                    None,
                    Activity::Extensions,
                    t,
                    cx,
                )),
        )
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .pb(px(4.0))
                .child(activity_static_icon(
                    "act-settings",
                    "ui_icons/settings-gear_tint.svg",
                    t,
                    move |this, _, _window, cx| {
                        this.open_settings(cx);
                    },
                    cx,
                )),
        )
}

fn activity_icon(
    id: &'static str,
    svg_path: &'static str,
    selected: bool,
    badge: Option<&'static str>,
    which: Activity,
    t: &crate::theme::Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let icon_color = if selected {
        t.text
    } else {
        t.icon_muted
    };

    let mut inner = div()
        .size(px(36.0))
        .rounded(px(10.0))
        .flex()
        .items_center()
        .justify_center()
        .relative();

    if selected {
        inner = inner
            .bg(rgba(t.element_selected))
            .border_1()
            .border_color(rgba(t.border_focused));
    } else {
        inner = inner.hover(|s| s.bg(rgba(t.ghost_hover)));
    }

    inner = inner.child(
        svg()
            .path(svg_path)
            .w(px(20.0))
            .h(px(20.0))
            .text_color(rgba(icon_color))
            .group_hover(SharedString::from(id), |s| s.text_color(rgba(t.text))),
    );

    if let Some(b) = badge {
        inner = inner.child(
            div()
                .absolute()
                .bottom(px(0.0))
                .right(px(0.0))
                .min_w(px(15.0))
                .h(px(15.0))
                .px(px(3.0))
                .rounded_full()
                .bg(rgba(t.text_accent))
                .border_1()
                .border_color(rgba(t.panel))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(9.5))
                .font_weight(FontWeight::BOLD)
                .text_color(rgba(t.background))
                .child(SharedString::from(b)),
        );
    }

    div()
        .id(SharedString::from(id))
        .group(SharedString::from(id))
        .w_full()
        .h(px(42.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(inner)
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_activity(which, cx)))
}

fn activity_static_icon(
    id: &'static str,
    svg_path: &'static str,
    t: &crate::theme::Colors,
    action: impl Fn(&mut Workspace, &gpui::ClickEvent, &mut Window, &mut Context<Workspace>) + 'static,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let mut inner = div()
        .size(px(36.0))
        .rounded(px(10.0))
        .flex()
        .items_center()
        .justify_center()
        .relative()
        .hover(|s| s.bg(rgba(t.ghost_hover)));

    inner = inner.child(
        svg()
            .path(svg_path)
            .w(px(20.0))
            .h(px(20.0))
            .text_color(rgba(t.icon_muted))
            .group_hover(SharedString::from(id), |s| s.text_color(rgba(t.text))),
    );

    div()
        .id(SharedString::from(id))
        .group(SharedString::from(id))
        .w_full()
        .h(px(42.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(inner)
        .on_click(cx.listener(action))
}
