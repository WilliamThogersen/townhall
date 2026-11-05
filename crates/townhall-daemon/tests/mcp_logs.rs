//! `read_logs` and `log_summary` over the MCP endpoint and `GET /api/logs`, on JSONL files
//! written into the temp home's logs folder.

use axum::http::StatusCode;
use serde_json::json;
use townhall_core::clock::now_rfc3339;

mod common;
mod mcp_support;
use common::call;
use mcp_support::Mcp;

fn line(
    ts: &str,
    level: &str,
    target: &str,
    event: &str,
    message: &str,
    task: Option<&str>,
) -> String {
    let mut v = json!({"timestamp": ts, "level": level, "target": target, "event": event, "message": message});
    if let Some(task) = task {
        v["task"] = json!(task);
    }
    v.to_string()
}

/// Entries stamped a minute ago, so the default 24h window holds them.
fn write_logs(m: &Mcp) {
    let ts = now_rfc3339().replace("Z", "000Z");
    let date = &ts[..10];
    let lines = [
        line(
            &ts,
            "ERROR",
            "townhall::task",
            "task.failed",
            "task foo-1 failed after 312ms",
            Some("foo-1"),
        ),
        line(
            &ts,
            "ERROR",
            "townhall::task",
            "task.failed",
            "task bar-9 failed after 18ms",
            Some("bar-9"),
        ),
        "garbage".to_string(),
        line(
            &ts,
            "WARN",
            "townhall::git",
            "cmd.run",
            "git push failed token=hunter2hunter2",
            None,
        ),
        line(
            &ts,
            "INFO",
            "townhall::http",
            "http.request",
            "GET /api/tasks",
            None,
        ),
    ];
    let dir = m.env.home.logs_dir().unwrap();
    std::fs::write(
        dir.join(format!("daemon.{date}.jsonl")),
        lines.join("\n") + "\n",
    )
    .unwrap();
}

#[tokio::test]
async fn log_summary_groups_failures_end_to_end() {
    let m = Mcp::new().await;
    write_logs(&m);

    let s = m.ok("log_summary", json!({})).await;
    assert_eq!(s["total"], 3, "{s}");
    assert_eq!(s["by_level"]["ERROR"], 2);
    let groups = s["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2, "{s}");
    assert_eq!(groups[0]["count"], 2);
    assert_eq!(groups[0]["event"], "task.failed");
    assert_eq!(groups[0]["fingerprint"].as_str().unwrap().len(), 16);
    let tasks: Vec<&str> = groups[0]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert_eq!(tasks.len(), 2);
    assert!(!s.to_string().contains("hunter2"));
}

#[tokio::test]
async fn read_logs_filters_and_refuses_bad_arguments() {
    let m = Mcp::new().await;
    write_logs(&m);

    let r = m.ok("read_logs", json!({"component": "git"})).await;
    assert_eq!(r["count"], 1, "{r}");
    assert!(r["lines"][0].as_str().unwrap().contains("cmd.run"));
    assert!(!r.to_string().contains("hunter2"), "{r}");
    assert_eq!(r["truncated"], false);

    let all = m
        .ok("read_logs", json!({"level": "info", "limit": 2}))
        .await;
    assert_eq!(all["count"], 2);

    let bad = m.refused("read_logs", json!({"since": "yesterday"})).await;
    assert!(bad.contains("since"), "{bad}");
    let bad = m.refused("read_logs", json!({"level": "loud"})).await;
    assert!(bad.contains("level"), "{bad}");
    m.refused("log_summary", json!({"nope": 1})).await;
}

#[tokio::test]
async fn empty_logs_folder_gives_empty_results() {
    let m = Mcp::new().await;
    let s = m.ok("log_summary", json!({})).await;
    assert_eq!(s["total"], 0);
    assert_eq!(s["groups"], json!([]));
    let r = m.ok("read_logs", json!({})).await;
    assert_eq!(r["lines"], json!([]));
}

#[tokio::test]
async fn http_mirror_reuses_the_same_functions() {
    let m = Mcp::new().await;
    write_logs(&m);

    let (status, body) = call(&m.app, "GET", "/api/logs?component=task&limit=1", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["count"], 1, "{body}");

    let (status, body) = call(&m.app, "GET", "/api/logs/summary?top=1", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["groups"].as_array().unwrap().len(), 1);

    let (status, body) = call(&m.app, "GET", "/api/logs?since=yesterday", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, _) = call(&m.app, "GET", "/api/logs?bogus=1", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
