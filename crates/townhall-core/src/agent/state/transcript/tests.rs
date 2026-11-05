use serde_json::{json, Value};

use super::*;

fn assistant(id: &str, stop: Option<&str>, content: Value) -> String {
    json!({"type": "assistant", "isSidechain": false, "timestamp": "2026-11-04T10:00:00.000Z",
           "cwd": "/w/a", "message": {"id": id, "stop_reason": stop, "content": content}})
    .to_string()
}

fn user(content: Value) -> String {
    json!({"type": "user", "isSidechain": false, "timestamp": "2026-11-04T10:00:01.000Z",
           "message": {"role": "user", "content": content}})
    .to_string()
}

fn tool(id: &str, name: &str, input: Value) -> Value {
    json!([{"type": "tool_use", "id": id, "name": name, "input": input}])
}

fn result(id: &str) -> String {
    user(json!([{"type": "tool_result", "tool_use_id": id, "content": "ok"}]))
}

/// 2026-11-04T10:00:00.000Z and one second later, as unix milliseconds.
const AT: i64 = 1_793_786_400_000;

fn idle(text: &str, at: Option<i64>) -> Option<Turn> {
    Some(Turn::Idle {
        text: text.into(),
        at,
    })
}

fn turn(lines: &[String]) -> Option<Turn> {
    parse(&lines.join("\n"))
}

#[test]
fn a_pending_ask_is_the_question_with_its_options() {
    let input = json!({"questions": [{"question": "Which db?", "header": "db", "multiSelect": false,
        "options": [{"label": "postgres", "description": "big"}, {"label": "sqlite", "description": "small"}]}]});
    let lines = [
        user(json!("set up the db")),
        assistant("m1", Some("tool_use"), tool("t1", "AskUserQuestion", input)),
    ];
    assert_eq!(
        turn(&lines),
        Some(Turn::Asking {
            question: "Which db?\n1. postgres\n2. sqlite".into()
        })
    );
    let answered = [lines[0].clone(), lines[1].clone(), result("t1")];
    assert_eq!(turn(&answered), Some(Turn::Busy));
}

#[test]
fn several_questions_say_how_many_more() {
    let input = json!({"questions": [{"question": "A?", "options": []}, {"question": "B?"}, {"question": "C?"}]});
    let lines = [assistant("m1", None, tool("t1", "AskUserQuestion", input))];
    assert_eq!(
        turn(&lines),
        Some(Turn::Asking {
            question: "A?\n+2 more questions".into()
        })
    );
}

#[test]
fn a_pending_tool_is_described_for_a_permission_prompt() {
    let lines = [assistant(
        "m1",
        Some("tool_use"),
        tool("t1", "Bash", json!({"command": "rm -rf build"})),
    )];
    assert_eq!(
        turn(&lines),
        Some(Turn::Pending {
            call: "Bash: rm -rf build".into()
        })
    );
    let unnamed = [assistant("m1", None, tool("t1", "Mystery", json!({})))];
    assert_eq!(
        turn(&unnamed),
        Some(Turn::Pending {
            call: "Mystery".into()
        })
    );
}

#[test]
fn an_ended_turn_keeps_the_last_message_text() {
    let text = |t: &str| json!([{"type": "text", "text": t}]);
    let lines = [
        assistant("m0", Some("end_turn"), text("older")),
        user(json!("go on")),
        assistant(
            "m1",
            Some("end_turn"),
            json!([{"type": "thinking", "thinking": ""}]),
        ),
        assistant("m1", Some("end_turn"), text("Should I push?")),
        json!({"type": "system", "subtype": "turn_duration", "timestamp": "2026-11-04T10:00:05.000Z"})
            .to_string(),
        json!({"type": "system", "subtype": "away_summary", "timestamp": "2026-11-04T10:09:00.000Z"})
            .to_string(),
    ];
    assert_eq!(turn(&lines), idle("Should I push?", Some(AT + 5000)));
    let marked = [
        assistant("m1", None, text("done")),
        json!({"type": "system", "subtype": "turn_duration"}).to_string(),
    ];
    assert_eq!(turn(&marked), idle("done", Some(AT)));
}

#[test]
fn busy_while_streaming_or_after_a_prompt_or_result() {
    let text = json!([{"type": "text", "text": "looking"}]);
    assert_eq!(
        turn(&[assistant("m1", None, text.clone())]),
        Some(Turn::Busy)
    );
    assert_eq!(turn(&[user(json!("hi"))]), Some(Turn::Busy));
    let after_tool = [
        assistant("m1", Some("tool_use"), tool("t1", "Read", json!({}))),
        result("t1"),
    ];
    assert_eq!(turn(&after_tool), Some(Turn::Busy));
}

#[test]
fn an_interrupt_or_api_error_ends_the_turn() {
    let lines = [
        assistant("m1", Some("tool_use"), tool("t1", "Bash", json!({}))),
        result("t1"),
        user(json!([{"type": "text", "text": "[Request interrupted by user for tool use]"}])),
    ];
    assert_eq!(turn(&lines), idle("", Some(AT + 1000)));
    let error = json!({"type": "assistant", "isApiErrorMessage": true,
        "message": {"id": "e1", "stop_reason": "stop_sequence", "content": [{"type": "text", "text": "API Error"}]}});
    assert_eq!(turn(&[error.to_string()]), idle("API Error", None));
}

#[test]
fn sidechains_meta_and_noise_are_ignored() {
    let side = json!({"type": "assistant", "isSidechain": true,
        "message": {"id": "s1", "content": [{"type": "tool_use", "id": "x", "name": "Bash", "input": {}}]}});
    let meta = json!({"type": "user", "isMeta": true, "message": {"content": "caveat"}});
    let lines = [
        assistant(
            "m1",
            Some("end_turn"),
            json!([{"type": "text", "text": "ok"}]),
        ),
        side.to_string(),
        meta.to_string(),
        "{\"type\":\"api-request\"}".to_string(),
        "{not json".to_string(),
    ];
    assert_eq!(turn(&lines), idle("ok", Some(AT)));
    assert_eq!(turn(&["{\"type\":\"mode\"}".to_string()]), None);
}

#[test]
fn read_grows_the_window_past_a_huge_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    let big = format!("x{}", "y".repeat(300 * 1024));
    let lines = [
        assistant(
            "m1",
            Some("end_turn"),
            json!([{"type": "text", "text": big}]),
        ),
        json!({"type": "last-prompt"}).to_string(),
    ];
    std::fs::write(&path, lines.join("\n")).unwrap();
    match read(&path).unwrap() {
        Some(Turn::Idle { text, .. }) => assert_eq!(text.len(), big.len()),
        other => panic!("{other:?}"),
    }
    assert!(read(&dir.path().join("missing.jsonl")).is_err());
}

/// Collects the events `tracing` emits on this thread, as JSON objects of their fields.
mod capture {
    use std::sync::{Arc, Mutex};

    use serde_json::{Map, Value};
    use tracing::field::{Field, Visit};
    use tracing::span::{Attributes, Id, Record};
    use tracing::{Event, Metadata, Subscriber};

    #[derive(Clone, Default)]
    pub struct Events(Arc<Mutex<Vec<Value>>>);

    impl Events {
        pub fn named(&self, event: &str) -> Vec<Value> {
            let all = self.0.lock().unwrap();
            all.iter().filter(|e| e["event"] == event).cloned().collect()
        }

        pub fn all(&self) -> Vec<Value> {
            self.0.lock().unwrap().clone()
        }
    }

    struct Fields(Map<String, Value>);

    impl Visit for Fields {
        fn record_str(&mut self, f: &Field, v: &str) {
            self.0.insert(f.name().into(), v.into());
        }
        fn record_debug(&mut self, f: &Field, v: &dyn std::fmt::Debug) {
            self.0.insert(f.name().into(), format!("{v:?}").into());
        }
    }

    impl Subscriber for Events {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _: &Id, _: &Record<'_>) {}
        fn record_follows_from(&self, _: &Id, _: &Id) {}
        fn event(&self, event: &Event<'_>) {
            let mut fields = Fields(Map::new());
            event.record(&mut fields);
            fields.0.insert("target".into(), event.metadata().target().into());
            self.0.lock().unwrap().push(Value::Object(fields.0));
        }
        fn enter(&self, _: &Id) {}
        fn exit(&self, _: &Id) {}
    }
}

const SECRET_INPUT: &str = "curl --data hunter2-in-the-tool-input";

fn transcript_file(lines: &[String]) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.jsonl");
    std::fs::write(&path, lines.join("\n")).unwrap();
    (dir, path)
}

fn failing_call(id: &str, is_error: bool, error: &str) -> Vec<String> {
    vec![
        user(json!("run it")),
        assistant(
            "m1",
            Some("tool_use"),
            tool(id, "Bash", json!({"command": SECRET_INPUT})),
        ),
        user(json!([{"type": "tool_result", "tool_use_id": id, "is_error": is_error, "content": error}])),
        assistant("m2", Some("end_turn"), json!([{"type": "text", "text": "done"}])),
    ]
}

#[test]
fn a_tool_error_is_one_event_with_the_tool_and_redacted_start_of_the_error() {
    let events = capture::Events::default();
    let _guard = tracing::subscriber::set_default(events.clone());
    let message = format!("sk-ant-abcdefghijklmnop failed {}", "x".repeat(400));
    let (_dir, path) = transcript_file(&failing_call("toolu_err1", true, &message));
    read_as(&path, "alice").unwrap();
    read_as(&path, "alice").unwrap();
    let found = events.named("agent.tool_error");
    assert_eq!(found.len(), 1, "{:?}", events.all());
    let event = &found[0];
    assert_eq!(event["target"], "townhall::agent");
    assert_eq!(event["agent"], "alice");
    assert_eq!(event["tool"], "Bash");
    let error = event["error"].as_str().unwrap();
    assert!(error.chars().count() <= 200, "{}", error.len());
    assert!(error.contains("[REDACTED]") && !error.contains("abcdefghijklmnop"));
    assert!(events.named("agent.permission_denied").is_empty());
    assert!(!format!("{:?}", events.all()).contains("hunter2"));
}

#[test]
fn a_permission_denial_is_one_event_with_the_tool_only() {
    let events = capture::Events::default();
    let _guard = tracing::subscriber::set_default(events.clone());
    let denied = "Permission to use Bash with command curl --data hunter2-in-the-tool-input has been denied.";
    let (_dir, path) = transcript_file(&failing_call("toolu_deny1", true, denied));
    read_as(&path, "alice").unwrap();
    read_as(&path, "alice").unwrap();
    let found = events.named("agent.permission_denied");
    assert_eq!(found.len(), 1, "{:?}", events.all());
    assert_eq!(found[0]["tool"], "Bash");
    assert_eq!(found[0]["agent"], "alice");
    assert!(found[0].get("error").is_none());
    assert!(events.named("agent.tool_error").is_empty());
    assert!(!format!("{:?}", events.all()).contains("hunter2"));
}

#[test]
fn a_successful_result_and_a_read_without_an_agent_log_nothing() {
    let events = capture::Events::default();
    let _guard = tracing::subscriber::set_default(events.clone());
    let (_dir, path) = transcript_file(&failing_call("toolu_ok1", false, "fine"));
    read_as(&path, "alice").unwrap();
    let (_dir2, path2) = transcript_file(&failing_call("toolu_silent1", true, "boom"));
    read(&path2).unwrap();
    assert!(events.all().is_empty(), "{:?}", events.all());
}
