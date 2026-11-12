use townhall_core::model::{Conversation, ConversationStatus, Task, TaskState};

use crate::glyph::Glyph;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Waiting,
    Error,
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowSummary {
    pub glyph: Glyph,
    pub state: Option<RowState>,
    pub status: String,
    pub needs_you: bool,
}

fn is_running(state: TaskState) -> bool {
    matches!(
        state,
        TaskState::Queued | TaskState::Working | TaskState::InCi | TaskState::InReview
    )
}

fn about(tasks: &[&Task], one: &str, many: &str) -> String {
    match tasks {
        [only] => format!("{} {one}", only.name),
        _ => format!("{} tasks {many}", tasks.len()),
    }
}

fn progress(conversation: &Conversation, tasks: &[Task]) -> String {
    let running = tasks.iter().filter(|t| is_running(t.state)).count();
    let merged = tasks
        .iter()
        .filter(|t| t.state == TaskState::Merged)
        .count();
    let mut parts = vec![if conversation.ended {
        "ended"
    } else {
        "your turn"
    }
    .to_string()];
    if running > 0 {
        parts.push(format!("{running} running"));
    }
    if merged > 0 {
        parts.push(format!("{merged} merged"));
    }
    parts.join(" · ")
}

fn quiet_glyph(conversation: &Conversation, tasks: &[Task]) -> Glyph {
    if conversation.status == ConversationStatus::Thinking {
        Glyph::Working
    } else if tasks.iter().any(|t| is_running(t.state)) {
        Glyph::Alive
    } else {
        Glyph::Dead
    }
}

pub fn summarize_row(conversation: &Conversation, tasks: &[Task]) -> RowSummary {
    let waiting: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.state == TaskState::NeedsYou)
        .collect();
    if !waiting.is_empty() {
        return RowSummary {
            glyph: Glyph::Waiting,
            state: Some(RowState::Waiting),
            status: about(&waiting, "needs you", "need you"),
            needs_you: true,
        };
    }
    let failed: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.state == TaskState::Error)
        .collect();
    if !failed.is_empty() {
        return RowSummary {
            glyph: Glyph::Error,
            state: Some(RowState::Error),
            status: about(&failed, "failed", "failed"),
            needs_you: false,
        };
    }
    let status = if conversation.status == ConversationStatus::Thinking {
        "thinking".to_string()
    } else {
        progress(conversation, tasks)
    };
    RowSummary {
        glyph: quiet_glyph(conversation, tasks),
        state: conversation.ended.then_some(RowState::Ended),
        status,
        needs_you: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{conversation, task};
    use townhall_core::model::{ConversationStatus, TaskState};

    fn conv() -> Conversation {
        conversation("c", "t", "2026-11-12T10:00:00.000Z")
    }

    fn tasks(states: &[TaskState]) -> Vec<Task> {
        states
            .iter()
            .enumerate()
            .map(|(i, s)| task(&format!("task-{i}"), *s))
            .collect()
    }

    fn summary(glyph: Glyph, state: Option<RowState>, status: &str, needs_you: bool) -> RowSummary {
        RowSummary {
            glyph,
            state,
            status: status.into(),
            needs_you,
        }
    }

    #[test]
    fn a_task_that_needs_you_comes_first_by_name() {
        let mut c = conv();
        c.status = ConversationStatus::Thinking;
        let row = summarize_row(
            &c,
            &[
                task("e2e-smoke", TaskState::Error),
                task("fix-login", TaskState::NeedsYou),
            ],
        );
        assert_eq!(
            row,
            summary(
                Glyph::Waiting,
                Some(RowState::Waiting),
                "fix-login needs you",
                true
            )
        );
    }

    #[test]
    fn several_tasks_that_need_you_are_counted() {
        let row = summarize_row(
            &conv(),
            &tasks(&[TaskState::NeedsYou, TaskState::NeedsYou, TaskState::Working]),
        );
        assert_eq!(row.status, "2 tasks need you");
    }

    #[test]
    fn a_failed_task_is_named_then_several_are_counted() {
        let one = summarize_row(&conv(), &[task("e2e-smoke", TaskState::Error)]);
        assert_eq!(
            one,
            summary(
                Glyph::Error,
                Some(RowState::Error),
                "e2e-smoke failed",
                false
            )
        );
        let two = summarize_row(&conv(), &tasks(&[TaskState::Error, TaskState::Error]));
        assert_eq!(two.status, "2 tasks failed");
    }

    #[test]
    fn thinking_shows_a_spinner_while_a_turn_runs() {
        let mut c = conv();
        c.status = ConversationStatus::Thinking;
        let row = summarize_row(&c, &tasks(&[TaskState::Working]));
        assert_eq!(row, summary(Glyph::Working, None, "thinking", false));
    }

    #[test]
    fn running_and_merged_tasks_are_counted_on_your_turn() {
        let row = summarize_row(
            &conv(),
            &tasks(&[
                TaskState::Working,
                TaskState::Queued,
                TaskState::InCi,
                TaskState::InReview,
                TaskState::Merged,
                TaskState::Ended,
            ]),
        );
        assert_eq!(
            row,
            summary(
                Glyph::Alive,
                None,
                "your turn · 4 running · 1 merged",
                false
            )
        );
    }

    #[test]
    fn idle_on_your_turn_with_nothing_running() {
        let row = summarize_row(&conv(), &[]);
        assert_eq!(row, summary(Glyph::Dead, None, "your turn", false));
    }

    #[test]
    fn an_ended_conversation_is_marked_and_keeps_its_counts() {
        let mut c = conv();
        c.ended = true;
        let row = summarize_row(&c, &tasks(&[TaskState::Merged, TaskState::Merged]));
        assert_eq!(
            row,
            summary(
                Glyph::Dead,
                Some(RowState::Ended),
                "ended · 2 merged",
                false
            )
        );
        assert_eq!(
            summarize_row(&c, &tasks(&[TaskState::Working])).status,
            "ended · 1 running"
        );
    }

    #[test]
    fn attention_wins_over_ended() {
        let mut c = conv();
        c.ended = true;
        let row = summarize_row(&c, &tasks(&[TaskState::NeedsYou]));
        assert_eq!(row.state, Some(RowState::Waiting));
    }
}
