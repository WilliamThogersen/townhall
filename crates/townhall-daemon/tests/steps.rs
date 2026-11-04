use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use axum::http::StatusCode;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::sync::{Mutex, MutexGuard};
use townhall_core::steps::{project_dir_for, CLAUDE_PROJECTS_ENV};
use townhall_core::worktree;

mod common;
use common::{call, unique, Env};

/// The claude projects folder is read from the environment, so the tests in this file take
/// turns: each points it at its own temp folder, never the real `~/.claude`.
static CLAUDE: Mutex<()> = Mutex::const_new(());

struct Claude {
    _turn: MutexGuard<'static, ()>,
    projects: TempDir,
}

async fn claude() -> Claude {
    let turn = CLAUDE.lock().await;
    let projects = TempDir::new().unwrap();
    std::env::set_var(CLAUDE_PROJECTS_ENV, projects.path());
    Claude {
        _turn: turn,
        projects,
    }
}

impl Claude {
    /// Writes `body` as transcript `session` of the agent working in `worktree`.
    fn transcript(&self, worktree: &Path, session: &str, body: &str) -> PathBuf {
        let dir = project_dir_for(self.projects.path(), worktree);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{session}.jsonl"));
        std::fs::write(&path, body).unwrap();
        path
    }
}

/// Moves a transcript's last write `secs` back: the earlier agent stopped a while before its
/// name was reused, even on a file system that keeps whole seconds.
fn backdate(path: &Path, secs: u64) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(secs))
        .unwrap();
}

/// One `Bash` call and its result, as Claude writes them.
fn transcript(cwd: &Path, id: &str, command: &str) -> String {
    let cwd = cwd.display().to_string();
    let call = json!({
        "type": "assistant", "timestamp": "2026-11-04T10:00:00.000Z", "cwd": cwd,
        "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": id, "name": "Bash", "input": {"command": command}}
        ]}
    });
    let done = json!({
        "type": "user", "timestamp": "2026-11-04T10:00:02.000Z", "cwd": cwd,
        "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": id, "content": "ok"}
        ]}
    });
    format!("{call}\n{done}\n")
}

fn ids(v: &Value) -> Vec<&str> {
    v["data"]
        .as_array()
        .unwrap_or_else(|| panic!("no steps in {v}"))
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn steps_come_from_the_agents_transcript() {
    let claude = claude().await;
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let name = unique("steps");
    let wt = worktree::create(&env.workspace(&env.startup), &name, "main")
        .await
        .unwrap();
    let route = format!("/api/agents/{name}/steps");

    let (s, v) = call(&app, "GET", &route, None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &json!([])), "{v}");

    claude.transcript(
        &wt.path,
        "s1",
        &transcript(&wt.path, "toolu_1", "cargo test"),
    );
    let (s, v) = call(&app, "GET", &route, None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"],
        json!([{
            "id": "toolu_1", "tool": "Bash", "summary": "cargo test",
            "startedAt": "2026-11-04T10:00:00.000Z", "endedAt": "2026-11-04T10:00:02.000Z",
            "result": "ok"
        }])
    );
    let (s, v) = call(&app, "GET", &format!("{route}?repo=ALPHA"), None).await;
    assert_eq!((s, ids(&v)), (StatusCode::OK, vec!["toolu_1"]), "{v}");
}

#[tokio::test]
async fn an_unknown_agent_or_repo_is_404_and_a_bad_name_400() {
    let _claude = claude().await;
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/agents/nobody/steps", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["error"], json!("agent `nobody` not found"));
    let (s, v) = call(&app, "GET", "/api/agents/nobody/steps?repo=ghost", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["error"], json!("repo `ghost` not found"));
    let (s, v) = call(&app, "GET", "/api/agents/bad.name/steps", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
}

#[tokio::test]
async fn a_name_in_two_repos_needs_the_repo_query() {
    let claude = claude().await;
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let beta = env.repo("beta").await;
    call(&app, "POST", "/api/repos", Some(json!({"path": beta}))).await;
    let twin = unique("twin");
    let mut made = vec![];
    for repo in [&env.startup, &beta] {
        let wt = worktree::create(&env.workspace(repo), &twin, "main")
            .await
            .unwrap();
        made.push(wt);
    }
    claude.transcript(
        &made[1].path,
        "s",
        &transcript(&made[1].path, "toolu_b", "ls"),
    );

    let route = format!("/api/agents/{twin}/steps");
    let (s, v) = call(&app, "GET", &route, None).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert!(v["error"].as_str().unwrap().contains("pass ?repo=<name>"));
    let (s, v) = call(&app, "GET", &format!("{route}?repo=beta"), None).await;
    assert_eq!((s, ids(&v)), (StatusCode::OK, vec!["toolu_b"]), "{v}");
    let (s, v) = call(&app, "GET", &format!("{route}?repo=alpha"), None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &json!([])), "{v}");
}

#[tokio::test]
async fn a_reused_name_never_shows_the_earlier_agents_steps() {
    let claude = claude().await;
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let ws = env.workspace(&env.startup);
    let name = unique("reused");
    let route = format!("/api/agents/{name}/steps");

    let first = worktree::create(&ws, &name, "main").await.unwrap();
    let old = claude.transcript(
        &first.path,
        "old",
        &transcript(&first.path, "toolu_old", "a"),
    );
    let (_, v) = call(&app, "GET", &route, None).await;
    assert_eq!(ids(&v), vec!["toolu_old"], "{v}");

    worktree::remove(&ws, &name).await.unwrap();
    backdate(&old, 5);
    let second = worktree::create(&ws, &name, "main").await.unwrap();
    assert_eq!(second.path, first.path, "same name, same folder");
    let (s, v) = call(&app, "GET", &route, None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &json!([])), "{v}");

    claude.transcript(
        &second.path,
        "new",
        &transcript(&second.path, "toolu_new", "b"),
    );
    let (_, v) = call(&app, "GET", &route, None).await;
    assert_eq!(ids(&v), vec!["toolu_new"], "{v}");
}
