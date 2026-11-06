//! `task_status`: what this conversation's tasks are doing and did, so townhall answers
//! from facts instead of guessing. Without a name: every task with its latest `agent` and
//! `needsYou` entries. With a name: that task plus its agent's last steps (2.7) and, once
//! it ended or merged, what its agent said last in its transcript.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agent::state;
use crate::context::Context;
use crate::model::{ArtifactKind, AuthorRole, EntryBody, Task, TaskState, ThreadEntry};
use crate::redact::redact;
use crate::steps::{self, Step};

use super::artifacts;
use super::checks::Checked;
use super::{MAX_NAME_LEN, NAME_PATTERN};

/// The `agent` and `needsYou` entries each task shows, newest last.
pub const RECENT_ENTRIES: usize = 3;
/// The steps a named task shows, newest last.
pub const MAX_STEPS: usize = 20;
/// The longest last message returned, in characters.
pub const MAX_MESSAGE_CHARS: usize = 2000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StatusInput {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskStatus {
    name: String,
    title: String,
    repo: String,
    branch: String,
    state: TaskState,
    after: Vec<String>,
    /// The agent type it runs as.
    agent: String,
    /// Why a queued task does not start although its `after` tasks finished.
    blocked: Option<String>,
    entries: Vec<ThreadEntry>,
    /// Its proof artifacts, oldest first.
    artifacts: Vec<ArtifactRef>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactRef {
    name: String,
    kind: ArtifactKind,
    author_role: AuthorRole,
    url: String,
}

async fn proof(ctx: &Context, task: &Task) -> Checked<Vec<ArtifactRef>> {
    let found = ctx
        .store()
        .map_err(text)?
        .artifacts(&task.name)
        .await
        .map_err(text)?;
    Ok(found
        .into_iter()
        .map(|a| ArtifactRef {
            url: artifacts::url(ctx.base_url(), &a),
            name: a.name,
            kind: a.kind,
            author_role: a.author_role,
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskDetail {
    #[serde(flatten)]
    task: TaskStatus,
    steps: Vec<Step>,
    last_message: Option<String>,
}

pub(super) fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string", "pattern": NAME_PATTERN, "minLength": 1, "maxLength": MAX_NAME_LEN,
                "description": "A task of this conversation. Without it: every task of this conversation."
            }
        },
        "additionalProperties": false
    })
}

fn text(e: crate::Error) -> String {
    e.to_string()
}

/// The answer for the conversation `conversation_id`.
pub(super) async fn task_status(
    ctx: &Context,
    conversation_id: &str,
    input: StatusInput,
) -> Checked<Value> {
    let store = ctx.store().map_err(text)?;
    let tasks = store.tasks(conversation_id).await.map_err(text)?;
    let entries = store.entries(conversation_id).await.map_err(text)?;
    let Some(name) = input.name else {
        let mut all = Vec::with_capacity(tasks.len());
        for task in &tasks {
            all.push(status(task, &entries, proof(ctx, task).await?));
        }
        return Ok(json!({ "tasks": all }));
    };
    let name = name.trim();
    if name.is_empty() {
        return Err("`name` must not be blank: leave it out to list every task".into());
    }
    let Some(task) = tasks.iter().find(|t| t.name.eq_ignore_ascii_case(name)) else {
        let elsewhere = store.find_task(name).await.map_err(text)?.is_some();
        return Err(not_here(name, elsewhere, &tasks));
    };
    let detail = detail(ctx, task, &entries, proof(ctx, task).await?).await;
    serde_json::to_value(detail).map_err(|e| e.to_string())
}

fn not_here(name: &str, elsewhere: bool, tasks: &[Task]) -> String {
    let names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
    let listed = match names.is_empty() {
        true => "this conversation has no tasks".to_string(),
        false => format!("this conversation's tasks are {}", names.join(", ")),
    };
    match elsewhere {
        true => format!("task `{name}` belongs to another conversation: {listed}"),
        false => format!("no task named `{name}`: {listed}"),
    }
}

fn status(task: &Task, entries: &[ThreadEntry], artifacts: Vec<ArtifactRef>) -> TaskStatus {
    let mine: Vec<&ThreadEntry> = entries
        .iter()
        .filter(|e| match &e.body {
            EntryBody::Agent { agent, .. } | EntryBody::NeedsYou { agent, .. } => {
                agent.eq_ignore_ascii_case(&task.name)
            }
            _ => false,
        })
        .collect();
    let recent = mine.len().saturating_sub(RECENT_ENTRIES);
    TaskStatus {
        name: task.name.clone(),
        title: task.title.clone(),
        repo: task.repo.clone(),
        branch: task.branch(),
        state: task.state,
        after: task.after.clone(),
        agent: task.agent.clone(),
        blocked: task.blocked.clone(),
        entries: mine.into_iter().skip(recent).cloned().collect(),
        artifacts,
    }
}

async fn detail(
    ctx: &Context,
    task: &Task,
    entries: &[ThreadEntry],
    artifacts: Vec<ArtifactRef>,
) -> TaskDetail {
    let found = match transcripts_of(ctx, task).await {
        Ok(found) => Some(found),
        Err(e) => {
            tracing::warn!(
                target: "townhall::mcp", event = "mcp.status_no_transcript",
                task = %task.name, error_kind = e.kind(), error = %redact(&e.to_string()),
                "task_status: no transcript"
            );
            None
        }
    };
    let (steps, last_message) = match found {
        Some((projects, worktree)) => (
            last_steps(&projects, &worktree, task).await,
            last_message(&projects, &worktree, task).await,
        ),
        None => (vec![], None),
    };
    TaskDetail {
        task: status(task, entries, artifacts),
        steps,
        last_message,
    }
}

/// Claude's projects folder and the task's worktree, whether or not it still exists.
async fn transcripts_of(ctx: &Context, task: &Task) -> crate::Result<(PathBuf, PathBuf)> {
    let repo = ctx.repo(&task.repo).await?;
    let worktree = ctx.workspace(&repo)?.worktrees_dir().join(&task.name);
    Ok((steps::claude_projects_dir()?, worktree))
}

async fn last_steps(projects: &Path, worktree: &Path, task: &Task) -> Vec<Step> {
    match steps::steps_for_agent(projects, worktree).await {
        Ok(all) => {
            let skip = all.len().saturating_sub(MAX_STEPS);
            all.into_iter().skip(skip).collect()
        }
        Err(e) => {
            tracing::warn!(
                target: "townhall::mcp", event = "mcp.status_no_steps",
                task = %task.name, error_kind = e.kind(), error = %redact(&e.to_string()),
                "task_status: no steps"
            );
            vec![]
        }
    }
}

async fn last_message(projects: &Path, worktree: &Path, task: &Task) -> Option<String> {
    if !matches!(task.state, TaskState::Ended | TaskState::Merged) {
        return None;
    }
    match state::last_message(projects, worktree).await {
        Ok(said) => said.map(|s| capped(&s)),
        Err(e) => {
            tracing::warn!(
                target: "townhall::mcp", event = "mcp.status_no_last_message",
                task = %task.name, error_kind = e.kind(), error = %redact(&e.to_string()),
                "task_status: no last message"
            );
            None
        }
    }
}

/// At most [`MAX_MESSAGE_CHARS`] characters, the last one `…` when it was cut.
fn capped(message: &str) -> String {
    if message.chars().count() <= MAX_MESSAGE_CHARS {
        return message.to_string();
    }
    let kept: String = message.chars().take(MAX_MESSAGE_CHARS - 1).collect();
    format!("{kept}\u{2026}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_message_is_cut_to_the_cap() {
        assert_eq!(capped("done"), "done");
        let exact = "a".repeat(MAX_MESSAGE_CHARS);
        assert_eq!(capped(&exact), exact);
        let long = "\u{e9}".repeat(MAX_MESSAGE_CHARS + 5);
        let cut = capped(&long);
        assert_eq!(cut.chars().count(), MAX_MESSAGE_CHARS);
        assert!(cut.ends_with('\u{2026}'));
    }

    #[test]
    fn a_missing_task_says_where_to_look() {
        assert_eq!(
            not_here("x", false, &[]),
            "no task named `x`: this conversation has no tasks"
        );
        assert!(not_here("x", true, &[]).contains("another conversation"));
    }

    #[test]
    fn the_input_takes_only_a_name() {
        let none: StatusInput = serde_json::from_value(json!({})).unwrap();
        assert!(none.name.is_none());
        assert!(serde_json::from_value::<StatusInput>(json!({"task": "x"})).is_err());
    }
}
