use axum::http::StatusCode;
use serde_json::json;
use townhall_core::agent::{self, LaunchOptions, ShellAdapter};
use townhall_core::events::EventBus;
use townhall_core::store::Store;
use townhall_core::ContextOptions;
use townhall_daemon::AppState;

mod common;
use common::{call, unique, Env};

#[tokio::test]
async fn a_phase_one_config_is_imported_without_losing_anything() {
    let env = Env::new("current").await;
    let legacy = env.repo("legacy").await;
    env.write_config(&json!({"repo": legacy, "windowSize": [800, 600]}));
    let name = unique("old");
    let opts = LaunchOptions {
        command: Some("sleep 30".into()),
        ..LaunchOptions::default()
    };
    agent::start(&env.workspace(&legacy), &name, "main", &ShellAdapter, &opts)
        .await
        .unwrap();

    let (state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/repos", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let repos: Vec<&str> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(repos, ["legacy", "current"]);

    assert!(
        !env.home.config_file().exists(),
        "townhall reads no config file"
    );
    let kept = std::fs::read_to_string(env.home.root().join("config.json.imported")).unwrap();
    assert!(kept.contains("windowSize"), "the old file is kept, renamed");
    let store = state.ctx().store().unwrap();
    assert_eq!(store.desktop_repo().await.unwrap(), Some(legacy.clone()));

    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(v["data"][0]["name"], json!(name));
    assert_eq!(v["data"][0]["repo"], "legacy");
    let (s, _) = call(&app, "DELETE", &format!("/api/agents/{name}"), None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn a_saved_repo_that_is_gone_is_skipped() {
    let env = Env::new("current").await;
    env.write_config(&json!({"repo": "/definitely/not/here"}));
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/repos", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"].as_array().unwrap().len(), 1);
    assert_eq!(v["data"][0]["name"], "current");
}

#[tokio::test]
async fn a_corrupt_config_is_left_alone() {
    let env = Env::new("current").await;
    env.home.ensure().unwrap();
    std::fs::write(env.home.config_file(), "{nope").unwrap();
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/repos", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        std::fs::read_to_string(env.home.config_file()).unwrap(),
        "{nope"
    );
}

#[tokio::test]
async fn a_held_database_is_a_clear_error() {
    let env = Env::new("current").await;
    let holder = Store::open(&env.home, EventBus::new()).unwrap();
    let ws = env.workspace(&env.startup);
    let err = AppState::open(ws.clone(), ContextOptions::default()).unwrap_err();
    assert!(
        err.to_string().contains("in use by another townhall"),
        "{err}"
    );
    let app = townhall_daemon::router(AppState::new(ws));
    let (s, v) = call(&app, "GET", "/api/repos", None).await;
    assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE, "{v}");
    assert!(v["error"]
        .as_str()
        .unwrap()
        .contains("in use by another townhall"));
    let (s, _) = call(&app, "GET", "/api/info", None).await;
    assert_eq!(s, StatusCode::OK, "info needs no store");
    drop(holder);
}
