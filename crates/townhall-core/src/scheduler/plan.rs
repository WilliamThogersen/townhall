//! The scheduler's decisions that need no git, tmux or store: which queued tasks may start,
//! how many slots are free and the prompt an agent starts with.

use std::path::PathBuf;

use crate::model::{Task, TaskState};

/// `ended` or `merged`: a task an `after` list waits for is done. `error` is not: a task
/// waiting for one stays queued.
pub fn finished(state: TaskState) -> bool {
    matches!(state, TaskState::Ended | TaskState::Merged)
}

fn done(tasks: &[Task], name: &str) -> bool {
    tasks
        .iter()
        .any(|t| t.name.eq_ignore_ascii_case(name) && finished(t.state))
}

/// Queued tasks whose every `after` task finished, in the order of `tasks` (oldest first).
pub fn ready(tasks: &[Task]) -> Vec<&Task> {
    tasks
        .iter()
        .filter(|t| t.state == TaskState::Queued)
        .filter(|t| t.after.iter().all(|a| done(tasks, a)))
        .collect()
}

/// Whether `name` is one of the tasks' (agent names are task names).
pub fn is_task(tasks: &[Task], name: &str) -> bool {
    tasks.iter().any(|t| t.name.eq_ignore_ascii_case(name))
}

/// How many more task agents may start next to `alive` under `limit`.
pub fn free_slots(limit: u32, alive: usize) -> usize {
    usize::try_from(limit)
        .unwrap_or(usize::MAX)
        .saturating_sub(alive)
}

/// The prompt a task's agent starts with: the task's own, then its attachments' paths.
pub fn prompt(task_prompt: &str, files: &[PathBuf]) -> String {
    if files.is_empty() {
        return task_prompt.to_string();
    }
    let listed: Vec<String> = files.iter().map(|f| format!("- {}", f.display())).collect();
    format!(
        "{task_prompt}\n\nFiles attached to this task:\n{}",
        listed.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(name: &str, state: TaskState, after: &[&str]) -> Task {
        Task {
            name: name.into(),
            title: name.into(),
            prompt: "p".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: after.iter().map(|a| a.to_string()).collect(),
            state,
            conversation_id: "c".into(),
            attachments: vec![],
            created_at: "2026-11-04T10:42:05.000Z".into(),
            pr: None,
        }
    }

    #[test]
    fn only_ended_and_merged_count_as_finished() {
        let all = [
            TaskState::Queued,
            TaskState::Working,
            TaskState::NeedsYou,
            TaskState::InCi,
            TaskState::InReview,
            TaskState::Merged,
            TaskState::Ended,
            TaskState::Error,
        ];
        let done: Vec<TaskState> = all.into_iter().filter(|s| finished(*s)).collect();
        assert_eq!(done, [TaskState::Merged, TaskState::Ended]);
    }

    #[test]
    fn ready_keeps_the_order_and_waits_for_every_after_task() {
        let tasks = [
            task("a", TaskState::Ended, &[]),
            task("b", TaskState::Queued, &["A"]),
            task("c", TaskState::Queued, &["a", "d"]),
            task("d", TaskState::Error, &[]),
            task("e", TaskState::Queued, &[]),
            task("f", TaskState::Queued, &["ghost"]),
            task("g", TaskState::Working, &[]),
        ];
        let names: Vec<&str> = ready(&tasks).iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["b", "e"]);
        assert!(is_task(&tasks, "G") && !is_task(&tasks, "z"));
    }

    #[test]
    fn slots_never_go_below_zero() {
        assert_eq!(free_slots(6, 2), 4);
        assert_eq!(free_slots(2, 5), 0);
        assert_eq!(free_slots(1, 0), 1);
    }

    #[test]
    fn attachments_are_listed_after_the_prompt() {
        assert_eq!(prompt("fix it", &[]), "fix it");
        let files = [PathBuf::from("/h/a.png"), PathBuf::from("/h/b c.txt")];
        assert_eq!(
            prompt("fix it", &files),
            "fix it\n\nFiles attached to this task:\n- /h/a.png\n- /h/b c.txt"
        );
    }
}
