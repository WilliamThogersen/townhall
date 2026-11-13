//! The line the planner is woken with when a task's thread entry says the user should
//! hear of it: the agent needs you, failed or ended (with what it said last). Going back
//! to work, `sent` and `passed` lines wake nobody.

use crate::agent::state;
use serde_json::{json, Value};

use crate::model::{AgentEvent, EntryBody, Task};
use crate::{steps, Context};

/// The longest last message put in an event, in characters.
const MAX_SAID_CHARS: usize = 1500;

fn capped(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_SAID_CHARS {
        return text.to_string();
    }
    let kept: String = text.chars().take(MAX_SAID_CHARS - 1).collect();
    format!("{kept}\u{2026}")
}

async fn said(ctx: &Context, task: &Task) -> Option<String> {
    let repo = ctx.repo(&task.repo).await.ok()?;
    let worktree = ctx.workspace(&repo).ok()?.worktrees_dir().join(&task.name);
    let projects = steps::claude_projects_dir().ok()?;
    let said = state::last_message(&projects, &worktree).await.ok()??;
    Some(capped(&said)).filter(|s| !s.is_empty())
}

/// The event for `entry`, the one just appended to `task`'s conversation, or none.
pub(super) async fn for_entry(ctx: &Context, task: &Task, entry: &EntryBody) -> Option<String> {
    let name = &task.name;
    let ended = matches!(
        entry,
        EntryBody::Agent {
            event: AgentEvent::Ended,
            ..
        }
    );
    if ended && of_a_run(ctx, task).await {
        return None;
    }
    match entry {
        EntryBody::NeedsYou { question, .. } => {
            Some(format!("task {name} needs you, its agent asks: {question}"))
        }
        EntryBody::Agent {
            event: AgentEvent::Failed,
            text,
            ..
        } => Some(format!("task {name} failed: {text}")),
        EntryBody::Agent {
            event: AgentEvent::Ended,
            text,
            ..
        } => {
            let last = said(ctx, task).await;
            if told_before(ctx, task, last.as_deref()).await {
                return None;
            }
            Some(match last {
                Some(last) => {
                    format!("task {name} ended ({text}). its agent's last message: {last}")
                }
                None => format!("task {name} ended ({text})"),
            })
        }
        _ => None,
    }
}

const LAST_MARKER: &str = ". its agent's last message: ";

fn same_end(told: &str, name: &str, last: Option<&str>) -> bool {
    if !told.starts_with(&format!("task {name} ended (")) {
        return false;
    }
    match last {
        Some(l) => told.ends_with(&format!("{LAST_MARKER}{l}")),
        None => !told.contains(LAST_MARKER),
    }
}

async fn told_before(ctx: &Context, task: &Task, last: Option<&str>) -> bool {
    let entries = match ctx.store() {
        Ok(store) => store.entries(&task.conversation_id).await,
        Err(e) => Err(e),
    };
    let mut texts = ctx.notes().waiting(&task.conversation_id);
    match entries {
        Ok(entries) => texts.extend(entries.into_iter().filter_map(|e| match e.body {
            EntryBody::Event { text } => Some(text),
            _ => None,
        })),
        Err(e) => {
            tracing::warn!(
                target: "townhall::agent", event = "agent.told_unread", task = %task.name,
                error = %crate::redact::redact(&e.to_string()),
                "could not read what the planner was already told"
            );
            return false;
        }
    }
    texts.iter().any(|t| same_end(t, &task.name, last))
}

async fn of_a_run(ctx: &Context, task: &Task) -> bool {
    let found = match ctx.store() {
        Ok(store) => store.run_task_names().await,
        Err(e) => Err(e),
    };
    match found {
        Ok(names) => names.contains(&task.name.to_lowercase()),
        Err(e) => {
            tracing::warn!(
                target: "townhall::agent", event = "agent.run_tasks_unread", task = %task.name,
                error = %crate::redact::redact(&e.to_string()),
                "could not tell whether a task belongs to a run"
            );
            false
        }
    }
}

pub(super) async fn hook_for(
    ctx: &Context,
    task: &Task,
    entry: &EntryBody,
) -> Option<(&'static str, Value)> {
    match entry {
        EntryBody::NeedsYou {
            question, choices, ..
        } => Some((
            "task.needs_you",
            json!({"task": task, "question": question, "choices": choices}),
        )),
        EntryBody::Agent {
            event: AgentEvent::Failed,
            text,
            ..
        } => Some((
            "task.failed",
            json!({"task": task, "reason": text, "phase": "run"}),
        )),
        EntryBody::Agent {
            event: AgentEvent::Ended,
            ..
        } => Some((
            "task.ended",
            json!({"task": task, "lastMessage": said(ctx, task).await}),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_message_is_cut_with_an_ellipsis() {
        assert_eq!(capped("  done \n"), "done");
        let long = "x".repeat(MAX_SAID_CHARS + 5);
        let cut = capped(&long);
        assert_eq!(cut.chars().count(), MAX_SAID_CHARS);
        assert!(cut.ends_with('\u{2026}'));
    }

    #[test]
    fn the_same_end_matches_across_parentheticals() {
        let told = "task fix ended (ended). its agent's last message: done";
        assert!(same_end(told, "fix", Some("done")));
        let removed = "task fix ended (the agent was removed). its agent's last message: done";
        assert!(same_end(removed, "fix", Some("done")));
    }

    #[test]
    fn another_last_message_is_not_the_same_end() {
        let told = "task fix ended (ended). its agent's last message: done";
        assert!(!same_end(told, "fix", Some("done more")));
        assert!(!same_end(told, "fix", Some("one")));
    }

    #[test]
    fn another_task_is_not_the_same_end() {
        let told = "task fix-2 ended (ended). its agent's last message: done";
        assert!(!same_end(told, "fix", Some("done")));
    }

    #[test]
    fn no_last_message_matches_only_a_text_without_one() {
        assert!(same_end("task fix ended (ended)", "fix", None));
        let told = "task fix ended (ended). its agent's last message: done";
        assert!(!same_end(told, "fix", None));
        assert!(!same_end("task fix ended (ended)", "fix", Some("done")));
    }
}
