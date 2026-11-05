//! What a task needs when its agent's state is seen. The task follows the agent, except
//! where another unit owns it: `queued` (the scheduler starts it), `merged` and the pull
//! request states. A thread entry marks each time the agent starts to need you, fails or
//! ends; going back to work adds none.

use crate::agent::state::choices;
use crate::model::{AgentEvent, AgentState, EntryBody, Task, TaskState};

/// A new task state, an entry for its conversation, or both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Change {
    pub state: Option<TaskState>,
    pub entry: Option<EntryBody>,
}

pub(super) const REMOVED: &str = "the agent was removed";

fn hands_off(state: TaskState) -> bool {
    matches!(
        state,
        TaskState::Queued | TaskState::Merged | TaskState::InCi | TaskState::InReview
    )
}

fn target(now: &AgentState) -> TaskState {
    match now {
        AgentState::Working => TaskState::Working,
        AgentState::Waiting { .. } => TaskState::NeedsYou,
        AgentState::Error { .. } => TaskState::Error,
        AgentState::Ended => TaskState::Ended,
    }
}

fn agent_line(agent: &str, event: AgentEvent, text: String) -> EntryBody {
    EntryBody::Agent {
        agent: agent.to_string(),
        event,
        text,
    }
}

/// The entry for an agent arriving in `now`; none for working.
fn entry_for(agent: &str, now: &AgentState) -> Option<EntryBody> {
    match now {
        AgentState::Working => None,
        AgentState::Waiting { question } => Some(EntryBody::NeedsYou {
            agent: agent.to_string(),
            question: question.clone(),
            choices: choices(question),
        }),
        AgentState::Error { exit_code } => {
            let text = match exit_code {
                Some(code) => format!("exited with code {code}"),
                None => "exited without an exit code".to_string(),
            };
            Some(agent_line(agent, AgentEvent::Failed, text))
        }
        AgentState::Ended => Some(agent_line(agent, AgentEvent::Ended, "ended".into())),
    }
}

/// The same question with text typed after it, not yet sent: not a new question.
fn grew(before: &AgentState, now: &AgentState) -> bool {
    match (before, now) {
        (AgentState::Waiting { question: old }, AgentState::Waiting { question: new }) => {
            new.starts_with(old.trim_end())
        }
        _ => false,
    }
}

/// The task's change for its agent in `now`. `before` is the state last acted on: when the
/// task already matches, a new state (another question) still gets its entry, but with no
/// memory (`None`) it is taken as already recorded, and a question that only grew by text
/// typed after it is the same question.
pub(super) fn plan(task: &Task, now: &AgentState, before: Option<&AgentState>) -> Option<Change> {
    if hands_off(task.state) {
        return None;
    }
    let state = target(now);
    if task.state != state {
        return Some(Change {
            state: Some(state),
            entry: entry_for(&task.name, now),
        });
    }
    let fresh = before.is_some_and(|b| b != now && !grew(b, now));
    let entry = entry_for(&task.name, now).filter(|_| fresh)?;
    Some(Change {
        state: None,
        entry: Some(entry),
    })
}

/// The agent is gone, worktree and session: a task still running or waiting ended.
pub(super) fn removed(task: &Task) -> Option<Change> {
    matches!(task.state, TaskState::Working | TaskState::NeedsYou).then(|| Change {
        state: Some(TaskState::Ended),
        entry: Some(agent_line(&task.name, AgentEvent::Ended, REMOVED.into())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(state: TaskState) -> Task {
        Task {
            name: "fix".into(),
            title: "fix it".into(),
            prompt: "p".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: vec![],
            state,
            conversation_id: "c".into(),
            attachments: vec![],
            created_at: "2026-11-04T10:00:00.000Z".into(),
            pr: None,
            agent: "builder".into(),
        }
    }

    fn asking(q: &str) -> AgentState {
        AgentState::Waiting { question: q.into() }
    }

    #[test]
    fn the_task_follows_its_agent_with_one_entry_per_arrival() {
        let working = task(TaskState::Working);
        let change = plan(&working, &asking("go? [y/n]"), None).unwrap();
        assert_eq!(change.state, Some(TaskState::NeedsYou));
        assert_eq!(
            change.entry,
            Some(EntryBody::NeedsYou {
                agent: "fix".into(),
                question: "go? [y/n]".into(),
                choices: vec!["y".into(), "n".into()],
            })
        );
        let back = plan(&task(TaskState::NeedsYou), &AgentState::Working, None).unwrap();
        assert_eq!(
            back,
            Change {
                state: Some(TaskState::Working),
                entry: None
            }
        );
        let failed = plan(&working, &AgentState::Error { exit_code: Some(3) }, None).unwrap();
        assert_eq!(
            failed.entry,
            Some(agent_line(
                "fix",
                AgentEvent::Failed,
                "exited with code 3".into()
            ))
        );
        let unknown = plan(&working, &AgentState::Error { exit_code: None }, None).unwrap();
        assert_eq!(
            unknown.entry,
            Some(agent_line(
                "fix",
                AgentEvent::Failed,
                "exited without an exit code".into()
            ))
        );
        let ended = plan(&task(TaskState::Ended), &AgentState::Working, None).unwrap();
        assert_eq!(ended.state, Some(TaskState::Working));
    }

    #[test]
    fn a_matching_task_gets_an_entry_only_for_a_new_state() {
        let waiting = task(TaskState::NeedsYou);
        assert_eq!(plan(&waiting, &asking("a?"), None), None);
        assert_eq!(plan(&waiting, &asking("a?"), Some(&asking("a?"))), None);
        let next = plan(&waiting, &asking("b?"), Some(&asking("a?"))).unwrap();
        assert_eq!(next.state, None);
        assert!(
            matches!(next.entry, Some(EntryBody::NeedsYou { question, .. }) if question == "b?")
        );
        assert_eq!(
            plan(
                &task(TaskState::Working),
                &AgentState::Working,
                Some(&asking("a?"))
            ),
            None
        );
    }

    #[test]
    fn a_question_that_only_grew_by_typed_text_is_not_new() {
        let waiting = task(TaskState::NeedsYou);
        let before = asking("name? ");
        assert_eq!(plan(&waiting, &asking("name? bo"), Some(&before)), None);
        assert_eq!(
            plan(&waiting, &asking("name? bob"), Some(&asking("name? bo"))),
            None
        );
        let other = plan(&waiting, &asking("age?"), Some(&before)).unwrap();
        assert!(
            matches!(other.entry, Some(EntryBody::NeedsYou { question, .. }) if question == "age?")
        );
    }

    #[test]
    fn other_units_tasks_are_left_alone() {
        for state in [
            TaskState::Queued,
            TaskState::Merged,
            TaskState::InCi,
            TaskState::InReview,
        ] {
            assert_eq!(
                plan(&task(state), &AgentState::Ended, None),
                None,
                "{state:?}"
            );
            assert_eq!(removed(&task(state)), None);
        }
    }

    #[test]
    fn a_removed_agent_ends_only_a_running_task() {
        for state in [TaskState::Working, TaskState::NeedsYou] {
            let change = removed(&task(state)).unwrap();
            assert_eq!(change.state, Some(TaskState::Ended));
        }
        for state in [TaskState::Ended, TaskState::Error] {
            assert_eq!(removed(&task(state)), None);
        }
    }
}
