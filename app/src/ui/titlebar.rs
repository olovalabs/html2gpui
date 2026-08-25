//! Custom title bar with the File / Edit / View / Terminal / Help menus.

use gpui::{div, prelude::*, px, rgba, Context, IntoElement, SharedString, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    input::{Copy, Cut, Paste, Redo, SelectAll, Undo},
    menu::{DropdownMenu as _, PopupMenu},
    TitleBar,
};

use crate::actions::*;
use crate::theme::{self, Colors};

pub(crate) fn render_titlebar(title: &str, t: &Colors, theme_ix: usize) -> impl IntoElement {
    TitleBar::new()
        .bg(rgba(t.title_bar))
        .border_color(rgba(t.border))
        .child(
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_row()
                .items_center()
                .text_size(px(13.0))
                .child(menu_btn("m-file", "File", t, |menu, _, _| {
                    menu.menu("New File", Box::new(NewFile))
                        .separator()
                        .menu("Open File…", Box::new(OpenFile))
                        .menu("Open Folder…", Box::new(OpenFolder))
                        .separator()
                        .menu("Save", Box::new(Save))
                        .separator()
                        .menu("Exit", Box::new(Quit))
                }))
                .child(menu_btn("m-edit", "Edit", t, |menu, _, _| {
                    menu.menu("Undo", Box::new(Undo))
                        .menu("Redo", Box::new(Redo))
                        .separator()
                        .menu("Cut", Box::new(Cut))
                        .menu("Copy", Box::new(Copy))
                        .menu("Paste", Box::new(Paste))
                        .separator()
                        .menu("Select All", Box::new(SelectAll))
                }))
                .child(menu_btn("m-view", "View", t, move |menu, window, cx| {
                    menu.menu("Explorer", Box::new(ShowExplorer))
                        .menu("Search", Box::new(ShowSearch))
                        .menu("Source Control", Box::new(ShowGit))
                        .menu("Extensions", Box::new(ShowExtensions))
                        .separator()
                        .submenu("Theme", window, cx, move |menu, _, _| {
                            let mut m = menu;
                            for (i, th) in theme::all().iter().enumerate() {
                                m = m.menu_with_check(
                                    th.name.clone(),
                                    i == theme_ix,
                                    Box::new(SelectTheme { ix: i }),
                                );
                            }
                            m
                        })
                        .separator()
                        .menu("Toggle Primary Side Bar", Box::new(ToggleSidebar))
                        .menu("Toggle Terminal", Box::new(ToggleTerminal))
                }))
                .child(menu_btn("m-term", "Terminal", t, |menu, _, _| {
                    menu.menu("New Terminal", Box::new(ToggleTerminal))
                }))
                .child(menu_btn("m-help", "Help", t, |menu, _, _| {
                    menu.menu("About", Box::new(About))
                }))
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.0))
                        .text_color(rgba(t.text))
                        .child(SharedString::from(title.to_string())),
                ),
        )
}

fn menu_btn(
    id: &'static str,
    label: &'static str,
    t: &Colors,
    build: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
) -> impl IntoElement {
    Button::new(id)
        .ghost()
        .compact()
        .label(label)
        .text_color(rgba(t.text))
        .dropdown_menu(build)
}
