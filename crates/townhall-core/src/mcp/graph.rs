use std::time::SystemTime;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::clock::unix_millis;
use crate::context::Context;
use crate::logs::parse_since;
use crate::redact::redact;
use crate::store::{AgentStat, GraphAnswer, GraphQuery, GraphTask, NodeKind};

use super::checks::Checked;
use super::ToolDef;

pub const HISTORY_GRAPH: &str = "history_graph";
pub const AGENT_STATS: &str = "agent_stats";

pub const DEFAULT_LIMIT: usize = 10;
pub const MAX_LIMIT: usize = 50;
pub const DEFAULT_STATS_SINCE: &str = "30d";

pub fn tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: HISTORY_GRAPH,
            description: "Ask townhall's history graph what is connected to one node, without loading old chats. A node is a file path (a suffix such as `insights/mod.rs` or a glob such as `src/store/*` works), a task name, a conversation id, a branch (`townhall/<task>`), a pull request (`<repo>#<number>`), an agent type, an outcome (merged, failed, api-error, needs-user, ended, in-review) or a log fingerprint (from log_summary). Returns the tasks linked to it, newest first, each with its conversation, agent type, outcome, branch, pr, date and the retries and hand backs it took. `depth` 2 adds each task's changed files and log fingerprints. Use it before touching an area that has a history. Open a conversation with read_conversation. Read-only.",
            input_schema: graph_schema(),
            read_only: true,
        },
        ToolDef {
            name: AGENT_STATS,
            description: "How each agent type performed over a period: tasks, outcomes, average retries, hand backs it made, its most common failure reasons and the areas (folders) its failed tasks changed most. Compact. Call it when asked how agents perform or which type to trust with a job. Read-only.",
            input_schema: stats_schema(),
            read_only: true,
        },
    ]
}

fn graph_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "node": { "type": "string", "minLength": 1, "description": "The node to look up: a file path or glob, task, conversation id, branch, pr, agent type, outcome or fingerprint." },
            "kind": {
                "type": "string",
                "enum": ["file", "task", "conversation", "branch", "pr", "agent", "outcome", "fingerprint"],
                "description": "Say what the node is when the guess could be wrong. Default: guessed in that order of likelihood."
            },
            "depth": { "type": "integer", "minimum": 1, "maximum": 2, "description": "1 (default): the linked tasks. 2: also each task's files and fingerprints." },
            "since": { "type": "string", "description": "Only tasks newer than: a duration like 7d or 24h, or an RFC 3339 time. Default: all time." },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIMIT, "description": format!("Tasks to return, default {DEFAULT_LIMIT}.") }
        },
        "required": ["node"],
        "additionalProperties": false
    })
}

fn stats_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "since": { "type": "string", "description": format!("Tasks created since: a duration like 7d or 24h, or an RFC 3339 time. Default {DEFAULT_STATS_SINCE}.") },
            "agent": { "type": "string", "description": "Only this agent type." }
        },
        "additionalProperties": false
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GraphInput {
    node: String,
    #[serde(default)]
    kind: Option<NodeKind>,
    #[serde(default)]
    depth: Option<u8>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StatsInput {
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    agent: Option<String>,
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

fn task_json(task: GraphTask) -> Value {
    let mut map = Map::new();
    map.insert("task".into(), json!(task.name));
    map.insert("title".into(), json!(clean(&task.title)));
    map.insert("conversation".into(), json!(task.conversation));
    map.insert(
        "conversationTitle".into(),
        json!(clean(&task.conversation_title)),
    );
    map.insert("agent".into(), json!(task.agent));
    if let Some(outcome) = task.outcome {
        map.insert("outcome".into(), json!(outcome));
    }
    map.insert("branch".into(), json!(task.branch));
    if let Some(pr) = task.pr {
        map.insert("pr".into(), json!(pr));
    }
    map.insert("at".into(), json!(task.at));
    if task.retries > 0 {
        map.insert("retries".into(), json!(task.retries));
    }
    if task.handbacks > 0 {
        map.insert("handBacks".into(), json!(task.handbacks));
    }
    if !task.files.is_empty() {
        map.insert("files".into(), json!(task.files));
    }
    if !task.fingerprints.is_empty() {
        map.insert("fingerprints".into(), json!(task.fingerprints));
    }
    Value::Object(map)
}

fn answer_json(answer: GraphAnswer) -> Value {
    json!({
        "node": { "kind": answer.kind.name(), "id": clean(&answer.node) },
        "matched": answer.matched,
        "tasks": answer.tasks.into_iter().map(task_json).collect::<Vec<_>>()
    })
}

pub(super) async fn graph(ctx: &Context, input: GraphInput) -> Checked<Value> {
    let node = input.node.trim().to_string();
    if node.is_empty() {
        return Err("`node` must not be blank".into());
    }
    let depth = input.depth.unwrap_or(1);
    if !(1..=2).contains(&depth) {
        return Err("`depth` must be 1 or 2".into());
    }
    let since_ms = input
        .since
        .as_deref()
        .map(|s| parse_since(s, now_ms()))
        .transpose()?;
    let query = GraphQuery {
        kind: input.kind,
        node: node.clone(),
        since_ms,
        limit: input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT),
        deep: depth == 2,
    };
    let answer = ctx
        .store()
        .map_err(text)?
        .history_graph(query)
        .await
        .map_err(text)?;
    match answer {
        Some(answer) => Ok(answer_json(answer)),
        None => Err(format!(
            "the history graph has no node `{}`: try search_history for words, or give `kind`",
            clean(&node)
        )),
    }
}

fn counted(pairs: &[(String, u64)], key: &str) -> Value {
    json!(pairs
        .iter()
        .map(|(name, count)| json!({ key: clean(name), "count": count }))
        .collect::<Vec<_>>())
}

fn stat_json(stat: &AgentStat) -> Value {
    let average = (stat.avg_retries * 100.0).round() / 100.0;
    json!({
        "agent": stat.agent,
        "tasks": stat.tasks,
        "outcomes": stat.outcomes,
        "avgRetries": average,
        "handBacks": stat.handbacks,
        "failures": counted(&stat.failures, "reason"),
        "failsOn": counted(&stat.areas, "area")
    })
}

pub(super) async fn stats(ctx: &Context, input: StatsInput) -> Checked<Value> {
    let since = input.since.as_deref().unwrap_or(DEFAULT_STATS_SINCE);
    let since_ms = parse_since(since, now_ms())?;
    let found = ctx
        .store()
        .map_err(text)?
        .agent_stats(Some(since_ms))
        .await
        .map_err(text)?;
    let agents: Vec<Value> = found
        .iter()
        .filter(|s| {
            input
                .agent
                .as_deref()
                .is_none_or(|a| s.agent.eq_ignore_ascii_case(a.trim()))
        })
        .map(stat_json)
        .collect();
    Ok(json!({ "since": since, "agents": agents }))
}
