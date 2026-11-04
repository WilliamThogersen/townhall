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
        json!({"type": "system", "subtype": "turn_duration"}).to_string(),
    ];
    assert_eq!(
        turn(&lines),
        Some(Turn::Idle {
            text: "Should I push?".into()
        })
    );
    let marked = [
        assistant("m1", None, text("done")),
        json!({"type": "system", "subtype": "turn_duration"}).to_string(),
    ];
    assert_eq!(
        turn(&marked),
        Some(Turn::Idle {
            text: "done".into()
        })
    );
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
    assert_eq!(
        turn(&lines),
        Some(Turn::Idle {
            text: String::new()
        })
    );
    let error = json!({"type": "assistant", "isApiErrorMessage": true,
        "message": {"id": "e1", "stop_reason": "stop_sequence", "content": [{"type": "text", "text": "API Error"}]}});
    assert_eq!(
        turn(&[error.to_string()]),
        Some(Turn::Idle {
            text: "API Error".into()
        })
    );
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
    assert_eq!(turn(&lines), Some(Turn::Idle { text: "ok".into() }));
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
        Some(Turn::Idle { text }) => assert_eq!(text.len(), big.len()),
        other => panic!("{other:?}"),
    }
    assert!(read(&dir.path().join("missing.jsonl")).is_err());
}
