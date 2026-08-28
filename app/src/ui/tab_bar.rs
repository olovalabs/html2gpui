//! Tab bar component with Dark Islands floating pill and top-accent active styling.

use gpui::prelude::*;
use gpui::{div, px, rgba, Context, FontWeight, IntoElement, SharedString};

use crate::file_icons;
use crate::theme::Colors;
use crate::ui::common::icon_img;
use crate::workspace::{OpenTab, Workspace};

/// Tab bar height in pixels
const TAB_HEIGHT: f32 = 36.0;

/// Renders a single tab matching the Dark Islands screenshot design
fn render_tab_content(
    tab: &OpenTab,
    index: usize,
    is_active: bool,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let label = if tab.is_settings {
        "settings.json".to_string()
    } else {
        tab.path
            .as_ref()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string())
            })
            .unwrap_or_else(|| "Untitled".to_string())
    };

    let icon_path = if tab.is_settings {
        "file_icons/file_type_json.svg"
    } else if let Some(p) = &tab.path {
        file_icons::icon_for(p)
    } else {
        "file_icons/default_file.svg"
    };

    let text_color = if is_active {
        0xf59e0bff // Warm gold / orange for active tab text in Dark Islands
    } else {
        0x94a3b8ff // Muted slate for inactive tabs
    };

    let mut tab_div = div()
        .id(("tab", index))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .text_size(px(13.0))
        .text_color(rgba(text_color))
        .flex_shrink()
        .overflow_hidden()
        .on_click(cx.listener(move |this, _, _window, cx| {
            this.switch_tab_to(index, cx);
        }));

    if is_active {
        tab_div = tab_div
            .h(px(TAB_HEIGHT))
            .bg(rgba(0x16181dff))
            .border_t(px(2.0))
            .border_color(rgba(0xff6f59ff)) // Coral / orange top accent line
            .border_r_1()
            .border_color(rgba(0x232731ff))
            .border_l_1()
            .border_color(rgba(0x232731ff))
            .px(px(10.0));
    } else {
        tab_div = tab_div
            .h(px(26.0))
            .my(px(5.0))
            .mx(px(3.0))
            .rounded(px(6.0))
            .bg(rgba(0x16181dff))
            .border_1()
            .border_color(rgba(0x282c35ff))
            .px(px(8.0))
            .hover(|h| h.bg(rgba(0x1e222aff)));
    }

    // Close button
    let mut close_btn = div()
        .id(("close-tab", index))
        .size(px(18.0))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _window, cx| {
            this.close_tab_at_index(index, cx);
        }));

    if !is_active {
        close_btn = close_btn
            .bg(rgba(0x232732ff))
            .hover(|h| h.bg(rgba(0x353b49ff)));
    } else {
        close_btn = close_btn.hover(|h| h.bg(rgba(t.element_hover)));
    }

    close_btn = close_btn.child(
        div()
            .text_size(px(10.5))
            .text_color(rgba(if is_active { 0xccccccff } else { 0x8b949eff }))
            .child("✕"),
    );

    let content = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .child(icon_img(icon_path, 15.0))
        .child(
            div()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(SharedString::from(label)),
        );

    let status_indicator = if tab.dirty {
        div()
            .text_size(px(11.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgba(0xf59e0bff))
            .child("M")
            .into_any_element()
    } else {
        div().into_any_element()
    };

    tab_div
        .child(content)
        .child(status_indicator)
        .child(close_btn)
}

/// Renders the tab bar container
pub fn render_tab_bar(
    tabs: &[OpenTab],
    active_tab: usize,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    div()
        .id("tab-bar")
        .flex()
        .flex_row()
        .items_center()
        .h(px(TAB_HEIGHT))
        .bg(rgba(0x0e1014ff))
        .border_b_1()
        .border_color(rgba(0x232731ff))
        .w_full()
        .px(px(4.0))
        .overflow_hidden()
        .children(tabs.iter().enumerate().map(|(idx, tab)| {
            let is_active = idx == active_tab;
            render_tab_content(tab, idx, is_active, t, cx)
        }))
}
