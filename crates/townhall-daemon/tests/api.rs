use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use townhall_daemon::{router, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst))
}

async fn init_repo() -> (TempDir, Router) {
    let dir = TempDir::new().unwrap();
    let git = |args: &'static [&'static str]| townhall_core::cmd::run("git", args, Some(dir.path()));
    git(&["init", "-b", "main"]).await.unwrap();
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "--allow-empty", "-m", "init"])
        .await
        .unwrap();
    let app = router(AppState::new(dir.path().to_path_buf()));
    (dir, app)
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(b) => builder
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => builder.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}

#[tokio::test]
async fn full_lifecycle_over_http() {
    let (_dir, app) = init_repo().await;
    let name = unique("api");

    let (s, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &json!([])));
    assert_eq!(v["success"], true);

    let body = json!({"name": name, "adapter": "shell", "command": "sh -c 'echo up; cat'"});
    let (s, v) = call(&app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["data"]["alive"], true);
    assert_eq!(v["data"]["kind"], "shell");
    settle().await;

    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(v["data"][0]["name"], name.as_str());

    let uri = format!("/api/agents/{name}/send");
    let (s, _) = call(&app, "POST", &uri, Some(json!({"text": "hello-api"}))).await;
    assert_eq!(s, StatusCode::OK);
    settle().await;

    let (s, v) = call(&app, "GET", &format!("/api/agents/{name}/output"), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(v["data"].as_str().unwrap().contains("hello-api"), "{v}");

    let (s, v) = call(&app, "DELETE", &format!("/api/agents/{name}"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(v["data"], json!([]));
}

#[tokio::test]
async fn invalid_name_is_400() {
    let (_dir, app) = init_repo().await;
    let body = json!({"name": "bad name!", "adapter": "shell", "command": "true"});
    let (s, v) = call(&app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["success"], false);
    assert!(v["error"].as_str().unwrap().contains("invalid name"));
}

#[tokio::test]
async fn unknown_adapter_and_missing_command_are_400() {
    let (_dir, app) = init_repo().await;
    let n = unique("bad");
    let (s, _) = call(&app, "POST", "/api/agents", Some(json!({"name": n, "adapter": "nope"}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(&app, "POST", "/api/agents", Some(json!({"name": n, "adapter": "shell"}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn missing_agent_is_404() {
    let (_dir, app) = init_repo().await;
    let n = unique("ghost");
    let (s, v) = call(&app, "DELETE", &format!("/api/agents/{n}"), None).await;
    assert_eq!((s, &v["success"]), (StatusCode::NOT_FOUND, &json!(false)));
    let (s, _) = call(&app, "GET", &format!("/api/agents/{n}/output"), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = call(&app, "POST", &format!("/api/agents/{n}/send"), Some(json!({"text": "x"}))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bad_base_is_500_and_rolls_back() {
    let (_dir, app) = init_repo().await;
    let n = unique("base");
    let body = json!({"name": n, "adapter": "shell", "command": "true", "base": "no-such-ref"});
    let (s, _) = call(&app, "POST", "/api/agents", Some(body)).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(v["data"], json!([]));
}
