use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tempfile::TempDir;
use townhall_core::agent::{self, LaunchOptions, ShellAdapter};
use townhall_core::{tmux, Error};

mod common;
use common::{fixture, git};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

fn shell(cmd: &str) -> LaunchOptions {
    LaunchOptions {
        prompt: None,
        command: Some(cmd.to_string()),
    }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}

#[tokio::test]
async fn start_list_send_output_stop() {
    let fx = fixture().await;
    let ws = &fx.ws;
    let name = unique("life");
    let a = agent::start(
        ws,
        &name,
        "main",
        &ShellAdapter,
        &shell("sh -c 'echo ready; cat'"),
    )
    .await
    .unwrap();
    assert!(a.alive && a.worktree.exists());
    assert_eq!(a.kind.as_deref(), Some("shell"));
    assert_eq!(a.branch, format!("townhall/{name}"));
    settle().await;

    let listed = agent::list(ws).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].alive);

    agent::send(ws, &name, "ping-townhall").await.unwrap();
    settle().await;
    let out = agent::output(ws, &name).await.unwrap();
    assert!(
        out.contains("ready") && out.contains("ping-townhall"),
        "{out}"
    );

    agent::stop(ws, &name).await.unwrap();
    assert!(agent::list(ws).await.unwrap().is_empty());
    assert!(!tmux::list(ws.key()).await.unwrap().contains(&name));
    assert!(git(fx.repo.path(), &["branch", "--list", "townhall/*"])
        .await
        .is_empty());
}

#[tokio::test]
async fn failed_spawn_rolls_back_worktree() {
    let fx = fixture().await;
    let ws = &fx.ws;
    let name = unique("roll");
    // Occupy the session name so tmux spawn fails after the worktree exists.
    let blocker = TempDir::new().unwrap();
    tmux::spawn(ws.key(), &name, blocker.path(), "sleep 30")
        .await
        .unwrap();

    let err = agent::start(ws, &name, "main", &ShellAdapter, &shell("sleep 30"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Command { .. }), "{err:?}");
    assert!(agent::list(ws).await.unwrap().is_empty());
    assert!(git(fx.repo.path(), &["branch", "--list", "townhall/*"])
        .await
        .is_empty());

    tmux::kill(ws.key(), &name).await.unwrap();
}

#[tokio::test]
async fn dead_session_shows_not_alive_and_stop_cleans_up() {
    let fx = fixture().await;
    let ws = &fx.ws;
    let name = unique("dead");
    agent::start(ws, &name, "main", &ShellAdapter, &shell("true"))
        .await
        .unwrap();
    settle().await;

    let listed = agent::list(ws).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].alive);

    agent::stop(ws, &name).await.unwrap();
    assert!(agent::list(ws).await.unwrap().is_empty());
}

#[tokio::test]
async fn stop_missing_agent_is_not_found() {
    let fx = fixture().await;
    let ws = &fx.ws;
    let err = agent::stop(ws, &unique("ghost")).await.unwrap_err();
    assert!(matches!(err, Error::NotFound(_)), "{err:?}");
}

#[tokio::test]
async fn send_and_output_on_missing_session_are_not_found() {
    let fx = fixture().await;
    let ws = &fx.ws;
    let name = unique("nosess");
    assert!(matches!(
        agent::send(ws, &name, "x").await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        agent::output(ws, &name).await,
        Err(Error::NotFound(_))
    ));
}

#[tokio::test]
async fn invalid_inputs_rejected_before_side_effects() {
    let fx = fixture().await;
    let ws = &fx.ws;
    let bad = agent::start(ws, "../x", "main", &ShellAdapter, &shell("true")).await;
    assert!(matches!(bad, Err(Error::InvalidName(_))));
    let no_cmd = agent::start(
        ws,
        &unique("nc"),
        "main",
        &ShellAdapter,
        &LaunchOptions::default(),
    )
    .await;
    assert!(matches!(no_cmd, Err(Error::InvalidInput(_))));
    assert!(agent::list(ws).await.unwrap().is_empty());
}

#[tokio::test]
async fn same_agent_name_in_two_repos_is_isolated() {
    let (a, b) = (fixture().await, fixture().await);
    let name = unique("twin");
    for fx in [&a, &b] {
        agent::start(&fx.ws, &name, "main", &ShellAdapter, &shell("sleep 30"))
            .await
            .unwrap();
    }
    for fx in [&a, &b] {
        let listed = agent::list(&fx.ws).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].alive);
    }
    agent::stop(&a.ws, &name).await.unwrap();
    assert!(agent::list(&a.ws).await.unwrap().is_empty());
    assert!(agent::list(&b.ws).await.unwrap()[0].alive);
    agent::stop(&b.ws, &name).await.unwrap();
}
