//! Root layout. Composes title bar → activity bar + sidebar + editor area
//! (+ terminal) → status bar into a single column, and wires every action to
//! its [`Workspace`] command.

use gpui::{div, prelude::*, px, rgba, Context, Render, Window};
use gpui_component::input::Input;

use crate::actions::*;
use crate::assets::SANS_FONT;
use crate::fs_tree::display_name;
use crate::ui;

use super::{Activity, Workspace};

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let th = self.theme();
        let t = th.colors;
        let welcome = self.welcome_visible();
        let title = self.title();
        let tree = &self.tree;
        let open = self.open.clone();
        let tree_scroll = self.tree_scroll;
        let status = self.status.clone();
        let editor = self.editor.clone();
        let activity = self.activity;
        let root_opt = self.root.clone();
        let show_sidebar = self.show_sidebar;
        let show_terminal = self.show_terminal;
        let theme_ix = self.theme_ix;
        let theme_name = th.name.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(SANS_FONT)
            .bg(rgba(t.background))
            .text_color(rgba(t.text))
            .text_size(px(13.0))
            .on_action(cx.listener(|this, _: &Save, window, cx| this.save(window, cx)))
            .on_action(cx.listener(|this, _: &Quit, _, cx| this.quit(cx)))
            .on_action(cx.listener(|this, _: &ShowExplorer, _, cx| {
                this.set_activity_explicit(Activity::Explorer, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowSearch, _, cx| {
                this.set_activity_explicit(Activity::Search, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowGit, _, cx| {
                this.set_activity_explicit(Activity::Git, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowExtensions, _, cx| {
                this.set_activity_explicit(Activity::Extensions, cx);
            }))
            .on_action(
                cx.listener(|this, _: &ToggleSidebar, _, cx| {
                    this.show_sidebar = !this.show_sidebar;
                    cx.notify();
                }),
            )
            .on_action(cx.listener(|this, _: &ToggleTerminal, _, cx| {
                this.show_terminal = !this.show_terminal;
                this.status = if this.show_terminal {
                    "Terminal".into()
                } else {
                    "Terminal hidden".into()
                };
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NewFile, window, cx| this.new_file(window, cx)))
            .on_action(cx.listener(|this, _: &OpenFile, window, cx| {
                this.open_file_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFolder, window, cx| {
                this.open_folder_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, action: &SelectTheme, window, cx| {
                this.apply_theme(action.ix, window, cx);
            }))
            .on_action(cx.listener(|this, _: &About, _, cx| this.about(cx)))
            .child(ui::titlebar::render_titlebar(&title, &t, theme_ix))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .child(ui::activity_bar::render_activity_bar(
                        activity, show_sidebar, &t, cx,
                    ))
                    .when(show_sidebar, |d| {
                        d.child(match activity {
                            Activity::Explorer => match &root_opt {
                                Some(root) => ui::sidebar::explorer::render_tree(
                                    tree,
                                    open.as_ref(),
                                    tree_scroll,
                                    &display_name(root),
                                    &t,
                                    cx,
                                ),
                                None => ui::welcome::render_no_folder_panel(&t, cx),
                            },
                            Activity::Search => ui::sidebar::search::render_search_panel(&t),
                            Activity::Git => ui::sidebar::git::render_git_panel(&t),
                            Activity::Extensions => {
                                ui::sidebar::extensions::render_extensions_panel(&t)
                            }
                        })
                    })
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .bg(rgba(t.editor_bg))
                            .when(welcome, |d| d.child(ui::welcome::render_welcome(&t, cx)))
                            .when(!welcome, |d| {
                                d.child(
                                    div()
                                        .flex_1()
                                        .min_h(px(0.0))
                                        .overflow_hidden()
                                        .child(
                                            Input::new(&editor)
                                                .h_full()
                                                .appearance(false)
                                                .bordered(false),
                                        ),
                                )
                            })
                            .when(show_terminal, |d| d.child(ui::terminal::render_terminal(&t))),
                    ),
            )
            .child(ui::status_bar::render_status_bar(&status, &theme_name, &t))
    }
}
