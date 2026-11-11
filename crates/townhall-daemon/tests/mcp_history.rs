use serde_json::{json, Value};
use townhall_core::model::{Checks, EntryBody, NewTask, Pr, Task, TaskState};

mod common;
mod mcp_support;
use common::call;
use mcp_support::Mcp;

const CODER: &str = "name = \"coder\"\nadapter = \"claude\"\n";

async fn names(answer: &Value) -> Vec<String> {
    answer["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

async fn make_task(m: &Mcp, name: &str, title: &str) {
    let new = NewTask {
        name: name.into(),
        title: title.into(),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "coder".into(),
        pr_base: None,
        done: Default::default(),
    };
    m.store().create_task(&m.id, new).await.unwrap();
}

async fn event(m: &Mcp, text: &str) {
    let body = EntryBody::Event { text: text.into() };
    m.store().append_entry(&m.id, body).await.unwrap();
}

async fn seed(m: &Mcp) {
    m.store().save_agent_type(CODER).await.unwrap();
    m.you("should we move billing to stripe?").await;
    let reply = EntryBody::Townhall {
        text: "stripe is fine, create a task".into(),
    };
    m.store().append_entry(&m.id, reply).await.unwrap();
    make_task(m, "stripe-move", "move billing to stripe").await;
    m.store()
        .update_task("stripe-move", |t| {
            Ok(Task {
                state: TaskState::Merged,
                pr: Some(Pr {
                    number: 7,
                    url: Some("https://git.example/shop/pull/7".into()),
                    checks: Checks::Pending,
                }),
                ..t
            })
        })
        .await
        .unwrap();
    event(
        m,
        "task stripe-move ended (done). its agent's last message: rejected the webhook rewrite token=hunter2hunter2",
    )
    .await;
    event(m, "townhall event: unrelated chatter about stripe").await;
}

#[tokio::test]
async fn both_tools_go_to_the_planner_and_to_every_agent() {
    let m = Mcp::new().await;
    seed(&m).await;
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let planner = names(&m.rpc(list.clone()).await).await;
    let (status, answer) = call(&m.app, "POST", "/api/tasks/stripe-move/mcp", Some(list)).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{answer}");
    let agent = names(&answer).await;
    for tool in ["search_history", "read_conversation"] {
        assert!(planner.contains(&tool.to_string()), "{tool}");
        assert!(agent.contains(&tool.to_string()), "{tool}");
        let allowed = format!("mcp__townhall__{tool}");
        assert!(townhall_core::mcp::allowed_tools().contains(&allowed));
        assert!(townhall_core::mcp::task_allowed_tools().contains(&allowed));
    }
    let find = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "search_history", "arguments": {"query": "stripe"}}});
    let (_, answer) = call(&m.app, "POST", "/api/tasks/stripe-move/mcp", Some(find)).await;
    assert_ne!(answer["result"]["isError"], true, "{answer}");
}

#[tokio::test]
async fn search_history_round_trip() {
    let m = Mcp::new().await;
    seed(&m).await;
    let out = m.ok("search_history", json!({"query": "stripe"})).await;
    let hits = out["hits"].as_array().unwrap();
    let mut kinds: Vec<&str> = hits.iter().map(|h| h["kind"].as_str().unwrap()).collect();
    kinds.sort_unstable();
    assert_eq!(
        kinds,
        ["agent_report", "message", "message", "task"],
        "{out}"
    );
    assert!(!out.to_string().contains("chatter"), "{out}");
    let task = hits.iter().find(|h| h["kind"] == "task").unwrap();
    assert_eq!(task["conversation"], m.id.as_str());
    assert_eq!(task["task"], "stripe-move");
    assert_eq!(task["agent"], "coder");
    assert_eq!(task["state"], "merged");
    assert_eq!(task["branch"], "townhall/stripe-move");
    assert_eq!(task["pr"], 7);
    assert_eq!(task["prUrl"], "https://git.example/shop/pull/7");
    assert!(task["snippet"].as_str().unwrap().contains("[stripe]"));
    assert!(task["at"].is_string() && task["title"].is_string());
    let message = hits.iter().find(|h| h["kind"] == "message").unwrap();
    assert!(message.get("task").is_none() && message.get("pr").is_none());

    let rejected = m
        .ok(
            "search_history",
            json!({"query": "rejected webhook", "kind": ["agent_report"], "since": "1h", "limit": 5, "repo": "shop"}),
        )
        .await;
    let hits = rejected["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{rejected}");
    assert_eq!(hits[0]["task"], "stripe-move");
    let text = rejected.to_string();
    assert!(!text.contains("hunter2"), "{text}");

    let none = m
        .ok(
            "search_history",
            json!({"query": "stripe", "since": "2999-01-01T00:00:00Z"}),
        )
        .await;
    assert_eq!(none["hits"], json!([]));
    let other = m
        .ok(
            "search_history",
            json!({"query": "stripe", "repo": "admin"}),
        )
        .await;
    assert_eq!(other["hits"], json!([]));
}

#[tokio::test]
async fn search_history_refuses_bad_arguments() {
    let m = Mcp::new().await;
    seed(&m).await;
    m.refused("search_history", json!({})).await;
    m.refused("search_history", json!({"query": "  "})).await;
    m.refused("search_history", json!({"query": "a", "nope": 1}))
        .await;
    m.refused("search_history", json!({"query": "a", "kind": ["chat"]}))
        .await;
    m.refused("search_history", json!({"query": "a", "limit": "many"}))
        .await;
    let since = m
        .refused(
            "search_history",
            json!({"query": "a", "since": "yesterday"}),
        )
        .await;
    assert!(since.contains("since"), "{since}");
    let syntax = m
        .refused("search_history", json!({"query": "\"open"}))
        .await;
    assert!(syntax.contains("double quotes"), "{syntax}");
}

#[tokio::test]
async fn search_history_caps_the_limit_at_fifty() {
    let m = Mcp::new().await;
    for n in 0..60 {
        m.you(&format!("crowded note {n}")).await;
    }
    let default = m.ok("search_history", json!({"query": "crowded"})).await;
    assert_eq!(default["hits"].as_array().unwrap().len(), 10);
    let max = m
        .ok("search_history", json!({"query": "crowded", "limit": 500}))
        .await;
    assert_eq!(max["hits"].as_array().unwrap().len(), 50);
}

#[tokio::test]
async fn read_conversation_is_compact_and_trimmed() {
    let m = Mcp::new().await;
    seed(&m).await;
    let long = "word ".repeat(400);
    m.you(&long).await;
    event(
        &m,
        &format!(
            "task stripe-move ended (done). its agent's last message: {}",
            "x".repeat(2000)
        ),
    )
    .await;
    let out = m.ok("read_conversation", json!({"id": m.id})).await;
    assert_eq!(out["id"], m.id.as_str());
    assert!(out["created"].is_string() && out["updated"].is_string());
    let messages = out["messages"].as_array().unwrap();
    assert_eq!(out["total"], 3);
    assert_eq!(messages.len(), 3, "{out}");
    assert_eq!(messages[0]["by"], "you");
    assert_eq!(messages[1]["by"], "planner");
    let last = messages[2]["text"].as_str().unwrap();
    assert!(
        last.chars().count() <= 600 && last.ends_with('\u{2026}'),
        "{last}"
    );
    let tasks = out["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["name"], "stripe-move");
    assert_eq!(tasks[0]["agent"], "coder");
    assert_eq!(tasks[0]["state"], "merged");
    assert_eq!(tasks[0]["branch"], "townhall/stripe-move");
    assert_eq!(tasks[0]["pr"], 7);
    let said = tasks[0]["lastMessage"].as_str().unwrap();
    assert!(
        said.chars().count() <= 600 && said.starts_with("xxx"),
        "{said}"
    );
    assert!(!out.to_string().contains("chatter"), "{out}");
}

#[tokio::test]
async fn read_conversation_pages_and_redacts() {
    let m = Mcp::new().await;
    seed(&m).await;
    m.you("deploy now token=hunter2hunter2").await;
    let page = m
        .ok(
            "read_conversation",
            json!({"id": m.id, "offset": 1, "limit": 1}),
        )
        .await;
    assert_eq!(page["total"], 3);
    assert_eq!(page["offset"], 1);
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    assert_eq!(page["messages"][0]["by"], "planner");
    let all = m.ok("read_conversation", json!({"id": m.id})).await;
    let text = all.to_string();
    assert!(!text.contains("hunter2"), "{text}");
    assert!(text.contains("REDACTED"), "{text}");
    let tail = m
        .ok("read_conversation", json!({"id": m.id, "offset": 99}))
        .await;
    assert_eq!(tail["messages"], json!([]));
}

#[tokio::test]
async fn read_conversation_refuses_unknown_ids_and_bad_arguments() {
    let m = Mcp::new().await;
    m.refused("read_conversation", json!({"id": "nope"})).await;
    m.refused("read_conversation", json!({})).await;
    m.refused("read_conversation", json!({"id": m.id, "limit": "x"}))
        .await;
    m.refused("read_conversation", json!({"id": m.id, "extra": 1}))
        .await;
}
