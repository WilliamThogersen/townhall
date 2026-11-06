use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Once;

use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::model::{Checks, NewTask, Task, TaskState};

mod common;
mod mcp_support;
use common::{call, git};
use mcp_support::Mcp;

const CODER: &str = "name = \"coder\"\nadapter = \"claude\"\n";

fn stubs() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        let dir = std::env::temp_dir().join(format!("townhall-open-pr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gh = dir.join("gh");
        let script = "#!/bin/sh\nif [ \"$2\" = status ]; then exit 0; fi\necho https://github.com/acme/shop/pull/7\n";
        std::fs::write(&gh, script).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("TOWNHALL_GH_BIN", &gh);
        std::env::set_var("TOWNHALL_GLAB_BIN", dir.join("missing-glab"));
    });
}

async fn running(m: &Mcp, name: &str) {
    stubs();
    m.store().save_agent_type(CODER).await.unwrap();
    let new = NewTask {
        name: name.into(),
        title: "fix".into(),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "coder".into(),
    };
    m.store().create_task(&m.id, new).await.unwrap();
    let change = |t: Task| {
        Ok(Task {
            state: TaskState::Working,
            ..t
        })
    };
    m.store().update_task(name, change).await.unwrap();
}

async fn with_worktree(m: &Mcp, name: &str) -> PathBuf {
    running(m, name).await;
    let ws = m.env.workspace(&m.env.startup);
    townhall_core::worktree::create(&ws, name, "main")
        .await
        .unwrap()
        .path
}

async fn open_pr(m: &Mcp, task: &str, arguments: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                      "params": {"name": "open_pr", "arguments": arguments}});
    let (status, answer) = call(
        &m.app,
        "POST",
        &format!("/api/tasks/{task}/mcp"),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer["result"].clone()
}

fn text(result: &Value) -> String {
    result["content"][0]["text"].as_str().unwrap().to_string()
}

fn payload(result: &Value) -> Value {
    assert!(result.get("isError").is_none(), "{result}");
    serde_json::from_str(&text(result)).unwrap()
}

async fn task(m: &Mcp, name: &str) -> Task {
    m.store().find_task(name).await.unwrap().unwrap()
}

#[tokio::test]
async fn a_repo_without_origin_answers_no_remote() {
    let m = Mcp::new().await;
    with_worktree(&m, "fix-login").await;
    let got = payload(&open_pr(&m, "fix-login", json!({"title": "Fix login"})).await);
    assert_eq!(got["state"], "noRemote");
    assert!(
        got["message"]
            .as_str()
            .unwrap()
            .contains("townhall/fix-login"),
        "{got}"
    );
    assert_eq!(task(&m, "fix-login").await.pr, None);
}

#[tokio::test]
async fn a_github_origin_opens_the_pr_and_stores_it() {
    let m = Mcp::new().await;
    with_worktree(&m, "fix-login").await;
    let bare = m.env.root.path().join("bare.git");
    let bare = bare.to_string_lossy().into_owned();
    git(m.env.root.path(), &["init", "-q", "--bare", &bare]).await;
    let start = &m.env.startup;
    git(
        start,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/shop.git",
        ],
    )
    .await;
    git(start, &["config", "remote.origin.pushurl", &bare]).await;
    let got = payload(
        &open_pr(
            &m,
            "fix-login",
            json!({"title": "Fix login", "body": "why"}),
        )
        .await,
    );
    assert_eq!(got["state"], "opened");
    assert_eq!(got["number"], 7);
    assert_eq!(got["url"], "https://github.com/acme/shop/pull/7");
    let pr = task(&m, "fix-login").await.pr.unwrap();
    assert_eq!(pr.number, 7);
    assert_eq!(
        pr.url.as_deref(),
        Some("https://github.com/acme/shop/pull/7")
    );
    assert_eq!(pr.checks, Checks::Pending);
    let pushed = git(
        start,
        &["ls-remote", &bare, "refs/heads/townhall/fix-login"],
    )
    .await;
    assert!(!pushed.is_empty());
}

#[tokio::test]
async fn a_missing_cli_is_an_actionable_error() {
    let m = Mcp::new().await;
    with_worktree(&m, "fix-login").await;
    let origin = "https://gitlab.com/acme/shop.git";
    git(&m.env.startup, &["remote", "add", "origin", origin]).await;
    let result = open_pr(&m, "fix-login", json!({"title": "Fix login"})).await;
    assert_eq!(result["isError"], true, "{result}");
    assert!(text(&result).contains("is not installed"), "{result}");
}

#[tokio::test]
async fn a_task_without_a_worktree_is_refused() {
    let m = Mcp::new().await;
    running(&m, "fix-login").await;
    let result = open_pr(&m, "fix-login", json!({"title": "Fix login"})).await;
    assert_eq!(result["isError"], true, "{result}");
    assert!(text(&result).contains("has no worktree"), "{result}");
}

#[tokio::test]
async fn a_blank_title_is_refused() {
    let m = Mcp::new().await;
    with_worktree(&m, "fix-login").await;
    let result = open_pr(&m, "fix-login", json!({"title": "  "})).await;
    assert_eq!(result["isError"], true, "{result}");
}
