//! `add_artifact` and `list_artifacts`: the proof tools of a task's agent (its own endpoint,
//! `/api/tasks/{name}/mcp`). An agent adds only to its own task and reads any task of its own
//! conversation; there is no tool to replace or delete one.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::context::Context;
use crate::model::{
    Artifact, ArtifactKind, AuthorRole, NewArtifact, Task, MAX_ARTIFACT_BYTES, MAX_CAPTION_CHARS,
};

use super::checks::Checked;
use super::{ToolDef, MAX_NAME_LEN, NAME_PATTERN};

pub const ADD_ARTIFACT: &str = "add_artifact";
pub const LIST_ARTIFACTS: &str = "list_artifacts";

const MAX_FILE_NAME_LEN: u64 = 120;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Encoding {
    #[default]
    Text,
    Base64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AddInput {
    task: String,
    name: String,
    kind: ArtifactKind,
    content: String,
    #[serde(default)]
    encoding: Encoding,
    author_role: AuthorRole,
    #[serde(default)]
    caption: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListInput {
    task: String,
}

fn wire_names<T: Serialize>(all: &[T]) -> Vec<Value> {
    all.iter()
        .filter_map(|v| serde_json::to_value(v).ok())
        .collect()
}

pub fn tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: ADD_ARTIFACT,
            description: "Attach one piece of proof to your own task: a screenshot (png, jpeg, gif or webp as base64), a log, a note or a report (text). It is stored outside the repo, shown in townhall's Proof section grouped by your role, and cannot be changed or deleted afterwards. Returns the artifact id and its URL.",
            input_schema: add_schema(),
        },
        ToolDef {
            name: LIST_ARTIFACTS,
            description: "The proof on a task of this conversation, oldest first: id, name, kind, author_role, caption, created and url. Name the task whose proof you want to read, for example the builder's.",
            input_schema: list_schema(),
        },
    ]
}

fn task_property() -> Value {
    json!({
        "type": "string", "pattern": NAME_PATTERN, "minLength": 1, "maxLength": MAX_NAME_LEN
    })
}

fn add_schema() -> Value {
    let content_chars = MAX_ARTIFACT_BYTES / 3 * 4 + 8;
    json!({
        "type": "object",
        "properties": {
            "task": {
                "type": "string", "pattern": NAME_PATTERN, "minLength": 1, "maxLength": MAX_NAME_LEN,
                "description": "Your own task's name."
            },
            "name": {
                "type": "string", "minLength": 1, "maxLength": MAX_FILE_NAME_LEN,
                "description": "File name, for example `login-after.png` or `test-run.log`. Folders are dropped."
            },
            "kind": { "type": "string", "enum": wire_names(&ArtifactKind::ALL) },
            "content": {
                "type": "string", "minLength": 1, "maxLength": content_chars,
                "description": format!("The text, or base64 when `encoding` is base64. At most {} KB once decoded.", MAX_ARTIFACT_BYTES / 1024)
            },
            "encoding": {
                "type": "string", "enum": ["text", "base64"],
                "description": "Default text. A screenshot must be base64."
            },
            "author_role": { "type": "string", "enum": wire_names(&AuthorRole::ALL) },
            "caption": {
                "type": "string", "maxLength": MAX_CAPTION_CHARS,
                "description": "One line saying what this proves."
            }
        },
        "required": ["task", "name", "kind", "content", "author_role"],
        "additionalProperties": false
    })
}

fn list_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "task": task_property() },
        "required": ["task"],
        "additionalProperties": false
    })
}

fn text(e: crate::Error) -> String {
    e.to_string()
}

pub(super) fn url(base_url: &str, artifact: &Artifact) -> String {
    format!(
        "{}/api/tasks/{}/artifacts/{}",
        base_url.trim_end_matches('/'),
        artifact.task,
        artifact.id
    )
}

fn row(base_url: &str, a: &Artifact) -> Value {
    json!({
        "id": a.id,
        "name": a.name,
        "kind": a.kind,
        "author_role": a.author_role,
        "caption": a.caption,
        "created": a.created_at,
        "url": url(base_url, a),
    })
}

fn bytes_of(input: &AddInput) -> Checked<Vec<u8>> {
    let too_big = || format!("an artifact is at most {} KB", MAX_ARTIFACT_BYTES / 1024);
    let limit = usize::try_from(MAX_ARTIFACT_BYTES).unwrap_or(usize::MAX);
    match input.encoding {
        Encoding::Text if input.content.len() > limit => Err(too_big()),
        Encoding::Text => Ok(input.content.clone().into_bytes()),
        Encoding::Base64 if input.content.len() > limit / 3 * 4 + 8 => Err(too_big()),
        Encoding::Base64 => {
            let packed: String = input.content.split_whitespace().collect();
            STANDARD
                .decode(packed)
                .map_err(|e| format!("`content` is not valid base64: {e}"))
        }
    }
}

async fn own(ctx: &Context, task: &str) -> Checked<Task> {
    ctx.store()
        .map_err(text)?
        .find_task(task)
        .await
        .map_err(text)?
        .ok_or_else(|| format!("no task named `{task}`"))
}

pub(super) async fn add(ctx: &Context, own_task: &str, input: AddInput) -> Checked<Value> {
    let mine = own(ctx, own_task).await?;
    if !input.task.trim().eq_ignore_ascii_case(&mine.name) {
        return Err(format!(
            "you can add proof only to your own task `{}`, not `{}`",
            mine.name,
            input.task.trim()
        ));
    }
    let new = NewArtifact {
        bytes: bytes_of(&input)?,
        name: input.name,
        kind: input.kind,
        author_role: input.author_role,
        caption: input.caption,
    };
    let saved = ctx
        .store()
        .map_err(text)?
        .add_artifact(&mine.name, new)
        .await
        .map_err(text)?;
    Ok(json!({
        "id": saved.id,
        "name": saved.name,
        "kind": saved.kind,
        "mime": saved.mime,
        "size": saved.size,
        "url": url(ctx.base_url(), &saved),
    }))
}

pub(super) async fn list(ctx: &Context, own_task: &str, input: ListInput) -> Checked<Value> {
    let store = ctx.store().map_err(text)?;
    let mine = own(ctx, own_task).await?;
    let siblings = store.tasks(&mine.conversation_id).await.map_err(text)?;
    let wanted = input.task.trim();
    let Some(task) = siblings
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(wanted))
    else {
        let names: Vec<&str> = siblings.iter().map(|t| t.name.as_str()).collect();
        return Err(format!(
            "no task named `{wanted}` in this conversation: its tasks are {}",
            names.join(", ")
        ));
    };
    let found = store.artifacts(&task.name).await.map_err(text)?;
    let rows: Vec<Value> = found.iter().map(|a| row(ctx.base_url(), a)).collect();
    Ok(json!({ "task": task.name, "artifacts": rows }))
}
