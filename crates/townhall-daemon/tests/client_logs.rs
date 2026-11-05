//! `POST /api/logs/client`: frontend error reports re-emitted as `townhall::web` events.
//! Capture is thread-local, so these run on the current-thread runtime `#[tokio::test]` gives.

use axum::http::StatusCode;
use serde_json::json;
use townhall_daemon::logging::test_capture;

mod common;
use common::{call, Env};

const URL: &str = "/api/logs/client";

#[tokio::test]
async fn valid_body_gives_202_and_a_web_event() {
    let env = Env::new("startup").await;
    let (_, app) = env.start();
    let (capture, _guard) = test_capture();

    let body = json!({
        "level": "error", "event": "web.error", "message": "boom",
        "stack": "at foo (a.js:1:1)", "route": "/tasks/x", "request_id": "r-1",
        "error_kind": "http", "status": 500
    });
    let (status, _) = call(&app, "POST", URL, Some(body)).await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let found = capture.events("web.error");
    assert_eq!(found.len(), 1, "{:?}", capture.lines());
    let line = &found[0];
    assert_eq!(line["target"], "townhall::web");
    assert_eq!(line["level"], "ERROR");
    assert_eq!(line["message"], "boom");
    assert_eq!(line["request_id"], "r-1");
    assert_eq!(line["error_kind"], "http");
    assert_eq!(line["status"], 500);
    assert_eq!(line["route"], "/tasks/x");
}

#[tokio::test]
async fn a_batch_emits_one_event_each() {
    let env = Env::new("startup").await;
    let (_, app) = env.start();
    let (capture, _guard) = test_capture();

    let body = json!([
        {"level": "warn", "event": "web.error", "message": "a"},
        {"level": "info", "event": "api.http", "message": "b", "status": 502},
    ]);
    let (status, _) = call(&app, "POST", URL, Some(body)).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(capture.events("web.error").len(), 1);
    assert_eq!(capture.events("api.http").len(), 1);
}

#[tokio::test]
async fn invalid_level_or_event_is_a_400() {
    let env = Env::new("startup").await;
    let (_, app) = env.start();
    let (capture, _guard) = test_capture();

    for body in [
        json!({"level": "loud", "event": "web.error", "message": "x"}),
        json!({"level": "error", "event": "panic", "message": "x"}),
        json!({"level": "error", "event": "web.error"}),
    ] {
        let (status, _) = call(&app, "POST", URL, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    assert!(capture.lines().is_empty(), "{:?}", capture.lines());
}

#[tokio::test]
async fn nine_kb_body_is_a_413() {
    let env = Env::new("startup").await;
    let (_, app) = env.start();
    let (capture, _guard) = test_capture();

    let body = json!({"level": "error", "event": "web.error", "message": "x".repeat(9 * 1024)});
    let (status, _) = call(&app, "POST", URL, Some(body)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(capture.events("web.error").is_empty());
}

#[tokio::test]
async fn the_31st_request_in_a_minute_is_dropped_and_reported_once() {
    let env = Env::new("startup").await;
    let (_, app) = env.start();
    let (capture, _guard) = test_capture();

    for i in 0..33 {
        let body = json!({"level": "error", "event": "web.error", "message": format!("m{i}")});
        let (status, _) = call(&app, "POST", URL, Some(body)).await;
        assert_eq!(status, StatusCode::ACCEPTED, "request {i}");
    }
    assert_eq!(capture.events("web.error").len(), 30);
    assert_eq!(capture.events("web.dropped").len(), 1);
}

#[tokio::test]
async fn secrets_in_every_field_are_masked() {
    let env = Env::new("startup").await;
    let (_, app) = env.start();
    let (capture, _guard) = test_capture();

    let token = "sk-ant-api03-abcdefghijklmnop";
    let body = json!({
        "level": "error", "event": "web.error",
        "message": format!("failed with {token}"),
        "stack": format!("at x?token=hunter2 {token}"),
    });
    let (status, _) = call(&app, "POST", URL, Some(body)).await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let found = capture.events("web.error");
    assert_eq!(found.len(), 1);
    let raw = found[0].to_string();
    assert!(!raw.contains("sk-ant-api03"), "{raw}");
    assert!(!raw.contains("hunter2"), "{raw}");
}
