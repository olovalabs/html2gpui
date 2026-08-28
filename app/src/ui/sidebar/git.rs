//! Source Control panel — real Git integration.
//!
//! Shows the actual `git status` of the opened repository: a commit box on
//! top, then STAGED CHANGES and CHANGES lists (only the files that changed,
//! with VS Code-style status letters). Clicking a file opens its diff in the
//! editor; the context menu stages / unstages / discards / opens files.

use gpui::{
    div, prelude::*, px, rgba, svg, AnyElement, Context, ElementId, Entity, FontWeight,
    IntoElement, SharedString, Window,
};
use gpui_component::{
    input::{Input, InputState},
    menu::ContextMenuExt,
    Sizable,
};

use crate::actions::{
    ExplorerCopyPath, ExplorerRevealInFinder, GitDiscardAll, GitDiscardFile, GitOpenDiff,
    GitOpenFile, GitRefresh, GitStageAll, GitStageFile, GitUnstageFile, GitUnstageAll,
};
use crate::git::{ChangeKind, GitChange, RepoStatus};
use crate::theme::Colors;
use crate::ui::common::section_strip;
use crate::workspace::Workspace;

const ROW_HEIGHT: f32 = 24.0;

/// VS Code-style color for a change kind.
fn kind_color(kind: ChangeKind, t: &Colors) -> u32 {
    match kind {
        ChangeKind::Modified => t.vc_modified,
        ChangeKind::Added | ChangeKind::Renamed | ChangeKind::Copied | ChangeKind::Untracked => {
            t.vc_added
        }
        ChangeKind::Deleted | ChangeKind::Conflicted => t.vc_deleted,
        ChangeKind::TypeChanged => t.icon_accent,
    }
}

pub(crate) fn render_git_panel(
    commit_input: Option<&Entity<InputState>>,
    repo: Option<&RepoStatus>,
    t: &Colors,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    let branch = repo
        .and_then(|r| r.branch.clone())
        .unwrap_or_else(|| "NO REPOSITORY".to_string());

    let mut col = div()
        .size_full()
        .flex()
        .flex_col()
        .bg(rgba(t.panel))
        .border_r_1()
        .border_color(rgba(t.border_variant))
        .overflow_hidden();

    col = col.child(header(&branch, t, window, cx));

    match repo {
        None => {
            col = col.child(empty_state(
                "Open a folder inside a Git repository\nto see your changes here.",
                t,
            ));
        }
        Some(repo) => {
            let staged: Vec<GitChange> =
                repo.changes.iter().filter(|c| c.is_staged()).cloned().collect();
            let unstaged: Vec<GitChange> = repo
                .changes
                .iter()
                .filter(|c| !c.is_staged() || c.worktree.is_some() || c.is_untracked())
                .cloned()
                .collect();

            col = col.child(commit_box(commit_input, t, cx));
            col = col.child(
                section_strip(&format!("STAGED CHANGES ({})", staged.len()), t),
            );
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .children(staged.iter().map(|c| {
                        change_row(c, true, "git-staged-row", t, cx)
                    })),
            );
            col = col.child(section_strip(&format!("CHANGES ({})", unstaged.len()), t));
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .children(unstaged.iter().map(|c| {
                        change_row(c, false, "git-change-row", t, cx)
                    })),
            );

            if staged.is_empty() && unstaged.is_empty() {
                col = col.child(empty_state(
                    &format!("Working tree clean — everything is committed on “{branch}”."),
                    t,
                ));
            }
        }
    }

    col.into_any_element()
}

/// Panel header: label + branch + refresh / stage-all / unstage-all buttons.
fn header(
    branch: &str,
    t: &Colors,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    div()
        .h(px(35.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgba(t.text_muted))
                        .child(SharedString::from("SOURCE CONTROL")),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(rgba(t.text_accent))
                        .child(SharedString::from(branch.to_string())),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(header_button("Refresh", "ui_icons/refresh_tint.svg", GitRefresh, t, window, cx))
                .child(header_button("Stage all", "ui_icons/chevron-down_tint.svg", GitStageAll, t, window, cx))
                .child(header_button("Unstage all", "ui_icons/chevron-up_tint.svg", GitUnstageAll, t, window, cx))
                .child(header_button("Discard all", "ui_icons/ellipsis_tint.svg", GitDiscardAll, t, window, cx)),
        )
}

fn header_button(
    tooltip: &'static str,
    icon: &'static str,
    action: impl gpui::Action + 'static,
    t: &Colors,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let _ = tooltip;
    div()
        .id(SharedString::from(format!("git-header-{icon}")))
        .size(px(22.0))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)))
        .child(
            svg()
                .path(icon)
                .w(px(14.0))
                .h(px(14.0))
                .text_color(rgba(t.icon_muted)),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            window.dispatch_action(action.boxed_clone(), cx);
        }))
}

/// Commit message box + Commit button.
fn commit_box(
    input: Option<&Entity<InputState>>,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let input_field = match input {
        Some(input) => div()
            .h(px(30.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .bg(rgba(t.element_bg))
            .border_1()
            .border_color(rgba(t.border))
            .rounded(px(4.0))
            .child(
                Input::new(input)
                    .xsmall()
                    .text_size(px(12.5))
                    .appearance(false)
                    .bordered(false),
            )
            .into_any_element(),
        None => crate::ui::common::mock_input("Message (Ctrl+Enter to commit)", 30.0, t)
            .into_any_element(),
    };

    div()
        .px(px(12.0))
        .py(px(8.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(input_field)
        .child(
            div()
                .h(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgba(t.border_focused))
                .hover(|s| s.bg(rgba(t.icon_accent)))
                .rounded(px(4.0))
                .cursor_pointer()
                .text_size(px(12.0))
                .text_color(rgba(t.background))
                .child(SharedString::from("✓ Commit"))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.git_commit(window, cx);
                })),
        )
}

/// One changed file row. `staged_section` selects the letter/actions shown;
/// `id_prefix` keeps element ids unique when a file appears in both sections.
fn change_row(
    change: &GitChange,
    staged_section: bool,
    id_prefix: &'static str,
    t: &Colors,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let letter = if staged_section {
        change.staged_letter()
    } else {
        change.worktree_letter()
    };
    let kind = if staged_section {
        change.index.unwrap_or(ChangeKind::Modified)
    } else if change.is_untracked() {
        ChangeKind::Untracked
    } else {
        change.worktree.unwrap_or(ChangeKind::Modified)
    };
    let color = kind_color(kind, t);

    let name = change
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| change.rel.trim_end_matches('/').to_string());
    let old_name = change
        .old_rel
        .as_ref()
        .and_then(|old| old.rsplit('/').next())
        .map(|s| s.to_string());

    let path = change.path.clone();

    let mut row = div()
        .id((ElementId::from(id_prefix), change.rel.clone()))
        .group("git-row")
        .w_full()
        .h(px(ROW_HEIGHT))
        .px(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(t.ghost_hover)))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.open_diff(&path, cx);
        }));

    row = row.child(
        div()
            .w(px(14.0))
            .flex_none()
            .text_size(px(11.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgba(color))
            .child(SharedString::from(letter)),
    );

    row = row.child(
        div()
            .flex_1()
            .min_w(px(0.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_size(px(12.5))
                    .text_color(rgba(t.text))
                    .child(SharedString::from(name)),
            )
            .when_some(old_name, |d, old| {
                d.child(
                    div()
                        .flex_none()
                        .text_size(px(11.0))
                        .text_color(rgba(t.text_muted))
                        .child(SharedString::from(format!("← {old}"))),
                )
            }),
    );

    // Per-file actions: unstage (−) in the staged section, stage (+) and
    // discard (↺) in the changes section. Revealed on row hover.
    let path_a = path.clone();
    let path_b = path.clone();
    let path_c = path.clone();
    if staged_section {
        row = row.child(
            row_action("-", "unstage", t, cx)
                .invisible()
                .group_hover("git-row", |s| s.visible())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.git_unstage_path(&path_a, cx);
                })),
        );
    } else {
        row = row.child(
            row_action("+", "stage", t, cx)
                .invisible()
                .group_hover("git-row", |s| s.visible())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.git_stage_path(&path_b, cx);
                })),
        );
        row = row.child(
            row_action("↺", "discard", t, cx)
                .invisible()
                .group_hover("git-row", |s| s.visible())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.git_discard_path(&path_c, cx);
                })),
        );
    }

    // Context menu: open / diff / stage / discard / copy.
    let path_c1 = change.path.clone();
    let path_c2 = change.path.clone();
    let path_c3 = change.path.clone();
    let path_c4 = change.path.clone();
    let path_c5 = change.path.clone();
    let is_staged = change.is_staged();
    let is_untracked = change.is_untracked();

    row.context_menu(move |menu, _window, _cx| {
        menu.menu("Open File", Box::new(GitOpenFile { path: path_c1.clone() }))
            .menu("Open Diff", Box::new(GitOpenDiff { path: path_c2.clone() }))
            .separator()
            .when(!is_staged && !is_untracked, |m| {
                m.menu("Stage Changes", Box::new(GitStageFile { path: path_c3.clone() }))
            })
            .when(is_staged, |m| {
                m.menu("Unstage Changes", Box::new(GitUnstageFile { path: path_c4.clone() }))
            })
            .when(!is_untracked, |m| {
                m.menu("Discard Changes", Box::new(GitDiscardFile { path: path_c5.clone() }))
            })
            .separator()
            .menu(
                "Reveal in File Explorer",
                Box::new(ExplorerRevealInFinder { path: path_c1.clone() }),
            )
            .menu("Copy Path", Box::new(ExplorerCopyPath { path: path_c2.clone() }))
    })
}

fn row_action(
    label: &'static str,
    hover_label: &'static str,
    t: &Colors,
    _cx: &mut Context<Workspace>,
) -> gpui::Stateful<gpui::Div> {
    let _ = hover_label;
    div()
        .id(SharedString::from(format!("git-action-{label}")))
        .size(px(18.0))
        .rounded(px(4.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_size(px(12.0))
        .text_color(rgba(t.text_muted))
        .hover(|s| s.bg(rgba(t.element_hover)).text_color(rgba(t.text)))
        .child(SharedString::from(label))
}

fn empty_state(message: &str, t: &Colors) -> impl IntoElement {
    div()
        .flex_1()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .px(px(20.0))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgba(t.text_muted))
                .child(SharedString::from(message.to_string())),
        )
}
