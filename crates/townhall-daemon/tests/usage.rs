//! `GET /api/usage` with an injected source: no Keychain, no network. Capture is thread-local,
//! so these run on the current-thread runtime `#[tokio::test]` gives.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::clock::unix_millis;
use townhall_core::usage::{BoxFuture, HttpReply, UsageService, UsageSource};
use townhall_daemon::logging::test_capture;
use townhall_daemon::router;

mod common;
use common::{call, Env};

const SECRET: &str = "sk-ant-oat01-do-not-leak";

struct Canned {
    token: Option<&'static str>,
    answer: Result<HttpReply, String>,
    calls: Arc<AtomicUsize>,
}

impl UsageSource for Canned {
    fn token(&self) -> BoxFuture<'_, Option<String>> {
        Box::pin(async { self.token.map(str::to_string) })
    }

    fn get<'a>(&'a self, _token: &'a str) -> BoxFuture<'a, Result<HttpReply, String>> {
        Box::pin(async {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.answer.clone()
        })
    }
}

fn now() -> i64 {
    unix_millis(SystemTime::now())
}

async fn app_with(
    token: Option<&'static str>,
    answer: Result<HttpReply, String>,
) -> (axum::Router, Arc<AtomicUsize>, Env) {
    let env = Env::new("startup").await;
    let calls = Arc::new(AtomicUsize::new(0));
    let source = Canned {
        token,
        answer,
        calls: calls.clone(),
    };
    let clock = Arc::new(|| unix_millis(SystemTime::now()));
    let service = UsageService::new(source, clock, None);
    let state = env.state().with_usage(Arc::new(service));
    (router(state), calls, env)
}

fn reply(body: Value) -> Result<HttpReply, String> {
    Ok(HttpReply {
        status: 200,
        body: body.to_string(),
    })
}

#[tokio::test]
async fn usage_is_served_as_camel_case_json() {
    let resets = now() + 150 * 60_000;
    let week = now() + 5 * 24 * 3_600_000;
    let body = json!({
        "five_hour": { "utilization": 30.0, "resets_at": resets },
        "seven_day": { "utilization": 54.0, "resets_at": week },
    });
    let (app, calls, _env) = app_with(Some(SECRET), reply(body)).await;

    let (status, v) = call(&app, "GET", "/api/usage", None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let data = &v["data"];
    assert_eq!(data["session"]["usedPercent"], json!(30.0));
    assert_eq!(data["session"]["resetsAt"], json!(resets));
    assert!(data["session"]["ratePerHour"].is_number(), "{data}");
    assert_eq!(data["weekly"]["usedPercent"], json!(54.0));
    assert!(data["updatedAt"].is_number());
    assert_eq!(data["error"], Value::Null);
    assert!(!v.to_string().contains("do-not-leak"));

    let (_, again) = call(&app, "GET", "/api/usage", None).await;
    assert_eq!(again["data"], v["data"]);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the next read reuses the result"
    );
}

#[tokio::test]
async fn without_a_token_it_says_to_sign_in() {
    let (app, calls, _env) = app_with(None, Err("unused".into())).await;
    let (status, v) = call(&app, "GET", "/api/usage", None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["success"], json!(true));
    assert_eq!(
        v["data"]["error"],
        json!("Sign in to Claude Code to see usage")
    );
    assert_eq!(
        (&v["data"]["session"], &v["data"]["weekly"]),
        (&Value::Null, &Value::Null)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_failing_fetch_is_logged_without_the_token() {
    let leaky = Err(format!("tls failed, sent Authorization: Bearer {SECRET}"));
    let (app, _calls, _env) = app_with(Some(SECRET), leaky).await;
    let (capture, _guard) = test_capture();

    let (status, v) = call(&app, "GET", "/api/usage", None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["error"], json!("Usage unavailable (offline?)"));
    assert!(!v.to_string().contains("do-not-leak"));

    let failed = capture.events("usage.fetch_failed");
    assert_eq!(failed.len(), 1, "{:?}", capture.lines());
    assert_eq!(failed[0]["target"], "townhall::usage");
    assert_eq!(failed[0]["level"], "WARN");
    let everything = Value::Array(capture.lines()).to_string();
    assert!(!everything.contains("do-not-leak"), "{everything}");
}
