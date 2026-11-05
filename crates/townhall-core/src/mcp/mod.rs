//! Townhall's MCP server for its own Claude session: JSON-RPC 2.0 over plain HTTP JSON
//! (no SSE) at `POST /api/conversations/{id}/mcp`, like Hive's `src/main/plan/mcp.ts`.
//! This module fixes the names, the tool schemas and the `--mcp-config` the runner (2.2)
//! passes; `handle` answers the requests (`rpc.rs`) with the task tools (`task_tools.rs`,
//! `status.rs`).
//! Units 2.8b and 2.8c add their tools in `library.rs` and `runs.rs`, and a match arm for
//! each in `TaskTools::call`. The agent type tools (2.12) live in `library.rs` and `ask.rs`.

mod artifacts;
mod ask;
mod checks;
mod hand_back;
mod library;
mod logs;
mod rpc;
mod runs;
mod status;
mod task_tools;

use serde_json::{json, Value};

use crate::context::Context;

pub use artifacts::{ADD_ARTIFACT, LIST_ARTIFACTS};
pub use hand_back::HAND_BACK;
pub use library::{LIST_AGENT_TYPES, READ_AGENT_TYPE, SAVE_AGENT_TYPE};
pub use rpc::{INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND};
pub use status::{MAX_MESSAGE_CHARS, MAX_STEPS, RECENT_ENTRIES};

/// Answers one JSON-RPC message or batch for the conversation `conversation_id` (which the
/// route checked exists). `None` when there is nothing to send back: the route answers `202`.
pub async fn handle(ctx: &Context, conversation_id: &str, body: Value) -> Option<Value> {
    let tools = task_tools::TaskTools {
        ctx,
        conversation_id,
    };
    rpc::answer(body, &tools).await
}

/// Answers one JSON-RPC message or batch for the task `task` (which the route checked
/// exists): the task agent's tools, only `hand_back`.
pub async fn handle_task(ctx: &Context, task: &str, body: Value) -> Option<Value> {
    rpc::answer(body, &task_tools::HandBackTools { ctx, task }).await
}

pub const SERVER_NAME: &str = "townhall";
pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const CREATE_TASK: &str = "create_task";
pub const SET_TITLE: &str = "set_title";
pub const TASK_STATUS: &str = "task_status";
pub const NAME_PATTERN: &str = "^[A-Za-z0-9_-]+$";
pub const MAX_NAME_LEN: u64 = 64;
pub const MAX_TITLE_LEN: u64 = 80;
pub const MAX_PROMPT_LEN: u64 = 50_000;
pub const MAX_LIST_LEN: u64 = 20;

pub const INSTRUCTIONS: &str = "townhall tools: create_task hands one piece of work to an agent of a saved agent type in its own git worktree; set_title names this conversation once the topic is clear; task_status tells what this conversation's tasks are doing and what they did; list_agent_types, read_agent_type and save_agent_type show and save the agent types tasks run as; log_summary and read_logs show townhall's own errors and warnings from its logs in ~/.townhall/logs/: when asked to improve townhall, start from log_summary and create tasks for the recurring groups, citing each group's fingerprint.";

/// One tool as `tools/list` returns it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// The URL of one conversation's MCP endpoint on this daemon.
pub fn endpoint(base_url: &str, conversation_id: &str) -> String {
    format!(
        "{}/api/conversations/{conversation_id}/mcp",
        base_url.trim_end_matches('/')
    )
}

/// The `--mcp-config` JSON for townhall's session in `conversation_id`.
pub fn config_json(base_url: &str, conversation_id: &str) -> String {
    json!({
        "mcpServers": {
            SERVER_NAME: { "type": "http", "url": endpoint(base_url, conversation_id) }
        }
    })
    .to_string()
}

/// The URL of one task's MCP endpoint on this daemon.
pub fn task_endpoint(base_url: &str, task: &str) -> String {
    format!("{}/api/tasks/{task}/mcp", base_url.trim_end_matches('/'))
}

/// The `--mcp-config` JSON for the agent of `task`.
pub fn task_config_json(base_url: &str, task: &str) -> String {
    json!({
        "mcpServers": {
            SERVER_NAME: { "type": "http", "url": task_endpoint(base_url, task) }
        }
    })
    .to_string()
}

/// Every tool a task's agent gets, in `tools/list` order.
pub fn task_tools_list() -> Vec<ToolDef> {
    [hand_back::tools(), artifacts::tools()].concat()
}

/// The tools a task's agent may use without asking.
pub fn task_allowed_tools() -> Vec<String> {
    task_tools_list()
        .iter()
        .map(|t| format!("mcp__{SERVER_NAME}__{}", t.name))
        .collect()
}

/// `--allowedTools` for townhall's session: only its own tools.
pub fn allowed_tools() -> Vec<String> {
    tools()
        .iter()
        .map(|t| format!("mcp__{SERVER_NAME}__{}", t.name))
        .collect()
}

fn names_array(description: &str) -> Value {
    json!({
        "type": "array",
        "items": { "type": "string" },
        "maxItems": MAX_LIST_LEN,
        "description": description
    })
}

fn create_task_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "name": {
                "type": "string", "pattern": NAME_PATTERN, "minLength": 1, "maxLength": MAX_NAME_LEN,
                "description": "Short kebab-case id, unique across every repo. Also the agent's name and its branch townhall/<name>."
            },
            "title": {
                "type": "string", "minLength": 1, "maxLength": MAX_TITLE_LEN,
                "description": "Short imperative title for people."
            },
            "prompt": {
                "type": "string", "minLength": 1, "maxLength": MAX_PROMPT_LEN,
                "description": "The brief the agent works from alone: goal, context, acceptance criteria."
            },
            "repo": {
                "type": "string",
                "description": "Name of a registered repo. Default: the repo townhall inferred for this conversation."
            },
            "baseBranch": {
                "type": "string",
                "description": "Branch the worktree starts from. Default: the repo's base branch."
            },
            "after": names_array("Names of tasks that must finish before this one starts."),
            "attachments": names_array("Attachment ids of this conversation the agent needs."),
            "agent": {
                "type": "string", "pattern": NAME_PATTERN, "minLength": 1, "maxLength": MAX_NAME_LEN,
                "description": "The agent type the task runs as (list_agent_types). Default: builder when it exists, else the first type by name."
            }
        },
        "required": ["name", "title", "prompt"],
        "additionalProperties": false
    })
}

fn set_title_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": { "type": "string", "minLength": 1, "maxLength": MAX_TITLE_LEN }
        },
        "required": ["title"],
        "additionalProperties": false
    })
}

/// Every tool townhall's session gets, in `tools/list` order.
pub fn tools() -> Vec<ToolDef> {
    [task_tools(), library::tools(), runs::tools(), logs::tools()].concat()
}

fn task_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: CREATE_TASK,
            description: "Create one task for an agent of a saved agent type (`agent`). It is queued and starts in its own worktree of the repo's base branch once its `after` tasks finished and a slot is free. Returns the task.",
            input_schema: create_task_schema(),
        },
        ToolDef {
            name: SET_TITLE,
            description: "Name this conversation in townhall's sidebar (3 to 6 words, lowercase), once the topic is clear. Returns the conversation.",
            input_schema: set_title_schema(),
        },
        ToolDef {
            name: TASK_STATUS,
            description: "What this conversation's tasks are doing and did. Without `name`: every task with its state, branch, `after` and its last 3 agent and needs-you entries. With `name`: that task plus its agent's last 20 steps and, once it is ended or merged, the agent's last message. Call it whenever you are asked what a task did or how it stands, instead of guessing.",
            input_schema: status::schema(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_points_at_the_conversation_endpoint() {
        let v: Value = serde_json::from_str(&config_json("http://127.0.0.1:7777/", "abc")).unwrap();
        assert_eq!(
            v,
            json!({"mcpServers": {"townhall": {"type": "http", "url": "http://127.0.0.1:7777/api/conversations/abc/mcp"}}})
        );
        assert_eq!(
            allowed_tools(),
            [
                "mcp__townhall__create_task",
                "mcp__townhall__set_title",
                "mcp__townhall__task_status",
                "mcp__townhall__list_agent_types",
                "mcp__townhall__read_agent_type",
                "mcp__townhall__save_agent_type",
                "mcp__townhall__read_logs",
                "mcp__townhall__log_summary"
            ]
        );
    }

    #[test]
    fn schemas_match_the_input_types() {
        let tools = tools();
        assert_eq!(tools.len(), 8);
        let create = &tools[0].input_schema;
        let props: Vec<&String> = create["properties"].as_object().unwrap().keys().collect();
        assert_eq!(
            props,
            [
                "after",
                "agent",
                "attachments",
                "baseBranch",
                "name",
                "prompt",
                "repo",
                "title"
            ]
        );
        assert_eq!(create["required"], json!(["name", "title", "prompt"]));
        assert_eq!(create["additionalProperties"], false);
        assert!(create
            .to_string()
            .chars()
            .all(|c| c != '\u{2014}' && c != '\u{2013}'));
        assert_eq!(tools[1].input_schema["required"], json!(["title"]));
        let status = &tools[2].input_schema;
        assert_eq!(tools[2].name, "task_status");
        assert!(status.get("required").is_none());
        assert_eq!(status["additionalProperties"], false);
        let save = &tools[5].input_schema;
        assert_eq!(tools[5].name, "save_agent_type");
        assert_eq!(save["required"], json!(["source"]));
        assert_eq!(tools[4].input_schema["required"], json!(["name"]));
        assert_eq!(tools[6].name, "read_logs");
        assert_eq!(tools[7].name, "log_summary");
        assert!(tools[6].input_schema.get("required").is_none());
        assert_eq!(tools[6].input_schema["properties"]["limit"]["maximum"], 200);
        assert_eq!(tools[7].input_schema["properties"]["top"]["maximum"], 50);
        assert!(INSTRUCTIONS.contains("log_summary") && INSTRUCTIONS.contains("~/.townhall/logs"));
        assert!(tools
            .iter()
            .all(|t| t.input_schema["additionalProperties"] == false));
    }
}
