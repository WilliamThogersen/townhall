use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use townhall_core::agent::{self, LaunchOptions, ShellAdapter};
use townhall_core::{tmux, Error};
use tempfile::TempDir;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst))
}

async fn git(dir: &std::path::Path, args: &[&str]) -> String {
    townhall_core::cmd::run("git", args, Some(dir)).await.unwrap()
}

async fn init_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-b", "main"]).await;
    git(dir.path(), &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "--allow-empty", "-m", "init"]).await;
    dir
}

fn shell(cmd: &str) -> LaunchOptions {
    LaunchOptions { prompt: None, command: Some(cmd.to_string()) }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}

#[tokio::test]
async fn start_list_send_output_stop() {
    let repo = init_repo().await;
    let name = unique("life");
    let a = agent::start(repo.path(), &name, "main", &ShellAdapter, &shell("sh -c 'echo ready; cat'"))
        .await
        .unwrap();
    assert!(a.alive && a.worktree.exists());
    assert_eq!(a.kind.as_deref(), Some("shell"));
    assert_eq!(a.branch, format!("townhall/{name}"));
    settle().await;

    let listed = agent::list(repo.path()).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].alive);

    agent::send(&name, "ping-townhall").await.unwrap();
    settle().await;
    let out = agent::output(&name).await.unwrap();
    assert!(out.contains("ready") && out.contains("ping-townhall"), "{out}");

    agent::stop(repo.path(), &name).await.unwrap();
    assert!(agent::list(repo.path()).await.unwrap().is_empty());
    assert!(!tmux::list().await.unwrap().contains(&name));
    assert!(git(repo.path(), &["branch", "--list", "townhall/*"]).await.is_empty());
}

#[tokio::test]
async fn failed_spawn_rolls_back_worktree() {
    let repo = init_repo().await;
    let name = unique("roll");
    // Occupy the session name so tmux spawn fails after the worktree exists.
    let blocker = TempDir::new().unwrap();
    tmux::spawn(&name, blocker.path(), "sleep 30").await.unwrap();

    let err = agent::start(repo.path(), &name, "main", &ShellAdapter, &shell("sleep 30"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Command { .. }), "{err:?}");
    assert!(agent::list(repo.path()).await.unwrap().is_empty());
    assert!(git(repo.path(), &["branch", "--list", "townhall/*"]).await.is_empty());

    tmux::kill(&name).await.unwrap();
}

#[tokio::test]
async fn dead_session_shows_not_alive_and_stop_cleans_up() {
    let repo = init_repo().await;
    let name = unique("dead");
    agent::start(repo.path(), &name, "main", &ShellAdapter, &shell("true")).await.unwrap();
    settle().await;

    let listed = agent::list(repo.path()).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].alive);

    agent::stop(repo.path(), &name).await.unwrap();
    assert!(agent::list(repo.path()).await.unwrap().is_empty());
}

#[tokio::test]
async fn stop_missing_agent_is_not_found() {
    let repo = init_repo().await;
    let err = agent::stop(repo.path(), &unique("ghost")).await.unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[tokio::test]
async fn send_and_output_on_missing_session_are_not_found() {
    let name = unique("nosess");
    assert!(matches!(agent::send(&name, "x").await, Err(Error::NotFound(_))));
    assert!(matches!(agent::output(&name).await, Err(Error::NotFound(_))));
}

#[tokio::test]
async fn invalid_inputs_rejected_before_side_effects() {
    let repo = init_repo().await;
    let bad = agent::start(repo.path(), "../x", "main", &ShellAdapter, &shell("true")).await;
    assert!(matches!(bad, Err(Error::InvalidName(_))));
    let no_cmd = agent::start(repo.path(), &unique("nc"), "main", &ShellAdapter, &LaunchOptions::default()).await;
    assert!(matches!(no_cmd, Err(Error::InvalidInput(_))));
    assert!(agent::list(repo.path()).await.unwrap().is_empty());
}
