use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use townhall_core::store::{Marketplace, Plugin, Store};

mod common;
use common::{call, send, Env};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ui-plugin");
const WEB: &str = "http://localhost:5173";

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

async fn install_panel(store: &Store, enabled: bool, approved: &[&str]) {
    let root = store
        .home()
        .plugins_dir()
        .unwrap()
        .join("cache/acme/panel")
        .join(SHA);
    copy_tree(Path::new(FIXTURE), &root);
    let market = Marketplace {
        name: "acme".into(),
        source: json!({"source": "github", "repo": "acme/market"}),
        account: None,
        enabled: true,
        official: false,
        head_sha: None,
        fetched_at: None,
        error: None,
    };
    store.add_marketplace(market).await.unwrap();
    let plugin = Plugin {
        id: String::new(),
        marketplace: "acme".into(),
        name: "panel".into(),
        version: Some("1.0.0".into()),
        sha: SHA.into(),
        source: json!({"source": "path", "path": "./"}),
        enabled,
        pinned: false,
        permissions_hash: "sha256:aa".into(),
        approved: json!(approved),
        installed_at: String::new(),
        updated_at: String::new(),
        error: None,
    };
    store.add_plugin(plugin).await.unwrap();
}

async fn mint(app: &Router, id: &str, origin: Option<&str>) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/api/plugins/{id}/host-token"));
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    send(app, builder.body(Body::empty()).unwrap()).await
}

async fn info(app: &Router, token: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .uri("/api/plugin/v1/info")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    send(app, req).await
}

#[tokio::test]
async fn with_no_plugin_the_ui_list_is_empty() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/plugins/ui", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"], json!([]));
}

#[tokio::test]
async fn the_ui_list_names_the_entries_of_enabled_plugins_only() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), false, &[]).await;
    let (_, v) = call(&app, "GET", "/api/plugins/ui", None).await;
    assert_eq!(v["data"], json!([]));

    call(
        &app,
        "PATCH",
        "/api/plugins/panel@acme",
        Some(json!({"enabled": true})),
    )
    .await;
    let (s, v) = call(&app, "GET", "/api/plugins/ui", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"],
        json!([{
            "plugin": "panel@acme", "slot": "page", "title": "Panel",
            "entry": "ui/panel.html", "height": null
        }])
    );
}

#[tokio::test]
async fn the_web_gets_a_plugin_token_with_the_approved_scopes() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(
        state.ctx().store().unwrap(),
        true,
        &["api:tasks:read", "api:storage", "hook:task.ended"],
    )
    .await;
    let (s, v) = mint(&app, "panel@acme", Some(WEB)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["plugin"], "panel@acme");
    assert_eq!(v["data"]["version"], "1.0.0");
    assert_eq!(v["data"]["scopes"], json!(["tasks:read", "storage"]));
    let token = v["data"]["token"].as_str().unwrap();
    assert_eq!(token.len(), 64);
    let (s, v) = info(&app, token).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["plugin"], "panel@acme");
}

#[tokio::test]
async fn a_call_without_an_origin_or_from_a_foreign_one_gets_no_token() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), true, &["api:storage"]).await;
    let (s, v) = mint(&app, "panel@acme", None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    let (s, v) = mint(&app, "panel@acme", Some("https://evil.example")).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    assert!(v["data"].is_null());
}

#[tokio::test]
async fn a_disabled_or_unknown_plugin_gets_no_token() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), false, &["api:storage"]).await;
    let (s, v) = mint(&app, "panel@acme", Some(WEB)).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    let (s, v) = mint(&app, "ghost@acme", Some(WEB)).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
}
