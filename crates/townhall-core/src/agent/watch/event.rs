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
        } => Some(match said(ctx, task).await {
            Some(last) => format!("task {name} ended ({text}). its agent's last message: {last}"),
            None => format!("task {name} ended ({text})"),
        }),
        _ => None,
    }
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
}
