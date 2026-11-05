use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::model::{EntryBody, NewTask, Task, TaskState, MAX_HANDOFFS};

mod common;
mod mcp_support;
use common::call;
use mcp_support::Mcp;

const CODER: &str = "name = \"coder\"\nadapter = \"claude\"\n";
const TESTER: &str = "name = \"tester\"\nadapter = \"claude\"\n";
const SHIPPER: &str = "name = \"shipper\"\nadapter = \"claude\"\ncan_hand_back_to = [\"coder\"]\n";

async fn running(m: &Mcp, name: &str, agent: &str) {
    for source in [CODER, TESTER, SHIPPER] {
        m.store().save_agent_type(source).await.unwrap();
    }
    let new = NewTask {
        name: name.into(),
        title: "fix".into(),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: agent.into(),
    };
    m.store().create_task(&m.id, new).await.unwrap();
    set_state(m, name, TaskState::Working).await;
}

async fn set_state(m: &Mcp, name: &str, state: TaskState) {
    let change = move |t: Task| Ok(Task { state, ..t });
    m.store().update_task(name, change).await.unwrap();
}

async fn rpc(m: &Mcp, task: &str, body: Value) -> (StatusCode, Value) {
    call(
        &m.app,
        "POST",
        &format!("/api/tasks/{task}/mcp"),
        Some(body),
    )
    .await
}

async fn hand_back(m: &Mcp, task: &str, arguments: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                      "params": {"name": "hand_back", "arguments": arguments}});
    let (status, answer) = rpc(m, task, body).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer["result"].clone()
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn a_task_agent_sees_only_hand_back() {
    let m = Mcp::new().await;
    running(&m, "fix-login", "shipper").await;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let (status, answer) = rpc(&m, "fix-login", body).await;
    assert_eq!(status, StatusCode::OK);
    let tools = answer["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "hand_back");
    assert_eq!(tools[0]["inputSchema"]["required"], json!(["to", "reason"]));
    let (status, _) = rpc(
        &m,
        "ghost",
        json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn hand_back_queues_the_task_for_the_target_type() {
    let m = Mcp::new().await;
    running(&m, "fix-login", "shipper").await;
    let result = hand_back(
        &m,
        "fix-login",
        json!({"to": "coder", "reason": "lint fails", "summary": "opened the pr"}),
    )
    .await;
    assert!(result.get("isError").is_none(), "{result}");
    assert!(text(&result).contains("coder"), "{result}");
    let task = m.store().find_task("fix-login").await.unwrap().unwrap();
    assert_eq!(
        (task.state, task.agent.as_str()),
        (TaskState::Queued, "coder")
    );
    let handoffs = m.store().handoffs("fix-login").await.unwrap();
    assert_eq!(handoffs.len(), 1);
    assert_eq!(handoffs[0].reason, "lint fails");
    assert!(handoffs[0].summary.contains("opened the pr"));
    let entries = m.store().entries(&m.id).await.unwrap();
    assert!(matches!(
        &entries.last().unwrap().body,
        EntryBody::Handoff { from, to, .. } if from == "shipper" && to == "coder"
    ));
}

#[tokio::test]
async fn hand_back_refuses_unknown_and_unlisted_types_and_blank_reasons() {
    let m = Mcp::new().await;
    running(&m, "fix-login", "shipper").await;
    let unknown = hand_back(&m, "fix-login", json!({"to": "ghost", "reason": "r"})).await;
    assert_eq!(unknown["isError"], true);
    assert!(
        text(&unknown).contains("unknown agent type `ghost`"),
        "{unknown}"
    );
    let unlisted = hand_back(&m, "fix-login", json!({"to": "tester", "reason": "r"})).await;
    assert_eq!(unlisted["isError"], true);
    assert!(
        text(&unlisted).contains("you can hand back to: coder"),
        "{unlisted}"
    );
    let blank = hand_back(&m, "fix-login", json!({"to": "coder", "reason": "  "})).await;
    assert_eq!(blank["isError"], true);
    let task = m.store().find_task("fix-login").await.unwrap().unwrap();
    assert_eq!(
        (task.state, task.agent.as_str()),
        (TaskState::Working, "shipper")
    );
}

#[tokio::test]
async fn a_type_without_a_list_may_hand_back_to_any_other_type() {
    let m = Mcp::new().await;
    running(&m, "fix-login", "coder").await;
    let result = hand_back(&m, "fix-login", json!({"to": "tester", "reason": "r"})).await;
    assert!(result.get("isError").is_none(), "{result}");
    let itself = hand_back(&m, "fix-login", json!({"to": "tester", "reason": "r"})).await;
    assert_eq!(
        itself["isError"], true,
        "a queued task cannot be handed back again"
    );
}

#[tokio::test]
async fn a_chain_of_hand_backs_ends() {
    let m = Mcp::new().await;
    running(&m, "fix-login", "coder").await;
    for _ in 0..MAX_HANDOFFS {
        let result = hand_back(&m, "fix-login", json!({"to": "tester", "reason": "r"})).await;
        assert!(result.get("isError").is_none(), "{result}");
        set_state(&m, "fix-login", TaskState::Working).await;
        m.store()
            .update_task("fix-login", |t| {
                Ok(Task {
                    agent: "coder".into(),
                    ..t
                })
            })
            .await
            .unwrap();
    }
    let last = hand_back(&m, "fix-login", json!({"to": "tester", "reason": "r"})).await;
    assert_eq!(last["isError"], true);
    assert!(text(&last).contains(&MAX_HANDOFFS.to_string()), "{last}");
}
