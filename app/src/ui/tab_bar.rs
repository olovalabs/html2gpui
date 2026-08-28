//! Tab bar component inspired by Zed's implementation.
//! Provides VS Code-style tabs with proper positioning and click handling.

use gpui::prelude::*;
use gpui::{div, px, rgba, svg, Context, IntoElement};
use crate::workspace::{OpenTab, Workspace};
use crate::theme::Colors;

/// Tab bar height in pixels
const TAB_HEIGHT: f32 = 36.0;

/// Tab close button size
const CLOSE_SIZE: f32 = 24.0;

/// Renders a single tab with Zed-style positioning
fn render_tab_content(
    tab: &OpenTab,
    index: usize,
    is_active: bool,
    is_first: bool,
    is_last: bool,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let label = if tab.is_settings {
        "Settings".to_string()
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

    // Colors based on active state
    let bg = if is_active { t.tab_active_bg } else { t.tab_inactive_bg };
    let fg = if is_active { t.tab_active_fg } else { t.tab_inactive_fg };

    // Build the base tab div with click handler for switching
    let mut tab_div = div()
        .id(("tab", index))
        .flex()
        .items_center()
        .h(px(TAB_HEIGHT))
        .bg(rgba(bg))
        .cursor_pointer()
        .text_size(px(13.5))
        .text_color(rgba(fg))
        .flex_shrink()
        .overflow_hidden()
        .on_click(cx.listener(move |this, _, _window, cx| {
            // Directly set the active tab
            this.switch_tab_to(index, cx);
        }))
        .hover(|h| {
            if !is_active {
                h.bg(rgba(t.element_hover))
            } else {
                h
            }
        });

    // Apply borders based on position and state
    if is_first && is_last {
        // Single tab
        if is_active {
            tab_div = tab_div.border_b(px(2.0)).border_color(rgba(t.text_accent));
        }
    } else if is_first {
        // First tab
        if is_active {
            tab_div = tab_div.border_b(px(2.0)).border_color(rgba(t.text_accent));
        } else {
            tab_div = tab_div.border_r(px(1.0)).border_color(rgba(t.border));
        }
    } else if is_last {
        // Last tab
        tab_div = tab_div.border_l(px(1.0)).border_color(rgba(t.border));
        tab_div = tab_div.border_r(px(1.0)).border_color(rgba(t.border));
        if is_active {
            tab_div = tab_div.border_b(px(2.0)).border_color(rgba(t.text_accent));
        }
    } else {
        // Middle tab
        tab_div = tab_div.border_l(px(1.0)).border_color(rgba(t.border));
        tab_div = tab_div.border_r(px(1.0)).border_color(rgba(t.border));
        if is_active {
            tab_div = tab_div.border_b(px(2.0)).border_color(rgba(t.text_accent));
        }
    }

    // Close button with its own click handler
    let close_button = div()
        .id(("close-tab", index))
        .w(px(CLOSE_SIZE))
        .h(px(CLOSE_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .flex_none()
        .cursor_pointer()
        .opacity(0.6)
        .hover(|h| h.opacity(1.0).bg(rgba(t.element_hover)).rounded(px(4.0)))
        .on_click(cx.listener(move |this, _, _window, cx| {
            this.close_tab_at_index(index, cx);
        }))
        .child(div().text_sm().text_color(rgba(fg)).child("×"));

    let content = div()
        .flex()
        .flex_row()
        .items_center()
        .h_full()
        .px(px(10.0))
        .gap_x(px(6.0))
        .flex_1()
        .overflow_hidden()
        .children([
            // Dirty indicator
            div()
                .when(tab.dirty, |d| {
                    d.w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(rgba(t.text_accent))
                        .flex_none()
                })
                .when(!tab.dirty, |d| d.w(px(0.0))),
            // Icon if settings tab
            div()
                .when(tab.is_settings, |d| {
                    d.child(
                        svg()
                            .path("ui_icons/settings-gear_tint.svg")
                            .w(px(14.0))
                            .h(px(14.0))
                            .text_color(rgba(fg))
                            .flex_none(),
                    )
                })
                .when(!tab.is_settings, |d| d.w(px(0.0))),
            // Tab label - italic for preview tabs (VS Code style)
            div()
                .flex_1()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .when(tab.preview, |d| d.italic())
                .child(label),
        ]);

    tab_div
        .child(content)
        .child(close_button)
}

/// Renders the tab bar container
pub fn render_tab_bar(
    tabs: &[OpenTab],
    active_tab: usize,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let tab_count = tabs.len();

    div()
        .id("tab-bar")
        .flex()
        .flex_row()
        .h(px(TAB_HEIGHT))
        .bg(rgba(t.tab_bar))
        .border_b(px(1.0))
        .border_color(rgba(t.border))
        .w_full()
        .overflow_hidden()
        .children(tabs.iter().enumerate().map(|(idx, tab)| {
            let is_first = idx == 0;
            let is_last = idx == tab_count - 1;
            let is_active = idx == active_tab;

            render_tab_content(tab, idx, is_active, is_first, is_last, t, cx)
        }))
}
