//! Root layout. Composes title bar → activity bar + sidebar + editor area
//! (+ terminal) → status bar into a single column, and wires every action to
//! its [`Workspace`] command.

use gpui::{div, prelude::*, px, rgba, Context, Render, Window};
use gpui_component::input::Input;
use gpui_component::resizable::{h_resizable, resizable_panel, v_resizable};

use crate::actions::*;
use crate::assets::SANS_FONT;
use crate::fs_tree::display_name;
use crate::ui;
use crate::workspace::CreatingKind;

use super::{Activity, Workspace};

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(path) = self.pending_open.take() {
            self.open_file(path, window, cx);
        }

        let th = self.theme();
        let t = th.colors;
        let welcome = self.welcome_visible();
        let title = self.title();
        let tree = &self.tree;
        // Get the currently open file from active tab
        let open = self.active_path().cloned();
        let selected_path = self.selected_path.clone();
        let explorer_section_expanded = self.explorer_section_expanded;
        let inline_creating = self.inline_creating.clone();
        let status = self.status.clone();
        let activity = self.activity;
        let root_opt = self.root.clone();
        let show_sidebar = self.show_sidebar;
        let show_terminal = self.show_terminal;
        let theme_ix = self.theme_ix;
        let theme_name = th.name.clone();
        let font_size = self.font_size;
        let terminal = self.terminal.clone();
        
        // Tab-related data
        let tabs = self.tabs.clone();
        let active_tab = self.active_tab;
        // Get the active editor from the current tab
        let editor = self.active_editor().cloned();

        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(SANS_FONT)
            .bg(rgba(t.background))
            .text_color(rgba(t.text))
            .text_size(px(13.5))
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
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal(window, cx);
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
            .on_action(cx.listener(|this, _: &ExplorerRefresh, _, cx| {
                this.refresh_explorer(cx);
            }))
            .on_action(cx.listener(|this, _: &ExplorerCollapseAll, _, cx| {
                this.collapse_all_folders(cx);
            }))
            .on_action(cx.listener(|this, action: &ExplorerNewFile, window, cx| {
                this.start_inline_create(CreatingKind::File, action.parent.clone(), window, cx);
            }))
            .on_action(cx.listener(|this, action: &ExplorerNewFolder, window, cx| {
                this.start_inline_create(CreatingKind::Folder, action.parent.clone(), window, cx);
            }))
            .on_action(cx.listener(|this, action: &ExplorerRevealInFinder, _, cx| {
                this.reveal_in_explorer(&action.path, cx);
            }))
            .on_action(cx.listener(|this, action: &ExplorerCopyPath, _, cx| {
                this.copy_path(&action.path, cx);
            }))
            .on_action(cx.listener(|this, action: &ExplorerCopyRelativePath, _, cx| {
                this.copy_relative_path(&action.path, cx);
            }))
            .on_action(cx.listener(|this, action: &ExplorerRename, _, cx| {
                this.rename_entry(&action.path, cx);
            }))
            .on_action(cx.listener(|this, action: &ExplorerDelete, _, cx| {
                this.delete_entry(&action.path, cx);
            }))
            // Tab action handlers
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                this.handle_close_tab(&CloseTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextTab, window, cx| {
                this.handle_next_tab(&NextTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &PrevTab, window, cx| {
                this.handle_prev_tab(&PrevTab, window, cx);
            }))
            .on_action(cx.listener(|this, action: &SwitchTab, window, cx| {
                this.handle_switch_tab(action, window, cx);
            }))
            .on_action(cx.listener(|this, action: &CloseTabAt, window, cx| {
                this.handle_close_tab_at(action, window, cx);
            }))
            // Font zoom actions
            .on_action(cx.listener(|this, _: &IncreaseFontSize, _, cx| {
                this.increase_font_size(cx);
            }))
            .on_action(cx.listener(|this, _: &DecreaseFontSize, _, cx| {
                this.decrease_font_size(cx);
            }))
            .on_action(cx.listener(|this, _: &ResetFontSize, _, cx| {
                this.reset_font_size(cx);
            }))
            .on_action(cx.listener(|this, _: &CopyDiagnostic, _, cx| {
                this.copy_active_diagnostic(cx);
            }))
            // Zed-style key context for workspace-level bindings. Note: do NOT
            // put `track_focus` on this full-window div — its hitbox breaks
            // the platform's client-decoration hit-testing, which kills
            // title-bar dragging on Windows. The focus target lives on the
            // tiny `workspace-focus-catcher` element below instead.
            .key_context("Workspace")
            // Invisible 1x1 focus target: when no editor or terminal is
            // focused, the workspace holds focus here so global keybindings
            // always have a dispatch path (Zed keeps an equivalent workspace
            // focus handle). 1x1px so its hitbox can't interfere with the
            // title bar or window controls.
            .child(
                div()
                    .id("workspace-focus-catcher")
                    .track_focus(&self.focus_handle)
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .size(px(1.0)),
            )
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
                    .child(
                        // Keep separate layout state for the sidebar-visible and
                        // sidebar-hidden layouts so the default width is respected.
                        h_resizable(if show_sidebar {
                            "workspace-h-resizable-with-sidebar"
                        } else {
                            "workspace-h-resizable-editor-only"
                        })
                            .when(show_sidebar, |group| {
                                group.child(
                                    resizable_panel()
                                        .size(px(300.0))
                                        // Keep the sidebar at the VS Code-like width even
                                        // when the window is maximized or resized.
                                        .size_range(px(170.0)..px(300.0))
                                        .child(match activity {
                                            Activity::Explorer => match &root_opt {
                                                Some(root) => ui::sidebar::explorer::render_tree(
                                                    tree,
                                                    Some(root.as_path()),
                                                    open.as_ref(),
                                                    selected_path.as_ref(),
                                                    explorer_section_expanded,
                                                    inline_creating.as_ref(),
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
                                        }),
                                )
                            })
                            .child(
                                resizable_panel().child(
                                    // Use a separate state when the terminal is visible so it
                                    // gets its configured initial height instead of inheriting
                                    // the editor-only layout's minimum panel size.
                                    v_resizable(if show_terminal {
                                        "workspace-v-resizable-with-terminal"
                                    } else {
                                        "workspace-v-resizable-editor-only"
                                    })
                                        .child(
                                            resizable_panel().child(
                                                div()
                                                    .flex_1()
                                                    .h_full()
                                                    .min_w(px(0.0))
                                                    .flex()
                                                    .flex_col()
                                                    .bg(rgba(t.editor_bg))
                                                    // Tab bar - only show when tabs exist
                                                    .when(!tabs.is_empty(), |d| {
                                                        d.child(ui::tab_bar::render_tab_bar(&tabs, active_tab, &t, cx))
                                                    })
                                                    // Editor area
                                                    .when(welcome, |d| d.child(ui::welcome::render_welcome(&t, cx)))
                                                    .when(!welcome, |d| {
                                                        if let Some(editor) = editor {
                                                            d.child(
                                                                div()
                                                                    .flex_1()
                                                                    .min_h(px(0.0))
                                                                    .overflow_hidden()
                                                                    .font_family(crate::assets::MONO_FONT)
                                                                    .text_size(px(font_size))
                                                                    .child(
                                                                        Input::new(&editor)
                                                                            .text_size(px(font_size))
                                                                            .h_full()
                                                                            .appearance(false)
                                                                            .bordered(false),
                                                                    ),
                                                            )
                                                        } else {
                                                            d.flex_1()
                                                        }
                                                    }),
                                            ),
                                        )
                                        .when(show_terminal, |group| {
                                            if let Some(term) = &terminal {
                                                group.child(
                                                    resizable_panel()
                                                        // VS Code opens the terminal as a substantial bottom panel.
                                                        // Keep this as the default while preserving the drag range.
                                                        .size(px(320.0))
                                                        .size_range(px(80.0)..px(2000.0))
                                                        .child(crate::terminal::render_terminal(term, &t, cx)),
                                                )
                                            } else {
                                                group
                                            }
                                        }),
                                ),
                            ),
                    ),
            )
            .child(ui::status_bar::render_status_bar(&status, &theme_name, &t))
    }
}
