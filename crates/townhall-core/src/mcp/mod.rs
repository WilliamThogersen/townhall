//! Townhall's MCP server for its own Claude session: JSON-RPC 2.0 over plain HTTP JSON
//! (no SSE) at `POST /api/conversations/{id}/mcp`, like Hive's `src/main/plan/mcp.ts`.
//! This module fixes the names, the tool schemas and the `--mcp-config` the runner (2.2)
//! passes; `handle` answers the requests (`rpc.rs`) with the task tools (`task_tools.rs`).
//! Units 2.8b and 2.8c add their tools in `library.rs` and `runs.rs`, and a match arm for
//! each in `TaskTools::call`.

mod checks;
mod library;
mod rpc;
mod runs;
mod task_tools;

use serde_json::{json, Value};

use crate::context::Context;

pub use rpc::{INVALID_PARAMS, INVALID_REQUEST, METHOD_NOT_FOUND};

/// Answers one JSON-RPC message or batch for the conversation `conversation_id` (which the
/// route checked exists). `None` when there is nothing to send back: the route answers `202`.
pub async fn handle(ctx: &Context, conversation_id: &str, body: Value) -> Option<Value> {
    let tools = task_tools::TaskTools {
        ctx,
        conversation_id,
    };
    rpc::answer(body, &tools).await
}

pub const SERVER_NAME: &str = "townhall";
pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const CREATE_TASK: &str = "create_task";
pub const SET_TITLE: &str = "set_title";
pub const NAME_PATTERN: &str = "^[A-Za-z0-9_-]+$";
pub const MAX_NAME_LEN: u64 = 64;
pub const MAX_TITLE_LEN: u64 = 80;
pub const MAX_PROMPT_LEN: u64 = 50_000;
pub const MAX_LIST_LEN: u64 = 20;

pub const INSTRUCTIONS: &str = "townhall tools: create_task hands one piece of work to a Claude agent in its own git worktree; set_title names this conversation once the topic is clear.";

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
            "attachments": names_array("Attachment ids of this conversation the agent needs.")
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
    [task_tools(), library::tools(), runs::tools()].concat()
}

fn task_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: CREATE_TASK,
            description: "Create one task for a Claude agent. It is queued and starts in its own worktree of the repo's base branch once its `after` tasks finished and a slot is free. Returns the task.",
            input_schema: create_task_schema(),
        },
        ToolDef {
            name: SET_TITLE,
            description: "Name this conversation in townhall's sidebar (3 to 6 words, lowercase), once the topic is clear. Returns the conversation.",
            input_schema: set_title_schema(),
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
            ["mcp__townhall__create_task", "mcp__townhall__set_title"]
        );
    }

    #[test]
    fn schemas_match_the_input_types() {
        let tools = tools();
        assert_eq!(tools.len(), 2);
        let create = &tools[0].input_schema;
        let props: Vec<&String> = create["properties"].as_object().unwrap().keys().collect();
        assert_eq!(
            props,
            [
                "after",
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
    }
}
