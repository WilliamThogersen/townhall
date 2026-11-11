//! JSON-RPC 2.0 for MCP over plain HTTP JSON (no SSE), like Hive's `rpc.ts`: `initialize`,
//! `ping`, `tools/list`, `tools/call` and batches. A notification (no `id`) runs nothing and
//! gets no answer, nor does a batch of only notifications (the route answers `202`): MCP's
//! notifications need nothing from townhall. A tool that fails answers a result with
//! `isError: true` and a text the session can act on.

use std::time::Instant;

use serde_json::{json, Map, Value};

use super::{ToolDef, PROTOCOL_VERSION, SERVER_NAME};
use crate::redact::redact;

pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

/// Runs one `tools/call`: the tool's result, or why it failed in words the session can act on.
pub(crate) trait ToolRunner {
    /// The conversation whose session calls the tools, for the `mcp.tool` log line.
    fn conversation(&self) -> &str;
    fn tools(&self) -> Vec<ToolDef>;
    fn instructions(&self) -> &'static str;
    async fn call(&self, name: &str, arguments: Value) -> Result<Value, String>;
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn fail(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool's answer: its value as JSON text, or the reason with `isError: true`.
fn tool_result(outcome: Result<Value, String>) -> Value {
    match outcome {
        Ok(value) => json!({ "content": [{ "type": "text", "text": value.to_string() }] }),
        Err(why) => json!({ "content": [{ "type": "text", "text": why }], "isError": true }),
    }
}

fn initialize(params: &Map<String, Value>, instructions: &str) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": crate::version::VERSION },
        "instructions": instructions
    })
}

fn tool_list(tools: Vec<ToolDef>) -> Value {
    let listed: Vec<Value> = tools
        .into_iter()
        .map(|t| {
            let mut tool = json!({ "name": t.name, "description": t.description, "inputSchema": t.input_schema });
            if t.read_only {
                tool["annotations"] = json!({ "readOnlyHint": true, "destructiveHint": false });
            }
            tool
        })
        .collect();
    json!({ "tools": listed })
}

fn valid_id(id: &Value) -> bool {
    id.is_string() || id.is_number() || id.is_null()
}

async fn call_tool(id: Value, params: &Map<String, Value>, tools: &impl ToolRunner) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return fail(
            id,
            INVALID_PARAMS,
            "tools/call needs params.name, the tool's name",
        );
    };
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    let started = Instant::now();
    let outcome = tools.call(name, arguments).await;
    log_call(name, tools.conversation(), started, &outcome);
    ok(id, tool_result(outcome))
}

/// One `mcp.tool` line per call: never the arguments or the result, only the error text.
fn log_call(name: &str, conversation: &str, started: Instant, outcome: &Result<Value, String>) {
    let tool = redact(name);
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    match outcome {
        Ok(_) => tracing::debug!(
            target: "townhall::mcp", event = "mcp.tool", tool = %tool, conversation,
            duration_ms, ok = true, "tool call"
        ),
        Err(why) => tracing::warn!(
            target: "townhall::mcp", event = "mcp.tool", tool = %tool, conversation,
            duration_ms, ok = false, error = %redact(why), "tool call failed"
        ),
    }
}

/// The answer to one message, `None` for a notification.
async fn one(message: Value, tools: &impl ToolRunner) -> Option<Value> {
    let Value::Object(message) = message else {
        return Some(fail(Value::Null, INVALID_REQUEST, "invalid request"));
    };
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").cloned();
    let valid = message.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
        && id.as_ref().is_none_or(valid_id);
    let (Some(method), true) = (method, valid) else {
        let id = id.filter(valid_id).unwrap_or(Value::Null);
        return Some(fail(id, INVALID_REQUEST, "invalid request"));
    };
    let id = id?;
    let params = match message.get("params") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(params)) => params.clone(),
        Some(_) => return Some(fail(id, INVALID_PARAMS, "params must be an object")),
    };
    Some(match method {
        "initialize" => ok(id, initialize(&params, tools.instructions())),
        "ping" => ok(id, json!({})),
        "tools/list" => ok(id, tool_list(tools.tools())),
        "tools/call" => call_tool(id, &params, tools).await,
        other => fail(id, METHOD_NOT_FOUND, &format!("method not found: {other}")),
    })
}

/// Answers one message or a batch (in order). `None` when there is nothing to send back.
pub(crate) async fn answer(body: Value, tools: &impl ToolRunner) -> Option<Value> {
    let Value::Array(batch) = body else {
        return one(body, tools).await;
    };
    if batch.is_empty() {
        return Some(fail(
            Value::Null,
            INVALID_REQUEST,
            "invalid request: empty batch",
        ));
    }
    let mut answers = vec![];
    for message in batch {
        answers.extend(one(message, tools).await);
    }
    (!answers.is_empty()).then_some(Value::Array(answers))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl ToolRunner for Echo {
        fn conversation(&self) -> &str {
            "c1"
        }

        fn tools(&self) -> Vec<ToolDef> {
            crate::mcp::tools()
        }

        fn instructions(&self) -> &'static str {
            ""
        }

        async fn call(&self, name: &str, arguments: Value) -> Result<Value, String> {
            match name {
                "echo" => Ok(json!({ "got": arguments })),
                other => Err(format!("unknown tool `{other}`")),
            }
        }
    }

    async fn ask(body: Value) -> Option<Value> {
        answer(body, &Echo).await
    }

    fn call(id: Value, name: &str, arguments: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
               "params": {"name": name, "arguments": arguments}})
    }

    #[tokio::test]
    async fn initialize_echoes_or_defaults_the_protocol_version() {
        let asked = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                           "params": {"protocolVersion": "2024-11-05"}});
        let got = ask(asked).await.unwrap();
        assert_eq!(got["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(got["result"]["serverInfo"]["name"], SERVER_NAME);
        let bare = json!({"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": null});
        let got = ask(bare).await.unwrap();
        assert_eq!(got["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(got["result"]["capabilities"], json!({"tools": {}}));
    }

    #[tokio::test]
    async fn tool_results_are_text_and_failures_are_marked() {
        let got = ask(call(json!("a"), "echo", json!({"x": 1})))
            .await
            .unwrap();
        assert_eq!(got["id"], "a");
        assert_eq!(got["result"]["content"][0]["type"], "text");
        let text = got["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(text).unwrap(),
            json!({"got": {"x": 1}})
        );
        assert!(got["result"].get("isError").is_none());
        let failed = ask(call(json!(3), "nope", Value::Null)).await.unwrap();
        assert_eq!(failed["result"]["isError"], true);
        assert_eq!(
            failed["result"]["content"][0]["text"],
            "unknown tool `nope`"
        );
        let no_name = json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {}});
        assert_eq!(ask(no_name).await.unwrap()["error"]["code"], INVALID_PARAMS);
        let bad_params = json!({"jsonrpc": "2.0", "id": 5, "method": "ping", "params": [1]});
        assert_eq!(
            ask(bad_params).await.unwrap()["error"]["code"],
            INVALID_PARAMS
        );
    }

    /// The names `tools/list` marks `readOnlyHint: true`, checking every tool is either
    /// marked (with `destructiveHint: false`) or carries no `annotations` at all.
    fn read_only_names(tools: Vec<ToolDef>) -> Vec<&'static str> {
        let listed = tool_list(tools.clone());
        let mut marked = vec![];
        for (def, wire) in tools.iter().zip(listed["tools"].as_array().unwrap()) {
            if let Some(a) = wire.get("annotations") {
                assert_eq!(
                    a,
                    &json!({"readOnlyHint": true, "destructiveHint": false}),
                    "{}",
                    def.name
                );
                marked.push(def.name);
            }
        }
        marked
    }

    #[test]
    fn tools_list_marks_only_the_store_and_read_tools_read_only() {
        assert_eq!(
            read_only_names(crate::mcp::task_tools_list()),
            [
                "hand_back",
                "add_artifact",
                "list_artifacts",
                "search_history",
                "read_conversation",
                "ask_adviser"
            ]
        );
        assert_eq!(
            read_only_names(crate::mcp::tools()),
            [
                "task_status",
                "list_agent_types",
                "read_agent_type",
                "list_pipelines",
                "read_pipeline",
                "read_logs",
                "log_summary",
                "conversation_insights",
                "search_history",
                "read_conversation",
                "ask_adviser"
            ]
        );
    }

    #[tokio::test]
    async fn notifications_and_invalid_messages() {
        let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert_eq!(ask(note.clone()).await, None);
        assert_eq!(ask(json!([note.clone(), note])).await, None);
        let null_id = json!({"jsonrpc": "2.0", "id": null, "method": "ping"});
        assert_eq!(ask(null_id).await.unwrap()["result"], json!({}));
        for bad in [
            json!(1),
            json!({"jsonrpc": "1.0", "id": 1, "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": 1}),
            json!({"jsonrpc": "2.0", "id": {"x": 1}, "method": "ping"}),
            json!({"jsonrpc": "2.0", "method": 5}),
            json!([]),
        ] {
            let got = ask(bad.clone()).await.unwrap();
            assert_eq!(got["error"]["code"], INVALID_REQUEST, "{bad}");
        }
        let kept = ask(json!({"jsonrpc": "1.0", "id": 9, "method": "ping"})).await;
        assert_eq!(kept.unwrap()["id"], 9);
        let unknown = ask(json!({"jsonrpc": "2.0", "id": 6, "method": "resources/list"})).await;
        assert_eq!(unknown.unwrap()["error"]["code"], METHOD_NOT_FOUND);
    }

    #[tokio::test]
    async fn batches_answer_in_order_without_notifications() {
        let batch = json!([
            {"jsonrpc": "2.0", "id": 1, "method": "ping"},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            "junk"
        ]);
        let got = ask(batch).await.unwrap();
        let got = got.as_array().unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!((&got[0]["id"], &got[1]["id"]), (&json!(1), &json!(2)));
        let names: Vec<&str> = got[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "create_task",
                "set_title",
                "task_status",
                "list_agent_types",
                "read_agent_type",
                "save_agent_type",
                "list_pipelines",
                "read_pipeline",
                "save_pipeline",
                "read_logs",
                "log_summary",
                "conversation_insights",
                "search_history",
                "read_conversation",
                "ask_adviser"
            ]
        );
        assert_eq!(got[2]["error"]["code"], INVALID_REQUEST);
    }
}
