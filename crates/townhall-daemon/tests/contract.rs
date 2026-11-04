use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::model::{AgentEvent, EntryBody, NewTask};
use townhall_core::ContextOptions;
use townhall_daemon::router;

mod common;
use common::{call, settle, unique, Env};

const STUBS: [(&str, &str); 17] = [
    ("GET", "/api/conversations"),
    ("POST", "/api/conversations"),
    ("GET", "/api/conversations/abc"),
    ("PATCH", "/api/conversations/abc"),
    ("POST", "/api/conversations/abc/messages"),
    ("POST", "/api/conversations/abc/stop"),
    ("POST", "/api/conversations/abc/end"),
    ("POST", "/api/conversations/abc/mcp"),
    ("GET", "/api/conversations/abc/attachments"),
    ("POST", "/api/conversations/abc/attachments"),
    ("GET", "/api/conversations/abc/attachments/def"),
    ("GET", "/api/tasks"),
    ("GET", "/api/tasks/fix-login"),
    ("GET", "/api/agents/x/steps"),
    ("GET", "/api/stats"),
    ("GET", "/api/stats?repo=alpha"),
    ("GET", "/api/events"),
];

#[tokio::test]
async fn routes_of_later_units_answer_501() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    for (method, uri) in STUBS {
        let (s, v) = call(&app, method, uri, Some(json!({}))).await;
        assert_eq!(s, StatusCode::NOT_IMPLEMENTED, "{method} {uri}");
        assert_eq!(
            v,
            json!({"success": false, "data": null, "error": "not implemented yet"})
        );
    }
}

#[tokio::test]
async fn info_keeps_its_shape_and_adds_the_user_name() {
    let env = Env::new("alpha").await;
    let (s, v) = call(&env.start().1, "GET", "/api/info", None).await;
    assert_eq!(s, StatusCode::OK);
    let keys: Vec<&String> = v["data"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["home", "repo", "repoKey", "userName"]);
    assert!(v["data"]["userName"].is_string() || v["data"]["userName"].is_null());
}

fn fake_claude(dir: &Path) -> String {
    let path = dir.join("fake-claude");
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\nexec sleep 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

async fn launch_args(app: &axum::Router, body: Value) -> Vec<String> {
    let (s, v) = call(app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    settle().await;
    let worktree = v["data"]["worktree"].as_str().unwrap();
    let args = std::fs::read_to_string(Path::new(worktree).join("args.txt")).unwrap();
    let name = v["data"]["name"].as_str().unwrap();
    call(app, "DELETE", &format!("/api/agents/{name}"), None).await;
    args.lines().map(String::from).collect()
}

#[tokio::test]
async fn the_permission_mode_comes_from_settings_only() {
    let env = Env::new("alpha").await;
    let options = ContextOptions {
        claude_bin: fake_claude(env.root.path()),
        ..ContextOptions::default()
    };
    let app = router(env.state_with(options));
    let sneaky = json!({"name": unique("c"), "adapter": "claude", "permissionMode": "bypassPermissions", "prompt": "hi"});
    let args = launch_args(&app, sneaky).await;
    assert_eq!(args, ["--permission-mode", "acceptEdits", "hi"]);

    let skip = json!({"value": "bypassPermissions"});
    let (s, v) = call(&app, "PUT", "/api/settings/permissionMode", Some(skip)).await;
    assert_eq!(
        (s, &v["data"]["permissionMode"]),
        (StatusCode::OK, &json!("bypassPermissions"))
    );
    let args = launch_args(&app, json!({"name": unique("c"), "adapter": "claude"})).await;
    assert_eq!(args, ["--dangerously-skip-permissions"]);
}

#[tokio::test]
async fn sending_to_a_task_agent_notes_it_in_the_conversation() {
    let env = Env::new("alpha").await;
    let state = env.state();
    let app = router(state.clone());
    let store = state.ctx().store().unwrap();
    let name = unique("task");
    let convo = store.create_conversation().await.unwrap();
    let task = NewTask {
        name: name.clone(),
        title: "a task".into(),
        prompt: "p".into(),
        repo: "alpha".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
    };
    store.create_task(&convo.id, task).await.unwrap();
    let body = json!({"name": name, "adapter": "shell", "command": "cat"});
    let (s, _) = call(&app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED);
    let mut events = state.ctx().bus().subscribe();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/agents/{name}/send"),
        Some(json!({"text": "y"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let entries = store.entries(&convo.id).await.unwrap();
    assert_eq!(
        entries.last().unwrap().body,
        EntryBody::Agent {
            agent: name.clone(),
            event: AgentEvent::Sent,
            text: "y".into()
        }
    );
    let published = events.recv().await.unwrap();
    assert_eq!(
        serde_json::to_value(published).unwrap()["type"],
        "entryAppended"
    );
    call(&app, "DELETE", &format!("/api/agents/{name}"), None).await;
}

#[tokio::test]
async fn settings_are_read_and_put_one_key_at_a_time() {
    let env = Env::new("alpha").await;
    let app = router(env.state());
    let (s, v) = call(&app, "GET", "/api/settings", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        v["data"],
        json!({
            "permissionMode": "acceptEdits", "workWithoutAsking": false, "agentsAtOnce": 6,
            "repoUnclear": "ask", "sessionPanelOpen": true, "notifyWhen": "needsYou"
        })
    );
    let (s, v) = call(
        &app,
        "PUT",
        "/api/settings/agentsAtOnce",
        Some(json!({"value": 12})),
    )
    .await;
    assert_eq!(
        (s, &v["data"]["agentsAtOnce"]),
        (StatusCode::OK, &json!(12))
    );
    let (_, v) = call(&app, "GET", "/api/settings", None).await;
    assert_eq!(v["data"]["agentsAtOnce"], 12);
    let bad = [
        ("/api/settings/theme", json!({"value": "dark"})),
        ("/api/settings/agentsAtOnce", json!({"value": 51})),
        ("/api/settings/permissionMode", json!({"value": "yolo"})),
        ("/api/settings/agentsAtOnce", json!({"v": 3})),
    ];
    for (uri, body) in bad {
        let (s, v) = call(&app, "PUT", uri, Some(body.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{uri} {body}: {v}");
    }
    let (s, _) = call(
        &app,
        "PATCH",
        "/api/settings",
        Some(json!({"agentsAtOnce": 3})),
    )
    .await;
    assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
}
