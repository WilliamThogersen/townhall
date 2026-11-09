//! `conversation_insights` over the MCP endpoint, on conversations, tasks and hand backs
//! seeded through the store: listed for the conversation's session only, finds the recurring
//! patterns of every conversation, redacts and changes nothing.

use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::model::{EntryBody, NewTask, Task, TaskState};

mod common;
mod mcp_support;
use common::call;
use mcp_support::Mcp;

const CODER: &str = "name = \"coder\"\nadapter = \"claude\"\n";
const TESTER: &str = "name = \"tester\"\nadapter = \"claude\"\n";

async fn tool_names(m: &Mcp) -> Vec<String> {
    let list = m
        .rpc(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .await;
    list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

/// A coder task of `m`'s conversation that was handed to the tester for `reason`.
async fn handed_to_tester(m: &Mcp, name: &str, conversation: &str, reason: &str) {
    let new = NewTask {
        name: name.into(),
        title: "fix".into(),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "coder".into(),
        pr_base: None,
        done: Default::default(),
    };
    m.store().create_task(conversation, new).await.unwrap();
    let working = |t: Task| {
        Ok(Task {
            state: TaskState::Working,
            ..t
        })
    };
    m.store().update_task(name, working).await.unwrap();
    m.store()
        .hand_back(name, "tester", reason, "did the work")
        .await
        .unwrap();
}

/// Two conversations that repeat a request, an agent failure and a hand back.
async fn seed(m: &Mcp) -> String {
    for source in [CODER, TESTER] {
        m.store().save_agent_type(source).await.unwrap();
    }
    let other = m.store().create_conversation().await.unwrap().id;
    m.you("Please run the linter on PR 12").await;
    m.you("something only said once").await;
    m.you("deploy now token=hunter2hunter2").await;
    for text in [
        "please run the linter on pr 98",
        "deploy now token=hunter2hunter2",
    ] {
        let again = EntryBody::You {
            text: text.into(),
            attachments: vec![],
            reply_to: None,
        };
        m.store().append_entry(&other, again).await.unwrap();
    }
    for (conversation, name) in [(&m.id, "fix-a"), (&other, "fix-b")] {
        let failed = EntryBody::Agent {
            agent: name.into(),
            event: townhall_core::model::AgentEvent::Failed,
            text: "cargo clippy failed with 3 errors".into(),
        };
        m.store().append_entry(conversation, failed).await.unwrap();
    }
    handed_to_tester(m, "fix-a", &m.id, "Tests missing for 3 files").await;
    handed_to_tester(m, "fix-b", &other, "tests missing for 9 files").await;
    other
}

/// Everything the tool could have changed.
async fn snapshot(m: &Mcp) -> Value {
    let store = m.store();
    let conversations = store.conversations().await.unwrap();
    let mut entries = vec![];
    for c in &conversations {
        entries.push(store.entries(&c.id).await.unwrap());
    }
    let tasks = store.all_tasks().await.unwrap();
    let mut handoffs = vec![];
    for t in &tasks {
        handoffs.push(store.handoffs(&t.name).await.unwrap());
    }
    json!({
        "conversations": conversations,
        "entries": entries,
        "tasks": tasks,
        "handoffs": handoffs,
        "ended": store.task_ended_times().await.unwrap(),
    })
}

#[tokio::test]
async fn only_the_conversation_session_is_given_the_tool() {
    let m = Mcp::new().await;
    seed(&m).await;
    assert!(tool_names(&m)
        .await
        .contains(&"conversation_insights".into()));
    assert!(townhall_core::mcp::allowed_tools()
        .contains(&"mcp__townhall__conversation_insights".to_string()));

    let path = "/api/tasks/fix-a/mcp";
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let (status, answer) = call(&m.app, "POST", path, Some(list)).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let names = answer["result"]["tools"].to_string();
    assert!(!names.contains("conversation_insights"), "{names}");
    assert!(!townhall_core::mcp::task_allowed_tools()
        .iter()
        .any(|t| t.contains("conversation_insights")));

    let body = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                      "params": {"name": "conversation_insights", "arguments": {}}});
    let (_, answer) = call(&m.app, "POST", path, Some(body)).await;
    assert_eq!(answer["result"]["isError"], true, "{answer}");
}

#[tokio::test]
async fn finds_patterns_across_every_conversation() {
    let m = Mcp::new().await;
    let other = seed(&m).await;

    let out = m.ok("conversation_insights", json!({})).await;
    assert_eq!(out["counts"]["conversations"], 2, "{out}");
    assert_eq!(out["counts"]["tasks"], 2);
    assert_eq!(out["counts"]["handoffs"], 2);
    assert_eq!(out["counts"]["agent_failed"], 2);
    let patterns = out["patterns"].as_array().unwrap();
    let get = |kind: &str| {
        patterns
            .iter()
            .find(|p| p["kind"] == kind)
            .unwrap_or_else(|| panic!("no {kind} in {out}"))
    };

    let request = patterns
        .iter()
        .find(|p| p["pattern"] == "please run the linter on pr")
        .unwrap_or_else(|| panic!("no linter request in {out}"));
    assert_eq!(request["kind"], "user_request");
    assert_eq!(request["count"], 2);
    let mut convs: Vec<&str> = request["conversations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    convs.sort_unstable();
    let mut want = [m.id.as_str(), other.as_str()];
    want.sort_unstable();
    assert_eq!(convs, want);
    assert_eq!(request["fingerprint"].as_str().unwrap().len(), 16);
    assert!(request["suggested_automation"].as_str().unwrap().len() > 10);
    assert!(request["first_seen"].is_string() && request["last_seen"].is_string());

    assert_eq!(
        get("agent_failed")["pattern"],
        "cargo clippy failed with errors"
    );
    assert_eq!(
        get("handoff")["pattern"],
        "coder -> tester: tests missing for files"
    );
    assert_eq!(get("task_chain")["pattern"], "coder -> tester");
    assert!(patterns.iter().all(|p| p["count"].as_u64().unwrap() >= 2));
    assert_eq!(
        out["counts"]["tasks_by_agent"]["tester"]["queued"], 2,
        "{out}"
    );

    let text = out.to_string();
    assert!(!text.contains("hunter2"), "{text}");
    assert!(text.contains("REDACTED"), "{text}");
}

#[tokio::test]
async fn honours_top_and_refuses_bad_arguments() {
    let m = Mcp::new().await;
    seed(&m).await;
    let one = m.ok("conversation_insights", json!({"top": 1})).await;
    assert_eq!(one["patterns"].as_array().unwrap().len(), 1);
    assert_eq!(one["truncated"], true);
    let recent = m.ok("conversation_insights", json!({"since": "1h"})).await;
    assert_eq!(recent["counts"]["tasks"], 2, "{recent}");
    let old = m
        .ok(
            "conversation_insights",
            json!({"since": "2999-01-01T00:00:00Z"}),
        )
        .await;
    assert_eq!(old["patterns"], json!([]));
    assert_eq!(old["counts"]["tasks"], 0);

    m.refused("conversation_insights", json!({"nope": 1})).await;
    let bad = m
        .refused("conversation_insights", json!({"since": "yesterday"}))
        .await;
    assert!(bad.contains("since"), "{bad}");
    m.refused("conversation_insights", json!({"top": "many"}))
        .await;
}

#[tokio::test]
async fn reading_changes_nothing() {
    let m = Mcp::new().await;
    seed(&m).await;
    let before = snapshot(&m).await;
    for args in [json!({}), json!({"top": 3, "since": "7d"})] {
        m.ok("conversation_insights", args).await;
    }
    assert_eq!(snapshot(&m).await, before);
}

#[tokio::test]
async fn an_empty_install_gives_empty_results() {
    let m = Mcp::new().await;
    let out = m.ok("conversation_insights", json!({})).await;
    assert_eq!(out["patterns"], json!([]));
    assert_eq!(out["counts"]["conversations"], 1);
    assert_eq!(out["truncated"], false);
}
