//! `conversation_insights`: the planner reads the recurring patterns of every conversation.
//! The analysis is `crate::insights`; this file has the tool schema and loads the data from
//! the store. Read-only. Only townhall's own session gets it, never a task's agent.

use std::time::SystemTime;

use serde_json::{json, Value};

use crate::clock::unix_millis;
use crate::context::Context;
use crate::insights::{
    self, ConversationData, InsightData, InsightsArgs, TaskData, DEFAULT_SINCE, DEFAULT_TOP,
    MAX_TOP,
};

use super::checks::Checked;
use super::ToolDef;

pub const CONVERSATION_INSIGHTS: &str = "conversation_insights";

pub fn tools() -> Vec<ToolDef> {
    vec![ToolDef {
        name: CONVERSATION_INSIGHTS,
        description: "Recurring patterns across every conversation of townhall (not just this one), counted without a model: repeated user requests, questions agents ask you, agent failures, steps typed into agents by hand, hand back sequences, task chains by agent type and townhall errors, with tasks per agent type and their end states. Each pattern has count, first and last seen, up to 3 samples, conversation and task ids, a stable `fingerprint` to cite in a task brief and a `suggested_automation` hint. Only patterns seen at least twice. Read-only. Use it when asked what to automate or improve in how work gets done.",
        input_schema: schema(),
    }]
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "since": { "type": "string", "description": format!("How far back: a duration like 7d or 24h, or an RFC 3339 time. Default {DEFAULT_SINCE}.") },
            "top": { "type": "integer", "minimum": 1, "maximum": MAX_TOP, "description": format!("Patterns to return, default {DEFAULT_TOP}.") }
        },
        "additionalProperties": false
    })
}

fn now_ms() -> i64 {
    unix_millis(SystemTime::now())
}

async fn load(ctx: &Context) -> Checked<InsightData> {
    let store = ctx.store().map_err(|e| e.to_string())?;
    let mut data = InsightData::default();
    for c in store.conversations().await.map_err(|e| e.to_string())? {
        let entries = store.entries(&c.id).await.map_err(|e| e.to_string())?;
        data.conversations
            .push(ConversationData { id: c.id, entries });
    }
    let mut ended = store.task_ended_times().await.map_err(|e| e.to_string())?;
    for task in store.all_tasks().await.map_err(|e| e.to_string())? {
        let handoffs = store
            .handoffs(&task.name)
            .await
            .map_err(|e| e.to_string())?;
        let ended_at = ended.remove(&task.name);
        data.tasks.push(TaskData {
            task,
            handoffs,
            ended_at,
        });
    }
    Ok(data)
}

pub(super) async fn insights(ctx: &Context, args: InsightsArgs) -> Checked<Value> {
    let data = load(ctx).await?;
    let out = tokio::task::spawn_blocking(move || insights::analyze(&data, now_ms(), &args))
        .await
        .map_err(|e| format!("analysing conversations failed: {e}"))??;
    serde_json::to_value(out).map_err(|e| e.to_string())
}
