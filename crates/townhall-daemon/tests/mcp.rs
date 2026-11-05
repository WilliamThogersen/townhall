use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;
mod mcp_support;
use common::call;
use mcp_support::Mcp;

#[tokio::test]
async fn initialize_echoes_the_version_and_lists_the_tools() {
    let m = Mcp::new().await;
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                      "params": {"protocolVersion": "2025-03-26", "capabilities": {}}});
    let answer = m.rpc(init).await;
    assert_eq!(answer["jsonrpc"], "2.0");
    assert_eq!(answer["id"], 1);
    let result = &answer["result"];
    assert_eq!(result["protocolVersion"], "2025-03-26");
    assert_eq!(result["capabilities"], json!({"tools": {}}));
    assert_eq!(result["serverInfo"]["name"], "townhall");
    assert!(result["serverInfo"]["version"].is_string());
    assert_eq!(result["instructions"], townhall_core::mcp::INSTRUCTIONS);

    let bare = json!({"jsonrpc": "2.0", "id": "x", "method": "initialize"});
    assert_eq!(
        m.rpc(bare).await["result"]["protocolVersion"],
        townhall_core::mcp::PROTOCOL_VERSION
    );
    let ping = m
        .rpc(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}))
        .await;
    assert_eq!(ping, json!({"jsonrpc": "2.0", "id": 2, "result": {}}));

    let list = m
        .rpc(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}))
        .await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "create_task",
            "set_title",
            "task_status",
            "list_agent_types",
            "read_agent_type",
            "save_agent_type"
        ]
    );
    for t in tools {
        assert!(t["description"].is_string());
        assert_eq!(t["inputSchema"]["type"], "object");
        assert!(!t["inputSchema"].to_string().contains("ermission"), "{t}");
    }
}

#[tokio::test]
async fn notifications_get_202_and_batches_answer_in_order() {
    let m = Mcp::new().await;
    let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    assert_eq!(
        m.post(note.clone()).await,
        (StatusCode::ACCEPTED, String::new())
    );
    let batch = json!([
        {"jsonrpc": "2.0", "id": 1, "method": "ping"},
        note,
        {"jsonrpc": "2.0", "id": "b", "method": "tools/list"},
        {"jsonrpc": "2.0", "id": 4, "method": "nope"}
    ]);
    let answers = m.rpc(batch).await;
    let answers = answers.as_array().unwrap();
    assert_eq!(answers.len(), 3);
    assert_eq!(answers[0], json!({"jsonrpc": "2.0", "id": 1, "result": {}}));
    assert_eq!(answers[1]["id"], "b");
    assert!(answers[1]["result"]["tools"].is_array());
    assert_eq!(answers[2]["error"]["code"], -32601);
    let only_notes = json!([{"jsonrpc": "2.0", "method": "notifications/initialized"}]);
    assert_eq!(
        m.post(only_notes).await,
        (StatusCode::ACCEPTED, String::new())
    );
    let empty = m.rpc(json!([])).await;
    assert_eq!(empty["error"]["code"], -32600);
    assert_eq!(empty["id"], Value::Null);
    let invalid = m.rpc(json!({"id": 5, "method": "ping"})).await;
    assert_eq!(invalid["error"]["code"], -32600);
}

#[tokio::test]
async fn the_endpoint_checks_the_conversation_and_the_body() {
    let m = Mcp::new().await;
    let ping = json!({"jsonrpc": "2.0", "id": 1, "method": "ping"});
    let (s, v) = call(
        &m.app,
        "POST",
        "/api/conversations/nope00000000/mcp",
        Some(ping),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["success"], false);
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/conversations/{}/mcp", m.id))
        .header("content-type", "application/json")
        .body(Body::from("{not json"))
        .unwrap();
    let res = m.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let no_name = json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {}});
    assert_eq!(m.rpc(no_name).await["error"]["code"], -32602);
    let unknown = m.refused("delete_everything", json!({})).await;
    assert!(unknown.contains("create_task"), "{unknown}");
}

#[tokio::test]
async fn set_title_names_the_conversation() {
    let m = Mcp::new().await;
    let named = m
        .ok("set_title", json!({"title": "  fix the\n login   loop "}))
        .await;
    assert_eq!(named["title"], "fix the login loop");
    assert_eq!(named["id"], m.id.as_str());
    assert_eq!(
        m.store().conversation(&m.id).await.unwrap().title,
        "fix the login loop"
    );
    for bad in [
        json!({"title": " "}),
        json!({"title": "t".repeat(81)}),
        json!({"title": "ok", "extra": 1}),
        json!({}),
    ] {
        m.refused("set_title", bad).await;
    }
}
