use std::sync::OnceLock;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use townhall_core::agent::state::PROJECTS_ENV;
use townhall_core::model::{NewTask, SettingKey};
use townhall_core::plugins::tokens;
use townhall_core::store::{Marketplace, Plugin, Store};
use townhall_core::tmux;

mod common;
use common::{call, send, unique, Env};

static PROJECTS: OnceLock<std::path::PathBuf> = OnceLock::new();

fn temp_projects() {
    PROJECTS.get_or_init(|| {
        let dir = tempfile::Builder::new()
            .prefix("townhall-daemon-projects")
            .tempdir()
            .unwrap()
            .keep();
        std::env::set_var(PROJECTS_ENV, &dir);
        dir
    });
}

struct Session(String);

impl Drop for Session {
    fn drop(&mut self) {
        let target = format!("={}", self.0);
        let _ = common::tmux()
            .args(["kill-session", "-t", &target])
            .output();
    }
}

const EVERYTHING: [&str; 5] = [
    "api:tasks:read",
    "api:conversations:read",
    "api:settings:read",
    "api:storage",
    "hook:task.ended",
];

async fn plugin(store: &Store, name: &str, approved: &[&str]) -> String {
    if store.marketplace("acme").await.is_err() {
        store
            .add_marketplace(Marketplace {
                name: "acme".into(),
                source: json!({"source": "github", "repo": "acme/market"}),
                account: None,
                enabled: true,
                official: false,
                head_sha: None,
                fetched_at: None,
                error: None,
            })
            .await
            .unwrap();
    }
    let added = store
        .add_plugin(Plugin {
            id: String::new(),
            marketplace: "acme".into(),
            name: name.into(),
            version: Some("1.0.0".into()),
            sha: "deadbeef".into(),
            source: json!({"source": "github", "repo": "acme/x"}),
            enabled: true,
            pinned: false,
            permissions_hash: "sha256:aa".into(),
            approved: json!(approved),
            installed_at: String::new(),
            updated_at: String::new(),
            error: None,
        })
        .await
        .unwrap();
    added.id
}

fn new_task(name: &str, repo: &str) -> NewTask {
    NewTask {
        name: name.into(),
        title: format!("do {name}"),
        prompt: "the brief".into(),
        repo: repo.into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "builder".into(),
        pr_base: None,
        done: Default::default(),
    }
}

async fn seed(store: &Store) -> (String, String) {
    let (a, b) = (
        store.create_conversation().await.unwrap().id,
        store.create_conversation().await.unwrap().id,
    );
    for (conversation, name) in [(&a, "one"), (&b, "two"), (&a, "three")] {
        store
            .create_task(conversation, new_task(name, "shop"))
            .await
            .unwrap();
    }
    (a, b)
}

async fn api(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/api/plugin/v1{path}"));
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let req = match body {
        Some(b) => builder
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => builder.body(Body::empty()),
    }
    .unwrap();
    send(app, req).await
}

async fn get(app: &Router, path: &str, token: &str) -> (StatusCode, Value) {
    api(app, "GET", path, Some(token), None).await
}

fn names(v: &Value) -> Vec<String> {
    v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn no_token_is_401_on_every_route() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    seed(state.ctx().store().unwrap()).await;
    let routes = [
        ("GET", "/info"),
        ("GET", "/tasks"),
        ("GET", "/tasks/one"),
        ("GET", "/tasks/one/artifacts"),
        ("GET", "/agents"),
        ("GET", "/stats"),
        ("GET", "/conversations"),
        ("GET", "/conversations/x"),
        ("GET", "/search?q=a"),
        ("GET", "/settings"),
        ("GET", "/storage/k"),
        ("PUT", "/storage/k"),
        ("DELETE", "/storage/k"),
    ];
    for (method, path) in routes {
        let body = (method == "PUT").then(|| json!({"value": "v"}));
        let (s, v) = api(&app, method, path, None, body).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{method} {path}: {v}");
        assert_eq!(v["success"], false);
        assert!(v["error"].as_str().is_some_and(|e| !e.is_empty()), "{v}");
        assert_eq!(v["data"], Value::Null);
    }
}

#[tokio::test]
async fn an_unknown_or_malformed_token_is_401() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let id = plugin(store, "lint", &EVERYTHING).await;
    let real = tokens::issue(store, &id, None).await.unwrap();
    let (s, _) = get(&app, "/info", &"ab".repeat(32)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = get(&app, "/info", "").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let basic = Request::builder()
        .uri("/api/plugin/v1/info")
        .header("authorization", format!("Basic {real}"))
        .body(Body::empty())
        .unwrap();
    let (s, _) = send(&app, basic).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let bare = Request::builder()
        .uri("/api/plugin/v1/info")
        .header("authorization", real.clone())
        .body(Body::empty())
        .unwrap();
    let (s, _) = send(&app, bare).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = get(&app, "/info", &real).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn a_revoked_token_is_401() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let id = plugin(store, "lint", &EVERYTHING).await;
    let live = tokens::issue(store, &id, None).await.unwrap();
    let (s, _) = get(&app, "/info", &live).await;
    assert_eq!(s, StatusCode::OK);
    tokens::revoke_plugin(store, &id).await.unwrap();
    let (s, v) = get(&app, "/info", &live).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "{v}");
}

#[tokio::test]
async fn disabling_the_plugin_over_the_ui_api_ends_its_access_at_once() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let id = plugin(store, "lint", &EVERYTHING).await;
    let off = call(
        &app,
        "PATCH",
        &format!("/api/plugins/{id}"),
        Some(json!({"enabled": false})),
    )
    .await;
    assert_eq!(off.0, StatusCode::OK, "{}", off.1);
    let on = call(
        &app,
        "PATCH",
        &format!("/api/plugins/{id}"),
        Some(json!({"enabled": true})),
    )
    .await;
    assert_eq!(on.0, StatusCode::OK, "{}", on.1);
    let live = tokens::issue(store, &id, None).await.unwrap();
    let (s, _) = get(&app, "/info", &live).await;
    assert_eq!(s, StatusCode::OK);
    let off = call(
        &app,
        "PATCH",
        &format!("/api/plugins/{id}"),
        Some(json!({"enabled": false})),
    )
    .await;
    assert_eq!(off.0, StatusCode::OK, "{}", off.1);
    let (s, _) = get(&app, "/info", &live).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn info_names_the_api_version_townhall_the_plugin_and_its_scopes() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let id = plugin(store, "lint", &EVERYTHING).await;
    let token = tokens::issue(store, &id, None).await.unwrap();
    let (s, v) = get(&app, "/info", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["success"], true);
    assert_eq!(v["data"]["apiVersion"], 1);
    assert_eq!(v["data"]["townhall"], townhall_core::version::VERSION);
    assert_eq!(v["data"]["plugin"], "lint@acme");
    assert_eq!(
        v["data"]["scopes"],
        json!([
            "tasks:read",
            "conversations:read",
            "settings:read",
            "storage"
        ])
    );
}

#[tokio::test]
async fn a_missing_scope_is_403_and_a_missing_token_still_401() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    seed(store).await;
    let only_storage = plugin(store, "stor", &["api:storage"]).await;
    let only_tasks = plugin(store, "tasks", &["api:tasks:read"]).await;
    let a = tokens::issue(store, &only_storage, None).await.unwrap();
    let b = tokens::issue(store, &only_tasks, None).await.unwrap();

    for path in [
        "/tasks",
        "/tasks/one",
        "/tasks/one/artifacts",
        "/agents",
        "/stats",
        "/conversations",
        "/conversations/x",
        "/search?q=a",
        "/settings",
    ] {
        let (s, v) = get(&app, path, &a).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{path}: {v}");
        assert_eq!(v["success"], false);
        assert!(v["error"].as_str().unwrap().contains("scope"), "{v}");
    }
    for path in [
        "/conversations",
        "/conversations/x",
        "/search?q=a",
        "/settings",
    ] {
        let (s, v) = get(&app, path, &b).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{path}: {v}");
    }
    for (method, body) in [
        ("GET", None),
        ("PUT", Some(json!({"value": "v"}))),
        ("DELETE", None),
    ] {
        let (s, v) = api(&app, method, "/storage/k", Some(&b), body).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{method}: {v}");
    }
    let (s, _) = get(&app, "/tasks", &b).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn tasks_read_mirrors_the_task_routes() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let (a, _) = seed(store).await;
    let id = plugin(store, "lint", &EVERYTHING).await;
    let token = tokens::issue(store, &id, None).await.unwrap();

    let (s, v) = get(&app, "/tasks", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(names(&v), ["one", "two", "three"]);
    let (_, legacy) = call(&app, "GET", "/api/tasks", None).await;
    assert_eq!(v, legacy);

    let (_, v) = get(&app, &format!("/tasks?conversation={a}"), &token).await;
    assert_eq!(names(&v), ["one", "three"]);
    let (s, v) = get(&app, "/tasks?state=done", &token).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");

    let (s, v) = get(&app, "/tasks/ONE", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["name"], "one");
    let (s, _) = get(&app, "/tasks/ghost", &token).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, v) = get(&app, "/tasks/one/artifacts", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"], json!([]));

    let (s, v) = get(&app, "/stats", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(v["data"]["days"].is_array());
}

#[tokio::test]
async fn conversations_read_mirrors_the_conversation_routes() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let (a, b) = seed(store).await;
    let id = plugin(store, "lint", &EVERYTHING).await;
    let token = tokens::issue(store, &id, None).await.unwrap();

    let (s, v) = get(&app, "/conversations", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (_, legacy) = call(&app, "GET", "/api/conversations", None).await;
    assert_eq!(v, legacy);
    assert_eq!(v["data"].as_array().unwrap().len(), 2);

    let (s, v) = get(&app, &format!("/conversations/{a}"), &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (_, legacy) = call(&app, "GET", &format!("/api/conversations/{a}"), None).await;
    assert_eq!(v, legacy);
    assert_eq!(v["data"]["tasks"].as_array().unwrap().len(), 2);
    let (s, _) = get(&app, &format!("/conversations/{b}"), &token).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = get(&app, "/conversations/nope", &token).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, v) = get(&app, "/search?q=", &token).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    let (s, v) = get(&app, "/search?q=anything", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(v["data"].is_array());
}

#[tokio::test]
async fn settings_read_is_townhalls_own_and_never_profile_or_forge_accounts() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    store
        .set_setting(SettingKey::GithubAccount, json!("work-me"))
        .await
        .unwrap();
    store
        .set_setting(SettingKey::ClonesDir, json!("/somewhere"))
        .await
        .unwrap();
    store
        .set_setting(SettingKey::AgentsAtOnce, json!(7))
        .await
        .unwrap();
    let id = plugin(store, "lint", &EVERYTHING).await;
    let token = tokens::issue(store, &id, None).await.unwrap();
    let (s, v) = get(&app, "/settings", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let mut keys: Vec<&str> = v["data"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "agentsAtOnce",
            "learnAboutMe",
            "minFreeDiskGb",
            "notifyWhen",
            "reflectAfterTasks",
            "repoUnclear",
            "sessionPanelOpen"
        ]
    );
    assert_eq!(v["data"]["agentsAtOnce"], 7);
    let text = v.to_string();
    assert!(
        !text.contains("work-me") && !text.contains("/somewhere"),
        "{text}"
    );
}

#[tokio::test]
async fn a_task_token_sees_only_its_task_and_conversation() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let (a, b) = seed(store).await;
    let id = plugin(store, "lint", &EVERYTHING).await;
    let token = tokens::issue(store, &id, Some("one")).await.unwrap();

    let (s, v) = get(&app, "/info", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"]["scopes"],
        json!(["tasks:read", "conversations:read", "storage"])
    );

    let (s, v) = get(&app, "/tasks", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(names(&v), ["one"]);
    let (s, v) = get(&app, "/tasks/one", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = get(&app, "/tasks/ONE/artifacts", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    for other in [
        "/tasks/two",
        "/tasks/three",
        "/tasks/two/artifacts",
        "/tasks/three/artifacts",
    ] {
        let (s, v) = get(&app, other, &token).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{other}: {v}");
    }

    let (s, v) = get(&app, "/conversations", &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let ids: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [a.as_str()]);
    let (s, v) = get(&app, &format!("/conversations/{a}"), &token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = get(&app, &format!("/conversations/{b}"), &token).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");

    for wide in ["/search?q=anything", "/stats", "/settings"] {
        let (s, v) = get(&app, wide, &token).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{wide}: {v}");
    }
}

#[tokio::test]
async fn a_task_token_sees_only_the_agent_of_its_task() {
    temp_projects();
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let ws = env.workspace(&env.startup);
    let (mine, other) = (unique("mine"), unique("other"));
    let _s1 = Session(tmux::session_name(ws.key(), &mine));
    let _s2 = Session(tmux::session_name(ws.key(), &other));
    common::start_shell(&ws, &mine, "sleep 30").await;
    common::start_shell(&ws, &other, "sleep 30").await;
    let id = plugin(store, "lint", &EVERYTHING).await;
    let wide = tokens::issue(store, &id, None).await.unwrap();
    let narrow = tokens::issue(store, &id, Some(&mine)).await.unwrap();
    let (s, v) = get(&app, "/agents", &wide).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let all = names(&v);
    assert!(all.contains(&mine) && all.contains(&other), "{all:?}");
    let (s, v) = get(&app, "/agents", &narrow).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(names(&v), [mine]);
}

#[tokio::test]
async fn storage_keeps_a_plugins_own_rows_with_caps() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let lint = plugin(store, "lint", &EVERYTHING).await;
    let fmt = plugin(store, "fmt", &EVERYTHING).await;
    let lint_token = tokens::issue(store, &lint, None).await.unwrap();
    let fmt_token = tokens::issue(store, &fmt, None).await.unwrap();

    let (s, v) = get(&app, "/storage/k", &lint_token).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    let put = api(
        &app,
        "PUT",
        "/storage/k",
        Some(&lint_token),
        Some(json!({"value": "one"})),
    )
    .await;
    assert_eq!(put.0, StatusCode::OK, "{}", put.1);
    let (s, v) = get(&app, "/storage/k", &lint_token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"], json!({"key": "k", "value": "one"}));
    let (s, v) = get(&app, "/storage/k", &fmt_token).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");

    let put = api(
        &app,
        "PUT",
        "/storage/k",
        Some(&lint_token),
        Some(json!({"value": "two"})),
    )
    .await;
    assert_eq!(put.0, StatusCode::OK);
    assert_eq!(
        get(&app, "/storage/k", &lint_token).await.1["data"]["value"],
        "two"
    );

    let big = "a".repeat(64 * 1024 + 1);
    let put = api(
        &app,
        "PUT",
        "/storage/big",
        Some(&lint_token),
        Some(json!({"value": big})),
    )
    .await;
    assert_eq!(put.0, StatusCode::PAYLOAD_TOO_LARGE, "{}", put.1);
    let put = api(
        &app,
        "PUT",
        "/storage/k2",
        Some(&lint_token),
        Some(json!({"value": 3})),
    )
    .await;
    assert_eq!(put.0, StatusCode::BAD_REQUEST, "{}", put.1);
    let put = api(
        &app,
        "PUT",
        "/storage/bad%20key",
        Some(&lint_token),
        Some(json!({"value": "v"})),
    )
    .await;
    assert_eq!(put.0, StatusCode::BAD_REQUEST, "{}", put.1);
    let (s, v) = get(&app, "/storage/..", &lint_token).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");

    let (s, v) = api(&app, "DELETE", "/storage/k", Some(&fmt_token), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    let (s, v) = api(&app, "DELETE", "/storage/k", Some(&lint_token), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, _) = get(&app, "/storage/k", &lint_token).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_task_token_uses_the_plugins_storage() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    seed(store).await;
    let id = plugin(store, "lint", &EVERYTHING).await;
    let wide = tokens::issue(store, &id, None).await.unwrap();
    let narrow = tokens::issue(store, &id, Some("one")).await.unwrap();
    let put = api(
        &app,
        "PUT",
        "/storage/k",
        Some(&narrow),
        Some(json!({"value": "v"})),
    )
    .await;
    assert_eq!(put.0, StatusCode::OK, "{}", put.1);
    assert_eq!(get(&app, "/storage/k", &wide).await.1["data"]["value"], "v");
}

#[tokio::test]
async fn the_ui_routes_stay_open_without_a_token() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    seed(state.ctx().store().unwrap()).await;
    let (s, v) = call(&app, "GET", "/api/tasks", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = call(&app, "GET", "/api/settings", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
}
