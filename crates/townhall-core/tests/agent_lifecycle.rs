use std::time::Duration;

use tempfile::TempDir;
use townhall_core::{tmux, worktree};

async fn git(dir: &std::path::Path, args: &[&str]) {
    townhall_core::cmd::run("git", args, Some(dir))
        .await
        .unwrap();
}

async fn init_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-b", "main"]).await;
    git(
        dir.path(),
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--allow-empty",
            "-m",
            "init",
        ],
    )
    .await;
    dir
}

#[tokio::test]
async fn worktree_create_list_remove() {
    let repo = init_repo().await;
    let wt = worktree::create(repo.path(), "alpha", "main")
        .await
        .unwrap();
    assert!(wt.path.exists());
    assert_eq!(worktree::list(repo.path()).await.unwrap(), vec![wt]);
    worktree::remove(repo.path(), "alpha").await.unwrap();
    assert!(worktree::list(repo.path()).await.unwrap().is_empty());
}

#[tokio::test]
async fn rejects_bad_names() {
    let repo = init_repo().await;
    assert!(worktree::create(repo.path(), "../evil", "main")
        .await
        .is_err());
}

#[tokio::test]
async fn tmux_session_runs_and_is_discoverable() {
    let dir = TempDir::new().unwrap();
    let name = format!("test-{}", std::process::id());
    tmux::spawn(&name, dir.path(), "sh -c 'echo hello-townhall; sleep 30'")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(tmux::list().await.unwrap().contains(&name));
    assert!(tmux::capture(&name)
        .await
        .unwrap()
        .contains("hello-townhall"));
    tmux::kill(&name).await.unwrap();
    assert!(!tmux::list().await.unwrap().contains(&name));
}
