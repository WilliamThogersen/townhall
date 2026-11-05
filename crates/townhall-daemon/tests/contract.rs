use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::model::{AgentEvent, EntryBody, NewTask};
use townhall_core::ContextOptions;
use townhall_daemon::router;
use townhall_daemon::state::AppState;

const ARGS_DEADLINE: Duration = Duration::from_secs(5);

mod common;
use common::{call, unique, Env};

const STUBS: &[(&str, &str)] = &[
    ("GET", "/api/library/agents"),
    ("GET", "/api/library/agents/coder"),
    ("PUT", "/api/library/agents/coder"),
    ("DELETE", "/api/library/agents/coder"),
    ("GET", "/api/library/workflows"),
    ("GET", "/api/library/workflows/ship"),
    ("PUT", "/api/library/workflows/ship"),
    ("DELETE", "/api/library/workflows/ship"),
    ("POST", "/api/library/import"),
    ("POST", "/api/workflows/ship/runs"),
    ("GET", "/api/runs/r1"),
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
        "#!/bin/sh\nprintf '%s\\0' \"$@\" > args.tmp && mv args.tmp args.txt\nexec sleep 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

/// The fake claude's argv, once it has written it: tmux can take a while to start it.
async fn written_args(worktree: &Path) -> Vec<String> {
    let path = worktree.join("args.txt");
    let deadline = Instant::now() + ARGS_DEADLINE;
    loop {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let text = text.strip_suffix('\0').unwrap_or(&text);
                return text.split('\0').map(String::from).collect();
            }
            Err(e) if Instant::now() < deadline => {
                assert_eq!(e.kind(), std::io::ErrorKind::NotFound, "{e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => panic!("fake claude never wrote {}: {e}", path.display()),
        }
    }
}

async fn launch_args(app: &axum::Router, body: Value) -> Vec<String> {
    let (s, v) = call(app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let args = written_args(Path::new(v["data"]["worktree"].as_str().unwrap())).await;
    let name = v["data"]["name"].as_str().unwrap();
    call(app, "DELETE", &format!("/api/agents/{name}"), None).await;
    args
}

#[tokio::test]
async fn the_permission_mode_comes_from_settings_only() {
    let env = Env::new("alpha").await;
    let options = ContextOptions {
        claude_bin: fake_claude(env.root.path()),
        ..ContextOptions::default()
    };
    let app = router(env.state_with(options));
    let sneaky = json!({
        "name": unique("c"), "adapter": "claude", "permissionMode": "bypassPermissions",
        "prompt": "--dangerously-skip-permissions"
    });
    let args = launch_args(&app, sneaky).await;
    assert_eq!(
        args,
        [
            "--permission-mode=acceptEdits",
            "--",
            "--dangerously-skip-permissions\n"
        ]
    );

    let skip = json!({"value": "bypassPermissions"});
    let (s, v) = call(&app, "PUT", "/api/settings/permissionMode", Some(skip)).await;
    assert_eq!(
        (s, &v["data"]["permissionMode"]),
        (StatusCode::OK, &json!("bypassPermissions"))
    );
    let args = launch_args(&app, json!({"name": unique("c"), "adapter": "claude"})).await;
    assert_eq!(args, ["--dangerously-skip-permissions"]);
}

/// A task's agent running `cat`, in a new conversation: the agent's name and the
/// conversation id.
async fn task_agent(state: &AppState, app: &axum::Router) -> (String, String) {
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
    let (s, _) = call(app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED);
    (name, convo.id)
}

/// The text of the conversation's last entry, which must be an `agent` `sent` one.
async fn last_sent(state: &AppState, convo: &str) -> String {
    let entries = state.ctx().store().unwrap().entries(convo).await.unwrap();
    match &entries.last().unwrap().body {
        EntryBody::Agent {
            event: AgentEvent::Sent,
            text,
            ..
        } => text.clone(),
        other => panic!("expected a sent entry, got {other:?}"),
    }
}

#[tokio::test]
async fn sending_to_a_task_agent_notes_it_in_the_conversation() {
    let env = Env::new("alpha").await;
    let state = env.state();
    let app = router(state.clone());
    let store = state.ctx().store().unwrap();
    let (name, convo) = task_agent(&state, &app).await;
    let mut events = state.ctx().bus().subscribe();
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/agents/{name}/send"),
        Some(json!({"text": "y"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let entries = store.entries(&convo).await.unwrap();
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
async fn send_takes_text_or_one_key_and_notes_the_key_as_a_symbol() {
    let env = Env::new("alpha").await;
    let state = env.state();
    let app = router(state.clone());
    let (name, convo) = task_agent(&state, &app).await;
    let uri = format!("/api/agents/{name}/send");
    let keys = [
        ("up", "\u{2191}"),
        ("down", "\u{2193}"),
        ("enter", "\u{23ce}"),
        ("escape", "esc"),
        ("tab", "tab"),
    ];
    for (key, symbol) in keys {
        let (s, v) = call(&app, "POST", &uri, Some(json!({"key": key}))).await;
        assert_eq!(
            (s, &v["data"]),
            (StatusCode::OK, &json!(name)),
            "{key}: {v}"
        );
        assert_eq!(last_sent(&state, &convo).await, symbol);
    }
    let bad = [
        json!({}),
        json!({"text": "y", "key": "enter"}),
        json!({"text": null, "key": null}),
        json!({"key": "Down"}),
        json!({"key": "C-c"}),
    ];
    for body in bad {
        let (s, v) = call(&app, "POST", &uri, Some(body.clone())).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body}: {v}");
    }
    let (s, v) = call(&app, "POST", &uri, Some(json!({"text": "y"}))).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &json!(name)));
    assert_eq!(last_sent(&state, &convo).await, "y");
    let ghost = format!("/api/agents/{}/send", unique("ghost"));
    let (s, _) = call(&app, "POST", &ghost, Some(json!({"key": "up"}))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
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
            "repoUnclear": "ask", "sessionPanelOpen": true, "notifyWhen": "needsYou",
            "townhallModel": "sonnet"
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
    let put = Some(json!({"value": "haiku"}));
    let (s, v) = call(&app, "PUT", "/api/settings/townhallModel", put).await;
    assert_eq!(
        (s, &v["data"]["townhallModel"]),
        (StatusCode::OK, &json!("haiku"))
    );
    let bad = [
        ("/api/settings/theme", json!({"value": "dark"})),
        ("/api/settings/agentsAtOnce", json!({"value": 51})),
        ("/api/settings/permissionMode", json!({"value": "yolo"})),
        ("/api/settings/townhallModel", json!({"value": "gpt-5"})),
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
