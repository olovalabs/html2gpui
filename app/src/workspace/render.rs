//! Root layout. Composes title bar → activity bar + sidebar + editor area
//! (+ terminal) → status bar into a single column, and wires every action to
//! its [`Workspace`] command.

use gpui::{
    div, prelude::*, px, rgba, Context, MouseButton, MouseMoveEvent, MouseUpEvent, Render, Window,
};
use gpui_component::input::Input;

use crate::actions::*;
use crate::assets::SANS_FONT;
use crate::fs_tree::display_name;
use crate::theme::Colors;
use crate::ui;
use crate::workspace::CreatingKind;

use super::{Activity, PanelResizeDrag, ResizeKind, Workspace};

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

        // Clamp panel sizes against the current viewport so shrinking or
        // maximizing the window never leaves a panel overflowing — and so
        // the panels keep their absolute pixel size on window resize
        // (VS Code behavior) instead of scaling proportionally.
        let max_sidebar = f32::from(window.viewport_size().width - px(320.0)).max(220.0);
        self.sidebar_width = self.sidebar_width.clamp(170.0, max_sidebar);
        let sidebar_w = self.sidebar_width;
        let max_terminal = f32::from(window.viewport_size().height - px(220.0)).max(120.0);
        self.terminal_height = self.terminal_height.clamp(80.0, max_terminal);
        let terminal_h = self.terminal_height;
        
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
            // Panel resize drags (sidebar width / terminal height): the
            // move/up listeners live on the root div so the drag keeps
            // working when the cursor moves fast off the thin handle.
            .on_mouse_move(cx.listener(
                |this, ev: &MouseMoveEvent, window, cx| {
                    let Some(rz) = this.panel_resize else {
                        return;
                    };
                    match rz.kind {
                        ResizeKind::Sidebar => {
                            let max = f32::from(window.viewport_size().width - px(320.0)).max(220.0);
                            this.sidebar_width =
                                (rz.start_size + (f32::from(ev.position.x) - rz.start_mouse))
                                    .clamp(170.0, max);
                        }
                        ResizeKind::Terminal => {
                            let max =
                                f32::from(window.viewport_size().height - px(220.0)).max(120.0);
                            this.terminal_height =
                                (rz.start_size - (f32::from(ev.position.y) - rz.start_mouse))
                                    .clamp(80.0, max);
                        }
                    }
                    cx.stop_propagation();
                    cx.notify();
                },
            ))
            .on_mouse_up(MouseButton::Left, cx.listener(
                |this, _: &MouseUpEvent, _, cx| {
                    if this.panel_resize.take().is_some() {
                        cx.notify();
                    }
                },
            ))
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
                    // Sidebar: fixed pixel width, resized by dragging the thin
                    // divider next to it. Not affected by window resize/maximize.
                    .when(show_sidebar, |row| {
                        row.child(
                            div()
                                .w(px(sidebar_w))
                                .flex_shrink_0()
                                .h_full()
                                .overflow_hidden()
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
                        .child(resize_handle(ResizeKind::Sidebar, &t, cx))
                    })
                    .child(
                                // Editor column absorbs all remaining space; the
                        // terminal panel below it is also a fixed-pixel panel
                        // with its own drag divider.
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(0.0))
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
                            )
                            .when(show_terminal, |col| {
                                if let Some(term) = &terminal {
                                    col.child(resize_handle(ResizeKind::Terminal, &t, cx))
                                        .child(
                                            div()
                                                .h(px(terminal_h))
                                                .flex_shrink_0()
                                                .overflow_hidden()
                                                .child(crate::terminal::render_terminal(term, &t, cx)),
                                        )
                                } else {
                                    col
                                }
                            }),
                    ),
            )
            .child(ui::status_bar::render_status_bar(&status, &theme_name, &t))
    }
}

/// Thin draggable divider between panels (VS Code style): a 5px hit zone
/// with a 1px visible line that brightens on hover. Grabbing it starts a
/// panel resize drag handled by the root div's mouse listeners.
fn resize_handle(kind: ResizeKind, t: &Colors, cx: &mut Context<Workspace>) -> impl IntoElement {
    let idle = rgba(t.border_variant);
    let hot = rgba(t.icon);
    let (id, vertical) = match kind {
        ResizeKind::Sidebar => ("sidebar-resize-handle", true),
        ResizeKind::Terminal => ("terminal-resize-handle", false),
    };

    let line = if vertical {
        div().w(px(1.0)).h_full()
    } else {
        div().h(px(1.0)).w_full()
    };

    let base = div()
        .id(id)
        .occlude()
        .flex_shrink_0()
        .group("resize-handle")
        .flex()
        .items_center()
        .justify_center()
        .hover(|s| s.bg(rgba(t.element_hover)));

    let base = if vertical {
        base.w(px(5.0)).h_full().cursor_col_resize()
    } else {
        base.h(px(5.0)).w_full().cursor_row_resize()
    };

    base.child(line.bg(idle).group_hover("resize-handle", |s| s.bg(hot)))
        .on_mouse_down(MouseButton::Left, cx.listener(
        move |this, ev: &gpui::MouseDownEvent, _, cx| {
            let (start_mouse, start_size) = match kind {
                ResizeKind::Sidebar => (f32::from(ev.position.x), this.sidebar_width),
                ResizeKind::Terminal => (f32::from(ev.position.y), this.terminal_height),
            };
            this.panel_resize = Some(PanelResizeDrag {
                kind,
                start_mouse,
                start_size,
            });
            cx.stop_propagation();
            cx.notify();
        },
    ))
}
