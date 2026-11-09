//! `claude -p --output-format stream-json` lines, as the runner needs them. Nothing is
//! trusted: every field is checked, and a line the runner does not use is skipped.
//! Partial deltas (`stream_event`) are skipped too: only finished messages count.

use serde_json::Value;

/// Claude Code's own error lines carry this model and no real usage.
const SYNTHETIC_MODEL: &str = "<synthetic>";
const ABORTED: &str = "aborted_streaming";

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// The CLI started (or resumed) the session.
    Init,
    /// A finished assistant message's text.
    Text(String),
    /// The context size of the model call behind an assistant message.
    Usage(u64),
    /// `can_use_tool`: the CLI asks whether the session may run a tool.
    ToolRequest {
        request_id: String,
        tool: String,
        input: Value,
    },
    /// Any other control request: answered with an error.
    OtherRequest {
        request_id: String,
        subtype: String,
    },
    Result(TurnResult),
}

/// The `result` line that ends a turn.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TurnResult {
    pub is_error: bool,
    /// Ended by an interrupt, which the user asked for.
    pub stopped: bool,
    pub message: Option<String>,
    /// Input tokens summed over the turn: a fallback for the context size.
    pub used: Option<u64>,
    pub tokens: Option<u64>,
    /// The model's context window, when the CLI says.
    pub limit: Option<u64>,
    /// What the turn cost in USD, when the CLI says.
    pub cost_usd: Option<f64>,
    /// Names of the tools the CLI refused to run (`permission_denials`). Never their input.
    pub denied: Vec<String>,
}

/// The events in one stdout line; none for a blank, non-JSON or unused line.
pub fn parse_line(line: &str) -> Vec<StreamEvent> {
    let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
        return vec![];
    };
    match value.get("type").and_then(Value::as_str) {
        Some("system") if value["subtype"] == "init" => vec![StreamEvent::Init],
        Some("assistant") => assistant(&value),
        Some("control_request") => control_request(&value).into_iter().collect(),
        Some("result") => vec![StreamEvent::Result(result(&value))],
        _ => vec![],
    }
}

fn text_of(content: &Value) -> String {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .filter(|t| !t.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Input, cache creation and cache read tokens: what the model was handed.
fn input_size(usage: &Value) -> Option<u64> {
    let input = usage.get("input_tokens")?.as_u64()?;
    let more = ["cache_creation_input_tokens", "cache_read_input_tokens"]
        .iter()
        .filter_map(|k| usage.get(*k).and_then(Value::as_u64))
        .sum::<u64>();
    Some(input + more)
}

fn turn_tokens(usage: &Value) -> Option<u64> {
    let count = |key: &str| usage.get(key).and_then(Value::as_u64);
    match (count("input_tokens"), count("output_tokens")) {
        (None, None) => None,
        (input, output) => Some(input.unwrap_or(0) + output.unwrap_or(0)),
    }
}

fn assistant(value: &Value) -> Vec<StreamEvent> {
    let message = &value["message"];
    if message.get("model").and_then(Value::as_str) == Some(SYNTHETIC_MODEL) {
        return vec![];
    }
    let text = text_of(&message["content"]);
    let usage = input_size(&message["usage"]).map(StreamEvent::Usage);
    let text = (!text.is_empty()).then_some(StreamEvent::Text(text));
    text.into_iter().chain(usage).collect()
}

fn control_request(value: &Value) -> Option<StreamEvent> {
    let request_id = value.get("request_id")?.as_str()?.to_string();
    let request = &value["request"];
    let subtype = request
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tool = request.get("tool_name").and_then(Value::as_str);
    match (subtype, tool) {
        ("can_use_tool", Some(tool)) => Some(StreamEvent::ToolRequest {
            request_id,
            tool: tool.to_string(),
            input: request.get("input").cloned().unwrap_or(Value::Null),
        }),
        _ => Some(StreamEvent::OtherRequest {
            request_id,
            subtype: subtype.to_string(),
        }),
    }
}

fn message_of(value: &Value) -> Option<String> {
    let text = |k: &str| value.get(k).and_then(Value::as_str).map(str::to_string);
    let errors = value.get("errors").and_then(Value::as_array).map(|list| {
        list.iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    });
    text("result")
        .or_else(|| text("error"))
        .or(errors)
        .filter(|m| !m.trim().is_empty())
}

fn window_of(value: &Value) -> Option<u64> {
    let models = value.get("modelUsage")?.as_object()?;
    models
        .values()
        .filter_map(|m| m.get("contextWindow").and_then(Value::as_u64))
        .filter(|w| *w > 0)
        .max()
}

fn result(value: &Value) -> TurnResult {
    let stopped = value.get("terminal_reason").and_then(Value::as_str) == Some(ABORTED);
    let subtype = value.get("subtype").and_then(Value::as_str);
    let failed = value.get("is_error").and_then(Value::as_bool) == Some(true)
        || subtype.is_some_and(|s| s != "success");
    TurnResult {
        is_error: failed && !stopped,
        stopped,
        message: message_of(value),
        used: input_size(&value["usage"]),
        tokens: turn_tokens(&value["usage"]),
        limit: window_of(value),
        cost_usd: value.get("total_cost_usd").and_then(Value::as_f64),
        denied: denied_tools(value),
    }
}

fn denied_tools(value: &Value) -> Vec<String> {
    value
        .get("permission_denials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| d.get("tool_name").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(v: Value) -> Vec<StreamEvent> {
        parse_line(&v.to_string())
    }

    #[test]
    fn finished_messages_count_and_deltas_do_not() {
        let assistant = json!({"type": "assistant", "message": {"model": "m", "content": [
            {"type": "text", "text": "one"}, {"type": "tool_use", "id": "t", "name": "Read"},
            {"type": "text", "text": "two"}],
            "usage": {"input_tokens": 10, "cache_read_input_tokens": 5, "output_tokens": 3}}});
        assert_eq!(
            line(assistant),
            [
                StreamEvent::Text("one\n\ntwo".into()),
                StreamEvent::Usage(15)
            ]
        );
        let delta = json!({"type": "stream_event", "event": {"type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "on"}}});
        assert!(line(delta).is_empty());
        let synthetic = json!({"type": "assistant", "message": {"model": "<synthetic>",
            "content": [{"type": "text", "text": "API Error"}]}});
        assert!(line(synthetic).is_empty());
        let tools_only = json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "t", "name": "Read"}]}});
        assert!(line(tools_only).is_empty());
        assert_eq!(
            line(json!({"type": "system", "subtype": "init"})),
            [StreamEvent::Init]
        );
        assert!(parse_line("not json").is_empty() && parse_line("").is_empty());
        assert!(line(json!({"type": "system", "subtype": "hook"})).is_empty());
    }

    #[test]
    fn control_requests_name_their_tool() {
        let req = json!({"type": "control_request", "request_id": "r1",
            "request": {"subtype": "can_use_tool", "tool_name": "Edit", "input": {"a": 1}}});
        assert_eq!(
            line(req),
            [StreamEvent::ToolRequest {
                request_id: "r1".into(),
                tool: "Edit".into(),
                input: json!({"a": 1})
            }]
        );
        let other = json!({"type": "control_request", "request_id": "r2",
            "request": {"subtype": "hook_callback"}});
        assert_eq!(
            line(other),
            [StreamEvent::OtherRequest {
                request_id: "r2".into(),
                subtype: "hook_callback".into()
            }]
        );
        let no_id = json!({"type": "control_request", "request": {"subtype": "can_use_tool"}});
        assert!(line(no_id).is_empty());
    }

    #[test]
    fn results_say_how_the_turn_ended() {
        let ok = json!({"type": "result", "subtype": "success", "is_error": false, "result": "done",
            "usage": {"input_tokens": 50, "output_tokens": 4, "cache_creation_input_tokens": 7},
            "modelUsage": {"a": {"contextWindow": 200000}, "b": {"contextWindow": 1000000}}});
        let want = TurnResult {
            is_error: false,
            stopped: false,
            message: Some("done".into()),
            used: Some(57),
            tokens: Some(54),
            limit: Some(1_000_000),
            cost_usd: None,
            denied: vec![],
        };
        assert_eq!(line(ok), [StreamEvent::Result(want)]);
        let failed =
            json!({"type": "result", "subtype": "error_max_turns", "errors": ["too", "long"]});
        let Some(StreamEvent::Result(r)) = line(failed).pop() else {
            panic!("a result")
        };
        assert!(r.is_error && !r.stopped);
        assert_eq!(r.message.as_deref(), Some("too long"));
        let stopped = json!({"type": "result", "subtype": "error_during_execution",
            "is_error": true, "terminal_reason": "aborted_streaming"});
        let Some(StreamEvent::Result(r)) = line(stopped).pop() else {
            panic!("a result")
        };
        assert!(r.stopped && !r.is_error && r.message.is_none() && r.limit.is_none());
    }

    #[test]
    fn results_carry_the_cost_and_only_the_names_of_denied_tools() {
        let result = json!({"type": "result", "subtype": "success", "total_cost_usd": 0.5,
            "permission_denials": [
                {"tool_name": "Bash", "tool_use_id": "a", "tool_input": {"command": "rm -rf /"}},
                {"tool_use_id": "no name"},
                {"tool_name": "Edit"}]});
        let Some(StreamEvent::Result(r)) = line(result).pop() else {
            panic!("a result")
        };
        assert_eq!(r.cost_usd, Some(0.5));
        assert_eq!(r.denied, ["Bash", "Edit"]);
    }
}
