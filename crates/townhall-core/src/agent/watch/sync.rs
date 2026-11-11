//! What a task needs when its agent's state is seen. The task follows the agent, except
//! where another unit owns it: `queued` (the scheduler starts it), `merged` and the pull
//! request states. An `error` task the scheduler's gate rejected (it keeps the reason in
//! `blocked`) stays there, so its agent session cannot carry it on to ended or needsYou; an
//! `error` set by a crash still follows the agent when it restarts. A thread entry marks each
//! time the agent starts to need you, fails or ends; going back to work adds none.

use crate::agent::state::choices;
use crate::model::{AgentEvent, AgentState, EntryBody, Task, TaskState, Verify, VerifyPhase};

/// A new task state, an entry for its conversation, a new verify state, or any of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Change {
    pub state: Option<TaskState>,
    pub entry: Option<EntryBody>,
    pub verify: Option<Verify>,
}

pub(super) const REMOVED: &str = "the agent was removed";

fn phase(task: &Task) -> Option<VerifyPhase> {
    task.done.verify.as_ref().map(|v| v.phase)
}

/// A task whose check runs, or that failed it for the last time, is owned by the scheduler's
/// verification (`scheduler/verify.rs`): its agent having ended must not end the task.
fn hands_off(task: &Task) -> bool {
    if matches!(
        phase(task),
        Some(VerifyPhase::Verifying | VerifyPhase::Failed)
    ) {
        return true;
    }
    match task.state {
        TaskState::Queued | TaskState::Merged | TaskState::InCi | TaskState::InReview => true,
        TaskState::Error | TaskState::NeedsYou => task.blocked.is_some(),
        _ => false,
    }
}

/// An agent that ended under a check is not done yet: the task stays as it is and is marked
/// `verifying`, keeping the retries and the last failing output it already has.
fn verifying(task: &Task, now: &AgentState) -> Option<Change> {
    let ends = matches!(now, AgentState::Ended);
    let running = matches!(task.state, TaskState::Working | TaskState::NeedsYou);
    if !(ends && running && task.done.check.is_some()) {
        return None;
    }
    let retried = task.done.verify.clone().unwrap_or(Verify {
        phase: VerifyPhase::Verifying,
        retries: 0,
        output: None,
    });
    Some(Change {
        state: None,
        entry: None,
        verify: Some(Verify {
            phase: VerifyPhase::Verifying,
            ..retried
        }),
    })
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
    if hands_off(task) {
        return None;
    }
    if let Some(change) = verifying(task, now) {
        return Some(change);
    }
    let state = target(now);
    if task.state != state {
        return Some(Change {
            state: Some(state),
            entry: entry_for(&task.name, now),
            verify: None,
        });
    }
    let fresh = before.is_some_and(|b| b != now && !grew(b, now));
    let entry = entry_for(&task.name, now).filter(|_| fresh)?;
    Some(Change {
        state: None,
        entry: Some(entry),
        verify: None,
    })
}

/// The agent is gone, worktree and session: a task still running or waiting ended.
pub(super) fn removed(task: &Task) -> Option<Change> {
    let running = matches!(task.state, TaskState::Working | TaskState::NeedsYou);
    (running && task.blocked.is_none()).then(|| Change {
        state: Some(TaskState::Ended),
        entry: Some(agent_line(&task.name, AgentEvent::Ended, REMOVED.into())),
        verify: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Done, Verify, VerifyPhase};

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
            blocked: None,
            pr_base: None,
            done: Default::default(),
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
                entry: None,
                verify: None
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
    fn a_gate_rejected_error_task_never_follows_its_agent_on() {
        let waiting = AgentState::Waiting {
            question: "go? [y/n]".into(),
        };
        let rejected = Task {
            blocked: Some("`t1` (tester) reported FAIL".into()),
            ..task(TaskState::Error)
        };
        for now in [AgentState::Ended, AgentState::Working, waiting] {
            assert_eq!(plan(&rejected, &now, None), None, "{now:?}");
        }
    }

    #[test]
    fn a_crash_set_error_task_still_recovers_when_its_agent_restarts() {
        let crashed = task(TaskState::Error);
        let back = plan(&crashed, &AgentState::Working, None).unwrap();
        assert_eq!(back.state, Some(TaskState::Working));
        let ended = plan(&crashed, &AgentState::Ended, None).unwrap();
        assert_eq!(ended.state, Some(TaskState::Ended));
    }

    fn checked(state: TaskState, verify: Option<Verify>) -> Task {
        let base = task(state);
        Task {
            done: Done {
                check: Some("npm test".into()),
                verify,
                ..base.done.clone()
            },
            ..base
        }
    }

    fn verify(phase: VerifyPhase, retries: u32, output: Option<&str>) -> Verify {
        Verify {
            phase,
            retries,
            output: output.map(String::from),
        }
    }

    #[test]
    fn an_agent_that_ends_under_a_check_is_verified_before_its_task_ends() {
        for state in [TaskState::Working, TaskState::NeedsYou] {
            let change = plan(&checked(state, None), &AgentState::Ended, None).unwrap();
            assert_eq!(
                change,
                Change {
                    state: None,
                    entry: None,
                    verify: Some(verify(VerifyPhase::Verifying, 0, None)),
                },
                "{state:?}"
            );
        }
    }

    #[test]
    fn a_retry_that_ends_is_verified_again_keeping_the_count_and_the_last_output() {
        let retrying = checked(
            TaskState::Working,
            Some(verify(VerifyPhase::Retrying, 1, Some("1 failed"))),
        );
        let change = plan(&retrying, &AgentState::Ended, None).unwrap();
        assert_eq!(
            change.verify,
            Some(verify(VerifyPhase::Verifying, 1, Some("1 failed")))
        );
        assert_eq!(change.state, None);
    }

    #[test]
    fn a_task_under_verification_or_given_up_on_stays_where_it_is() {
        let held = [
            checked(
                TaskState::Working,
                Some(verify(VerifyPhase::Verifying, 0, None)),
            ),
            checked(
                TaskState::NeedsYou,
                Some(verify(VerifyPhase::Failed, 2, Some("boom"))),
            ),
        ];
        for t in held {
            for now in [AgentState::Ended, AgentState::Working, asking("go? [y/n]")] {
                assert_eq!(plan(&t, &now, None), None, "{:?} {now:?}", t.state);
            }
        }
    }

    #[test]
    fn a_check_does_not_hold_back_a_crash_or_a_question() {
        let t = checked(TaskState::Working, None);
        let crashed = plan(&t, &AgentState::Error { exit_code: Some(1) }, None).unwrap();
        assert_eq!(crashed.state, Some(TaskState::Error));
        assert_eq!(crashed.verify, None);
        let asked = plan(&t, &asking("go? [y/n]"), None).unwrap();
        assert_eq!(asked.state, Some(TaskState::NeedsYou));
        let passed = checked(TaskState::Ended, Some(verify(VerifyPhase::Passed, 0, None)));
        assert_eq!(plan(&passed, &AgentState::Ended, None), None);
    }

    #[test]
    fn a_task_parked_for_the_user_stays_parked() {
        let parked = Task {
            blocked: Some("its pull request still has pending checks".into()),
            ..task(TaskState::NeedsYou)
        };
        assert_eq!(plan(&parked, &AgentState::Ended, None), None);
        assert_eq!(removed(&parked), None);
        let asking_user = task(TaskState::NeedsYou);
        assert!(plan(&asking_user, &AgentState::Ended, None).is_some());
        assert!(removed(&asking_user).is_some());
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
