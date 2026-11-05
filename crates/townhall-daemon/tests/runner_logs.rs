//! The runner and MCP events (`turn.*`, `claude.*`, `mcp.tool`) seen through
//! `logging::test_capture`. Capture is thread-local, so these tests use the default
//! current-thread runtime and everything runs on the test's thread.

use serde_json::json;
use townhall_daemon::logging::test_capture;

mod common;
mod mcp_support;
mod runner_support;
use common::Env;
use mcp_support::Mcp;
use runner_support::{app_with, create, idle, Fake};

#[tokio::test]
async fn an_unknown_tool_logs_mcp_tool_with_ok_false() {
    let m = Mcp::new().await;
    let (capture, _guard) = test_capture();
    m.refused(
        "no_such_tool",
        json!({"secret": "sk-ant-api03-abcdefghijklmnop"}),
    )
    .await;

    let found = capture.events("mcp.tool");
    assert_eq!(found.len(), 1, "{:?}", capture.lines());
    let line = &found[0];
    assert_eq!(line["target"], "townhall::mcp");
    assert_eq!(line["tool"], "no_such_tool");
    assert_eq!(line["conversation"], m.id.as_str());
    assert_eq!(line["ok"], false);
    assert!(line["duration_ms"].is_u64(), "{line}");
    assert!(
        line["error"].as_str().unwrap().contains("unknown tool"),
        "{line}"
    );
    assert!(
        !line.to_string().contains("sk-ant-"),
        "arguments are not logged: {line}"
    );
}

#[tokio::test]
async fn a_good_tool_call_logs_ok_true_without_an_error() {
    let m = Mcp::new().await;
    let (capture, _guard) = test_capture();
    m.ok("list_agent_types", json!({})).await;

    let found = capture.events("mcp.tool");
    assert_eq!(found.len(), 1, "{:?}", capture.lines());
    assert_eq!(found[0]["ok"], true);
    assert!(found[0].get("error").is_none(), "{}", found[0]);
}

#[tokio::test]
async fn a_normal_turn_logs_start_spawn_exit_and_end() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let (capture, _guard) = test_capture();

    let created = create(&app, "plan a settings page").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;

    let start = capture.events("turn.start");
    assert_eq!(start.len(), 1, "{:?}", capture.lines());
    assert_eq!(start[0]["target"], "townhall::runner");
    assert_eq!(start[0]["conversation"], id.as_str());
    assert_eq!(
        start[0]["prompt_chars"],
        "plan a settings page".chars().count()
    );

    let end = capture.events("turn.end");
    assert_eq!(end.len(), 1, "{:?}", capture.lines());
    assert_eq!(end[0]["conversation"], id.as_str());
    assert_eq!(end[0]["outcome"], "ok");
    assert!(end[0]["duration_ms"].is_u64(), "{}", end[0]);
    assert_eq!(end[0]["cost_usd"], 0.0123);

    assert_eq!(capture.events("claude.spawn").len(), 1);
    let exit = capture.events("claude.exit");
    assert_eq!(exit.len(), 1, "{:?}", capture.lines());
    assert_eq!(exit[0]["code"], 0);
    assert!(capture.events("turn.failed").is_empty());

    let all = serde_json::to_string(&capture.lines()).unwrap();
    assert!(
        !all.contains("plan a settings page"),
        "the prompt text must not be logged"
    );
}

#[tokio::test]
async fn a_crashed_turn_logs_turn_failed_and_an_exit_code() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let (capture, _guard) = test_capture();

    let created = create(&app, "CRASH please").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;

    let failed = capture.events("turn.failed");
    assert_eq!(failed.len(), 1, "{:?}", capture.lines());
    assert_eq!(failed[0]["level"], "WARN");
    assert!(
        failed[0]["error"].as_str().unwrap().contains("boom"),
        "{}",
        failed[0]
    );
    assert_eq!(capture.events("turn.end")[0]["outcome"], "failed");
    assert_eq!(capture.events("claude.exit")[0]["code"], 3);
}

#[tokio::test]
async fn denied_tools_are_logged_by_name_only() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let (capture, _guard) = test_capture();

    let created = create(&app, "DENIAL please").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;

    let denied = capture.events("turn.permission_denials");
    assert_eq!(denied.len(), 1, "{:?}", capture.lines());
    assert_eq!(denied[0]["tools"], "Bash,Edit");
    assert!(!denied[0].to_string().contains("rm -rf"), "{}", denied[0]);
}
