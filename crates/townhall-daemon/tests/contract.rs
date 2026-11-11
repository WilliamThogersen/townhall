use axum::http::StatusCode;
use serde_json::json;
use townhall_core::model::{AgentEvent, EntryBody, NewTask};
use townhall_daemon::router;
use townhall_daemon::state::AppState;

mod common;
use common::{call, unique, Env};

const STUBS: &[(&str, &str)] = &[("POST", "/api/workflows/ship/runs")];

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
    assert_eq!(keys, ["home", "repo", "repoKey", "userName", "version"]);
    assert!(v["data"]["userName"].is_string() || v["data"]["userName"].is_null());
}

/// A task's agent running `cat`, in a new conversation: the agent's name and the
/// conversation id.
async fn task_agent(state: &AppState) -> (String, String) {
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
        agent: "builder".into(),
        pr_base: None,
        done: Default::default(),
    };
    store.create_task(&convo.id, task).await.unwrap();
    common::start_shell(state.ctx().startup(), &name, "cat").await;
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
    let (name, convo) = task_agent(&state).await;
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
    let (name, convo) = task_agent(&state).await;
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
            "agentsAtOnce": 6,
            "repoUnclear": "ask", "sessionPanelOpen": true, "notifyWhen": "needsYou",
            "minFreeDiskGb": 5, "learnAboutMe": false,
            "githubAccount": "WilliamThogersen", "forgeAccounts": [],
            "reflectAfterTasks": false, "clonesDir": ""
        })
    );
    let put = Some(json!({"value": [{"forge": "gitlab", "login": "me"}]}));
    let (s, v) = call(&app, "PUT", "/api/settings/forgeAccounts", put).await;
    assert_eq!(
        (s, &v["data"]["forgeAccounts"]),
        (
            StatusCode::OK,
            &json!([{"forge": "gitlab", "login": "me", "host": "gitlab.com", "label": "me"}])
        )
    );
    let put = Some(json!({"value": [{"forge": "gitlab", "login": "a b"}]}));
    let (s, _) = call(&app, "PUT", "/api/settings/forgeAccounts", put).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let put = Some(json!({"value": 0}));
    let (s, v) = call(&app, "PUT", "/api/settings/minFreeDiskGb", put).await;
    assert_eq!(
        (s, &v["data"]["minFreeDiskGb"]),
        (StatusCode::OK, &json!(0))
    );
    let put = Some(json!({"value": true}));
    let (s, _) = call(&app, "PUT", "/api/settings/sharedCargoTarget", put).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let put = Some(json!({"value": 100_001}));
    let (s, _) = call(&app, "PUT", "/api/settings/minFreeDiskGb", put).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
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
    let put = Some(json!({"value": "octocat"}));
    let (s, v) = call(&app, "PUT", "/api/settings/githubAccount", put).await;
    assert_eq!(
        (s, &v["data"]["githubAccount"]),
        (StatusCode::OK, &json!("octocat"))
    );
    let bad = [
        (
            "/api/settings/githubAccount",
            json!({"value": "not a login"}),
        ),
        ("/api/settings/theme", json!({"value": "dark"})),
        ("/api/settings/agentsAtOnce", json!({"value": 51})),
        ("/api/settings/permissionMode", json!({"value": "plan"})),
        ("/api/settings/workWithoutAsking", json!({"value": true})),
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
