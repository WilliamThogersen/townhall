use gpui::{
    div, prelude::*, px, AnyElement, App, FontWeight, MouseButton, MouseDownEvent, Rgba,
    SharedString, Window,
};
use townhall_core::model::ConversationDetail;

use crate::session_model::{context_view, repo_line, ContextView, RepoLine};
use crate::task_rows::{task_summary, task_views, TaskTone, TaskView};
use crate::theme::{self, Theme};

pub struct SessionModel {
    pub summary: String,
    pub views: Vec<TaskView>,
    pub repo: Option<RepoLine>,
    pub files: Vec<String>,
    pub context: Option<ContextView>,
}

pub fn session_model(detail: &ConversationDetail) -> SessionModel {
    SessionModel {
        summary: task_summary(&detail.tasks),
        views: task_views(&detail.tasks),
        repo: repo_line(detail),
        files: detail.attachments.iter().map(|a| a.name.clone()).collect(),
        context: context_view(detail.conversation.context.as_ref()),
    }
}

fn faint_line(t: &Theme, text: &str) -> AnyElement {
    div()
        .px(px(theme::SPACE_4))
        .py(px(theme::SPACE_1))
        .text_size(px(theme::TEXT_SM))
        .text_color(t.palette.fg_faint)
        .child(text.to_string())
        .into_any_element()
}

fn section(t: &Theme, label: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(theme::SPACE_2))
        .pt(px(theme::SPACE_3))
        .pb(px(theme::SPACE_1))
        .pl(px(theme::SPACE_4))
        .pr(px(theme::SPACE_3))
        .text_size(px(theme::TEXT_XS))
        .text_color(t.palette.fg_faint)
        .child(label)
        .child(
            div()
                .flex_1()
                .border_t(px(theme::BORDER))
                .border_color(t.palette.line),
        )
        .into_any_element()
}

fn tag(t: &Theme, text: String) -> AnyElement {
    div()
        .flex_none()
        .px(px(4.0))
        .border(px(theme::BORDER))
        .border_color(t.palette.line)
        .text_size(px(theme::TEXT_XS))
        .text_color(t.palette.fg_faint)
        .child(text)
        .into_any_element()
}

fn state_color(t: &Theme, tone: Option<TaskTone>) -> Rgba {
    match tone {
        Some(TaskTone::Waiting) => t.palette.accent,
        Some(TaskTone::Error) => t.palette.danger,
        _ => t.palette.fg_faint,
    }
}

fn task_row(t: &Theme, view: &TaskView) -> AnyElement {
    let p = t.palette;
    let ch = theme::TEXT_SM * 0.6;
    let task = &view.task;
    let name_color = match view.tone {
        Some(TaskTone::Merged | TaskTone::Dead) => p.fg_muted,
        _ => p.fg,
    };
    let mut sub = div()
        .flex()
        .items_center()
        .gap(px(ch))
        .pl(px(3.0 * ch))
        .child(
            div()
                .flex_1()
                .truncate()
                .text_size(px(theme::TEXT_XS))
                .text_color(p.fg_faint)
                .child(task.title.clone()),
        )
        .child(tag(t, task.agent.to_lowercase()));
    if let Some(target) = &view.pr_target {
        sub = sub.child(tag(t, format!("pr to {target}")));
    }
    if view.pr_open {
        sub = sub.child(tag(t, "pr open".to_string()));
    }
    let mut main = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .py(px(6.0))
        .pl(px(theme::SPACE_3))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(ch))
                .child(
                    div()
                        .w(px(2.0 * ch))
                        .text_color(view.glyph.color(t))
                        .when(view.glyph.bold(), |d| d.font_weight(FontWeight::BOLD))
                        .child(view.glyph.symbol()),
                )
                .child(
                    div()
                        .flex_1()
                        .truncate()
                        .text_color(name_color)
                        .child(task.name.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(theme::TEXT_XS))
                        .text_color(state_color(t, view.tone))
                        .child(view.words.clone()),
                ),
        )
        .child(sub);
    if let Some(reason) = &view.blocked {
        main = main.child(
            div()
                .pl(px(3.0 * ch))
                .text_size(px(theme::TEXT_XS))
                .text_color(p.fg_faint)
                .child(reason.clone()),
        );
    }
    let link = match &task.pr {
        None => div().into_any_element(),
        Some(pr) => {
            let label = format!("#{}", pr.number);
            let link = div()
                .id(SharedString::from(format!("pr-{}", task.name)))
                .pt(px(6.0))
                .pr(px(theme::SPACE_3))
                .pl(px(theme::SPACE_2))
                .text_size(px(theme::TEXT_XS))
                .text_color(p.fg_faint);
            match pr.url.clone() {
                Some(url) => link
                    .cursor_pointer()
                    .hover(move |s| s.text_color(p.fg))
                    .on_mouse_down(
                        MouseButton::Left,
                        move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| cx.open_url(&url),
                    )
                    .child(label)
                    .into_any_element(),
                None => link.child(label).into_any_element(),
            }
        }
    };
    div()
        .w_full()
        .flex()
        .items_start()
        .hover(move |s| s.bg(p.surface_2))
        .child(main)
        .child(link)
        .into_any_element()
}

fn context_line(t: &Theme, view: &ContextView) -> AnyElement {
    let p = t.palette;
    let color = if view.high { p.accent } else { p.fg_faint };
    div()
        .flex()
        .items_center()
        .gap(px(theme::TEXT_SM * 0.6))
        .px(px(theme::SPACE_4))
        .py(px(theme::SPACE_1))
        .text_size(px(theme::TEXT_XS))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_color(color)
                .child(
                    div()
                        .flex_none()
                        .w(px(32.0))
                        .h(px(4.0))
                        .bg(p.surface_3)
                        .child(
                            div()
                                .h_full()
                                .w(px(32.0 * view.percent as f32 / 100.0))
                                .bg(if view.high { p.accent } else { p.fg_faint }),
                        ),
                )
                .child(format!("{}%", view.percent)),
        )
        .child(div().text_color(p.fg_faint).child(view.words.clone()))
        .into_any_element()
}

pub fn session_panel(detail: &ConversationDetail, t: &Theme) -> AnyElement {
    let p = t.palette;
    let m = session_model(detail);
    let mut body = div()
        .id("session-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .pb(px(theme::SPACE_4))
        .child(section(t, format!("tasks {}", m.views.len())));
    if m.views.is_empty() {
        body = body.child(faint_line(t, "no tasks yet"));
    }
    for view in &m.views {
        body = body.child(task_row(t, view));
    }
    body = body.child(section(t, "repo".to_string()));
    body = body.child(match &m.repo {
        None => faint_line(t, "no repo yet"),
        Some(line) => div()
            .flex()
            .items_baseline()
            .gap(px(theme::TEXT_SM * 0.6))
            .px(px(theme::SPACE_4))
            .py(px(theme::SPACE_1))
            .text_size(px(theme::TEXT_SM))
            .child(line.label.clone())
            .child(
                div()
                    .text_size(px(theme::TEXT_XS))
                    .text_color(p.fg_faint)
                    .child(line.reason.clone()),
            )
            .into_any_element(),
    });
    body = body.child(section(t, format!("files {}", m.files.len())));
    if m.files.is_empty() {
        body = body.child(faint_line(t, "no files yet"));
    }
    for name in &m.files {
        body = body.child(faint_line(t, name));
    }
    body = body.child(section(t, "context".to_string()));
    body = body.child(match &m.context {
        None => faint_line(t, "no context yet"),
        Some(view) => context_line(t, view),
    });
    div()
        .debug_selector(|| "session".to_string())
        .w(px(theme::RAIL))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .bg(p.surface)
        .border_l(px(theme::BORDER))
        .border_color(p.line)
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(theme::TEXT_MD * 0.6))
                .h(px(theme::BAR))
                .pl(px(theme::SPACE_4))
                .pr(px(theme::SPACE_2))
                .border_b(px(theme::BORDER))
                .border_color(p.line)
                .child(div().font_weight(FontWeight::BOLD).child("session"))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(theme::TEXT_XS))
                        .text_color(p.fg_faint)
                        .child(m.summary.clone()),
                ),
        )
        .child(body)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{conversation, detail_of, entry_of, task, you};
    use townhall_core::model::{Attachment, ContextUsage, EntryBody, RepoRule, TaskState};

    fn attachment(name: &str) -> Attachment {
        serde_json::from_value(serde_json::json!({
            "id": name, "conversationId": "c", "name": name, "mime": "text/plain",
            "size": 3, "kind": "file", "createdAt": "2026-11-12T09:00:00.000Z"
        }))
        .unwrap()
    }

    fn detail() -> ConversationDetail {
        let mut c = conversation("c", "t", "2026-11-12T10:00:00.000Z");
        c.context = Some(ContextUsage {
            used: 160_000,
            limit: 200_000,
        });
        let mut d = detail_of(
            c,
            vec![
                you(1, "hi"),
                entry_of(
                    2,
                    EntryBody::Repo {
                        repo: "shop".into(),
                        base_branch: "develop".into(),
                        rule: RepoRule::Named,
                        reason: "you named it".into(),
                    },
                ),
            ],
            vec![
                task("done", TaskState::Merged),
                task("ask", TaskState::NeedsYou),
                task("run", TaskState::Working),
            ],
        );
        d.attachments = vec![attachment("a.log"), attachment("b.png")];
        d
    }

    #[test]
    fn the_session_lists_tasks_by_attention_with_a_count_and_a_summary() {
        let m = session_model(&detail());
        let names: Vec<&str> = m.views.iter().map(|v| v.task.name.as_str()).collect();
        assert_eq!(names, ["ask", "run", "done"]);
        assert_eq!(m.summary, "3 tasks · 1 needs you · 1 working · 1 merged");
    }

    #[test]
    fn the_session_carries_repo_files_and_context() {
        let m = session_model(&detail());
        assert_eq!(m.repo.unwrap().label, "~ shop · develop");
        assert_eq!(m.files, ["a.log", "b.png"]);
        let ctx = m.context.unwrap();
        assert_eq!((ctx.percent, ctx.high), (80, true));
    }

    #[test]
    fn an_empty_session_has_nothing_to_show() {
        let d = detail_of(
            conversation("c", "t", "2026-11-12T10:00:00.000Z"),
            vec![],
            vec![],
        );
        let m = session_model(&d);
        assert_eq!(m.summary, "0 tasks");
        assert!(m.views.is_empty() && m.repo.is_none() && m.files.is_empty());
        assert!(m.context.is_none());
    }
}
