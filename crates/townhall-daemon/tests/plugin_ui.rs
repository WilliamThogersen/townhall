use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use townhall_core::store::{Marketplace, Plugin, Store};

mod common;
use common::Env;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ui-plugin");
const CSP: &str = "sandbox allow-scripts; default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'none'; frame-ancestors 'self'";

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

async fn install_panel(store: &Store, enabled: bool) -> std::path::PathBuf {
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
        approved: json!([]),
        installed_at: String::new(),
        updated_at: String::new(),
        error: None,
    };
    store.add_plugin(plugin).await.unwrap();
    root
}

async fn get(app: &Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn header<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> &'a str {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

#[tokio::test]
async fn the_entry_page_is_served_with_the_strict_headers() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), true).await;
    let (s, h, body) = get(&app, "/api/plugins/panel@acme/ui/ui/panel.html").await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert!(body.contains("<script src=\"panel.js\">"));
    assert_eq!(header(&h, "content-type"), "text/html; charset=utf-8");
    assert_eq!(header(&h, "content-security-policy"), CSP);
    assert_eq!(header(&h, "x-content-type-options"), "nosniff");
    assert_eq!(header(&h, "access-control-allow-origin"), "*");
    assert_eq!(header(&h, "cache-control"), "no-store");
}

#[tokio::test]
async fn the_page_cannot_reach_the_api_or_storage_by_its_policy() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), true).await;
    let (_, h, _) = get(&app, "/api/plugins/panel@acme/ui/ui/panel.html").await;
    let csp = header(&h, "content-security-policy");
    let directives: Vec<&str> = csp.split("; ").collect();
    assert!(directives.contains(&"connect-src 'none'"));
    assert!(directives.contains(&"default-src 'none'"));
    assert!(directives.contains(&"sandbox allow-scripts"));
    assert!(!csp.contains("allow-same-origin"));
}

#[tokio::test]
async fn assets_beside_the_entry_are_served_with_their_types() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), true).await;
    let (s, h, _) = get(&app, "/api/plugins/panel@acme/ui/ui/panel.js").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(header(&h, "content-type"), "text/javascript; charset=utf-8");
    assert_eq!(header(&h, "content-security-policy"), CSP);
    let (s, h, _) = get(&app, "/api/plugins/panel@acme/ui/ui/panel.css").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(header(&h, "content-type"), "text/css; charset=utf-8");
}

#[tokio::test]
async fn files_outside_the_ui_folder_are_not_served() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), true).await;
    for path in [
        "hook.sh",
        ".townhall-plugin/plugin.json",
        "ui/missing.js",
        "ui",
        "",
    ] {
        let (s, _, body) = get(&app, &format!("/api/plugins/panel@acme/ui/{path}")).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{path}: {body}");
    }
}

#[tokio::test]
async fn traversal_is_refused() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), true).await;
    for path in [
        "ui/../hook.sh",
        "ui/%2e%2e/hook.sh",
        "ui/..%2fhook.sh",
        "ui%2f..%2fhook.sh",
        "ui/./panel.js",
        "ui//panel.js",
        "ui\\panel.js",
    ] {
        let (s, _, body) = get(&app, &format!("/api/plugins/panel@acme/ui/{path}")).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{path}: {body}");
        assert!(!body.contains("secret"), "{path}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_inside_the_ui_folder_is_refused() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let root = install_panel(state.ctx().store().unwrap(), true).await;
    std::os::unix::fs::symlink(root.join("hook.sh"), root.join("ui/link.js")).unwrap();
    let (s, _, body) = get(&app, "/api/plugins/panel@acme/ui/ui/link.js").await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{body}");
    assert!(!body.contains("secret"));
}

#[tokio::test]
async fn a_disabled_or_unknown_plugin_serves_nothing() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    install_panel(state.ctx().store().unwrap(), false).await;
    let (s, _, _) = get(&app, "/api/plugins/panel@acme/ui/ui/panel.html").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _, _) = get(&app, "/api/plugins/ghost@acme/ui/ui/panel.html").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_unknown_file_type_is_not_served() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let root = install_panel(state.ctx().store().unwrap(), true).await;
    std::fs::write(root.join("ui/data.bin"), "x").unwrap();
    let (s, _, _) = get(&app, "/api/plugins/panel@acme/ui/ui/data.bin").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
