use std::time::SystemTime;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::clock::unix_millis;
use crate::context::Context;
use crate::logs::parse_since;
use crate::model::{EntryBody, Task, ThreadEntry};
use crate::redact::redact;
use crate::store::{HistoryHit, HistoryKind, HistoryQuery};

use super::checks::Checked;
use super::ToolDef;

pub const SEARCH_HISTORY: &str = "search_history";
pub const READ_CONVERSATION: &str = "read_conversation";

pub const DEFAULT_LIMIT: usize = 10;
pub const MAX_LIMIT: usize = 50;
pub const DEFAULT_MESSAGES: usize = 30;
pub const MAX_MESSAGES: usize = 100;
pub const MAX_TEXT_CHARS: usize = 600;

const REPORT_MARKER: &str = "its agent's last message: ";

pub fn tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: SEARCH_HISTORY,
            description: "Full-text search over every old conversation of townhall, without loading them: your and the planner's messages, task titles and briefs, what agents reported when they ended or failed, and what they asked the user. Plain words just work (stemmed, all must match); FTS5 syntax is accepted too (OR, NOT, \"phrases\", prefix*). Returns ranked hits, best first, each with a short snippet (the match in [brackets]), the conversation id and title, and for task hits the task name, agent type, state, branch and pr. Call it before planning work that may have been done or rejected before. Open a hit's conversation with read_conversation. Read-only.",
            input_schema: search_schema(),
            read_only: true,
        },
        ToolDef {
            name: READ_CONVERSATION,
            description: "Read one old conversation compactly by id (from search_history): its title and dates, the user's and the planner's messages (long ones trimmed, `offset` and `limit` page through them) and each task with its agent type, state, branch, pr and the agent's last message, trimmed. Never raw tool output. Read-only.",
            input_schema: read_schema(),
            read_only: true,
        },
    ]
}

fn search_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": { "type": "string", "minLength": 1, "description": "What to look for: plain words, or FTS5 syntax." },
            "repo": { "type": "string", "description": "Only conversations linked to this repo or with a task in it." },
            "since": { "type": "string", "description": "Only newer than: a duration like 7d or 24h, or an RFC 3339 time. Default: all time." },
            "kind": {
                "type": "array",
                "items": { "type": "string", "enum": ["message", "task", "agent_report", "needs_you"] },
                "description": "Only these kinds of hit. Default: all."
            },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": format!("Hits to return, default {DEFAULT_LIMIT}.") }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

fn read_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "minLength": 1, "description": "The conversation id." },
            "offset": { "type": "integer", "minimum": 0, "description": "Messages to skip, default 0." },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_MESSAGES, "description": format!("Messages to return, default {DEFAULT_MESSAGES}.") }
        },
        "required": ["id"],
        "additionalProperties": false
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchInput {
    query: String,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    kind: Vec<HistoryKind>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadInput {
    id: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

fn text(e: crate::Error) -> String {
    e.to_string()
}

fn now_ms() -> i64 {
    unix_millis(SystemTime::now())
}

fn clean(value: &str) -> String {
    redact(value).into_owned()
}

fn trimmed(value: &str) -> String {
    let value = value.trim();
    match value.chars().count() <= MAX_TEXT_CHARS {
        true => clean(value),
        false => {
            let kept: String = value.chars().take(MAX_TEXT_CHARS - 1).collect();
            format!("{}\u{2026}", clean(&kept))
        }
    }
}

fn put(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        map.insert(key.into(), value);
    }
}

fn hit_json(hit: HistoryHit) -> Value {
    let mut map = Map::new();
    map.insert("kind".into(), json!(hit.kind.name()));
    map.insert("conversation".into(), json!(hit.conversation_id));
    map.insert("title".into(), json!(clean(&hit.title)));
    put(&mut map, "task", hit.task.map(Value::from));
    put(&mut map, "agent", hit.agent.map(Value::from));
    put(&mut map, "state", hit.state.map(|s| json!(s)));
    put(&mut map, "branch", hit.branch.map(Value::from));
    put(&mut map, "pr", hit.pr_number.map(Value::from));
    put(&mut map, "prUrl", hit.pr_url.map(Value::from));
    map.insert("at".into(), json!(hit.at));
    map.insert("snippet".into(), json!(clean(&hit.snippet)));
    Value::Object(map)
}

pub(super) async fn search(ctx: &Context, input: SearchInput) -> Checked<Value> {
    if input.query.trim().is_empty() {
        return Err("`query` must not be blank".into());
    }
    let since_ms = input
        .since
        .as_deref()
        .map(|s| parse_since(s, now_ms()))
        .transpose()?;
    let query = HistoryQuery {
        query: input.query,
        repo: input.repo.filter(|r| !r.trim().is_empty()),
        since_ms,
        kinds: input.kind,
        limit: input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
    };
    let hits = ctx
        .store()
        .map_err(text)?
        .search_history(query)
        .await
        .map_err(text)?;
    Ok(json!({ "hits": hits.into_iter().map(hit_json).collect::<Vec<_>>() }))
}

fn last_report(task: &Task, entries: &[ThreadEntry]) -> Option<String> {
    let ended = format!("task {} ended (", task.name).to_lowercase();
    let failed = format!("task {} failed: ", task.name).to_lowercase();
    let prefix = format!("task {} ", task.name).len();
    entries.iter().rev().find_map(|e| match &e.body {
        EntryBody::Event { text } => {
            let lower = text.to_lowercase();
            match lower.starts_with(&ended) || lower.starts_with(&failed) {
                true => Some(match text.split_once(REPORT_MARKER) {
                    Some((_, said)) => trimmed(said),
                    None => trimmed(text.get(prefix..).unwrap_or(text)),
                }),
                false => None,
            }
        }
        _ => None,
    })
}

fn task_json(task: &Task, entries: &[ThreadEntry]) -> Value {
    let mut map = Map::new();
    map.insert("name".into(), json!(task.name));
    map.insert("title".into(), json!(clean(&task.title)));
    map.insert("agent".into(), json!(task.agent));
    map.insert("state".into(), json!(task.state));
    map.insert("branch".into(), json!(task.branch()));
    put(
        &mut map,
        "pr",
        task.pr.as_ref().map(|pr| Value::from(pr.number)),
    );
    put(
        &mut map,
        "prUrl",
        task.pr
            .as_ref()
            .and_then(|pr| pr.url.clone())
            .map(Value::from),
    );
    put(
        &mut map,
        "lastMessage",
        last_report(task, entries).map(Value::from),
    );
    Value::Object(map)
}

fn message_json(entry: &ThreadEntry) -> Option<Value> {
    let (by, said) = match &entry.body {
        EntryBody::You { text, .. } => ("you", text),
        EntryBody::Townhall { text } => ("planner", text),
        _ => return None,
    };
    Some(json!({ "at": entry.at, "by": by, "text": trimmed(said) }))
}

pub(super) async fn read(ctx: &Context, input: ReadInput) -> Checked<Value> {
    let store = ctx.store().map_err(text)?;
    let id = input.id.trim();
    let conversation = store.conversation(id).await.map_err(text)?;
    let entries = store.entries(id).await.map_err(text)?;
    let tasks = store.tasks(id).await.map_err(text)?;
    let messages: Vec<Value> = entries.iter().filter_map(message_json).collect();
    let total = messages.len();
    let offset = input.offset.unwrap_or(0);
    let limit = input
        .limit
        .unwrap_or(DEFAULT_MESSAGES)
        .clamp(1, MAX_MESSAGES);
    let page: Vec<Value> = messages.into_iter().skip(offset).take(limit).collect();
    let mut out = Map::new();
    out.insert("id".into(), json!(conversation.id));
    out.insert("title".into(), json!(clean(&conversation.title)));
    put(&mut out, "repo", conversation.linked_repo.map(Value::from));
    out.insert("created".into(), json!(conversation.created_at));
    out.insert("updated".into(), json!(conversation.updated_at));
    out.insert("total".into(), json!(total));
    out.insert("offset".into(), json!(offset));
    out.insert("messages".into(), json!(page));
    out.insert(
        "tasks".into(),
        json!(tasks
            .iter()
            .map(|t| task_json(t, &entries))
            .collect::<Vec<_>>()),
    );
    Ok(Value::Object(out))
}
