//! Whether an agent goes: a pure decision on the tasks and the ended conversations.

use std::collections::HashSet;

use crate::model::{Task, TaskState};

/// Why an agent is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// Its task's conversation ended.
    Ended,
    /// Its task's branch was merged.
    Merged,
    Settled,
    /// No task has its name: nothing will ever use it.
    Orphan,
}

impl Why {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ended => "ended",
            Self::Merged => "merged",
            Self::Settled => "settled",
            Self::Orphan => "orphan",
        }
    }
}

/// Why the agent `name` goes, `None` while its task is live work. Task names are unique
/// ignoring case, as worktree folders and branches are on a case-insensitive disk.
pub fn verdict(
    name: &str,
    tasks: &[Task],
    ended: &HashSet<String>,
    settled: &HashSet<String>,
) -> Option<Why> {
    match tasks.iter().find(|t| t.name.eq_ignore_ascii_case(name)) {
        None => Some(Why::Orphan),
        Some(t) if ended.contains(&t.conversation_id) => Some(Why::Ended),
        Some(t) if t.state == TaskState::Merged => Some(Why::Merged),
        Some(t) if t.state == TaskState::Ended && settled.contains(&t.name) => Some(Why::Settled),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(name: &str, conversation: &str, state: TaskState) -> Task {
        Task {
            name: name.into(),
            title: name.into(),
            prompt: "p".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: vec![],
            state,
            conversation_id: conversation.into(),
            attachments: vec![],
            created_at: "2026-11-05T10:00:00.000Z".into(),
            pr: None,
            agent: "builder".into(),
            blocked: None,
            pr_base: None,
            done: Default::default(),
        }
    }

    #[test]
    fn live_work_stays_and_the_rest_goes() {
        let tasks = [
            task("live", "open", TaskState::Working),
            task("done", "open", TaskState::Ended),
            task("shipped", "open", TaskState::Merged),
            task("old", "closed", TaskState::Working),
        ];
        let ended = HashSet::from(["closed".to_string()]);
        let none = HashSet::new();
        assert_eq!(verdict("live", &tasks, &ended, &none), None);
        assert_eq!(
            verdict("done", &tasks, &ended, &none),
            None,
            "an ended task of an open conversation may still be talked about"
        );
        assert_eq!(verdict("shipped", &tasks, &ended, &none), Some(Why::Merged));
        assert_eq!(verdict("old", &tasks, &ended, &none), Some(Why::Ended));
        assert_eq!(verdict("nobody", &tasks, &ended, &none), Some(Why::Orphan));
    }

    #[test]
    fn an_ended_task_goes_once_settled() {
        let tasks = [
            task("done", "open", TaskState::Ended),
            task("busy", "open", TaskState::Working),
        ];
        let settled = HashSet::from(["done".to_string(), "busy".to_string()]);
        let none = HashSet::new();
        assert_eq!(verdict("done", &tasks, &none, &settled), Some(Why::Settled));
        assert_eq!(
            verdict("busy", &tasks, &none, &settled),
            None,
            "only an ended task settles"
        );
    }

    #[test]
    fn names_match_ignoring_case() {
        let tasks = [task("Fix", "open", TaskState::Working)];
        assert_eq!(
            verdict("fix", &tasks, &HashSet::new(), &HashSet::new()),
            None
        );
    }
}
