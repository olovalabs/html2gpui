//! VS Code-style activity bar: the 48px icon rail on the far left that
//! switches the active sidebar panel using official VS Code Codicons.

use gpui::{div, prelude::*, px, rgba, svg, Context, IntoElement, SharedString, Window};

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
                    "ui_icons/files_tint.svg",
                    show_sidebar && activity == Activity::Explorer,
                    Activity::Explorer,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-search",
                    "ui_icons/search_tint.svg",
                    show_sidebar && activity == Activity::Search,
                    Activity::Search,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-git",
                    "ui_icons/source-control_tint.svg",
                    show_sidebar && activity == Activity::Git,
                    Activity::Git,
                    t,
                    cx,
                ))
                .child(activity_icon(
                    "act-ext",
                    "ui_icons/extensions_tint.svg",
                    show_sidebar && activity == Activity::Extensions,
                    Activity::Extensions,
                    t,
                    cx,
                )),
        )
        .child(
            div().w_full().flex().flex_col().child(activity_static_icon(
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
    which: Activity,
    t: &crate::theme::Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let icon_color = if selected {
        0xffffffff
    } else {
        0x858585ff
    };

    div()
        .id(SharedString::from(id))
        .group(SharedString::from(id))
        .w_full()
        .h(px(48.0))
        .flex()
        .items_center()
        .justify_center()
        .relative()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)))
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(0.0))
                .bottom(px(0.0))
                .w(px(2.0))
                .bg(rgba(if selected { 0xffffffff } else { 0x00000000 })),
        )
        .child(
            svg()
                .path(svg_path)
                .w(px(24.0))
                .h(px(24.0))
                .text_color(rgba(icon_color))
                .group_hover(SharedString::from(id), |s| s.text_color(rgba(0xffffffff))),
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
        .group(SharedString::from(id))
        .w_full()
        .h(px(48.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)))
        .child(
            svg()
                .path(svg_path)
                .w(px(24.0))
                .h(px(24.0))
                .text_color(rgba(0x858585ff))
                .group_hover(SharedString::from(id), |s| s.text_color(rgba(0xffffffff))),
        )
        .on_click(cx.listener(action))
}
