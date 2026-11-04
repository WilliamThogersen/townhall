use std::time::Duration;

use tempfile::TempDir;
use townhall_core::{tmux, worktree, Home, Workspace};

mod common;
use common::{fixture, git, init_repo};

#[tokio::test]
async fn worktree_create_list_remove() {
    let fx = fixture().await;
    let wt = worktree::create(&fx.ws, "alpha", "main").await.unwrap();
    assert!(wt.path.exists());
    assert_eq!(wt.path, fx.ws.worktrees_dir().join("alpha"));
    assert!(wt.path.starts_with(fx.home.path().join(".townhall")));
    assert_eq!(worktree::list(&fx.ws).await.unwrap(), vec![wt]);
    worktree::remove(&fx.ws, "alpha").await.unwrap();
    assert!(worktree::list(&fx.ws).await.unwrap().is_empty());
}

#[tokio::test]
async fn list_without_home_dir_is_empty() {
    let fx = fixture().await;
    assert!(!fx.ws.home().root().exists());
    assert!(worktree::list(&fx.ws).await.unwrap().is_empty());
}

#[tokio::test]
async fn rejects_bad_names() {
    let fx = fixture().await;
    assert!(worktree::create(&fx.ws, "../evil", "main").await.is_err());
}

#[tokio::test]
async fn repo_is_left_untouched() {
    let fx = fixture().await;
    worktree::create(&fx.ws, "clean", "main").await.unwrap();
    let status = git(fx.repo.path(), &["status", "--porcelain", "--ignored"]).await;
    assert!(status.is_empty(), "stray files in repo: {status}");
    let entries: Vec<_> = std::fs::read_dir(fx.repo.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec![".git".to_string()]);
    worktree::remove(&fx.ws, "clean").await.unwrap();
}

#[tokio::test]
async fn same_name_in_two_repos_does_not_collide() {
    let home = TempDir::new().unwrap();
    let (r1, r2) = (init_repo().await, init_repo().await);
    let h = Home::at(home.path().join(".townhall"));
    let (w1, w2) = (
        Workspace::new(h.clone(), r1.path()).unwrap(),
        Workspace::new(h, r2.path()).unwrap(),
    );
    assert_ne!(w1.key(), w2.key());
    let a = worktree::create(&w1, "same", "main").await.unwrap();
    let b = worktree::create(&w2, "same", "main").await.unwrap();
    assert_ne!(a.path, b.path);
    assert_eq!(worktree::list(&w1).await.unwrap(), vec![a]);
    assert_eq!(worktree::list(&w2).await.unwrap(), vec![b]);
    worktree::remove(&w1, "same").await.unwrap();
    worktree::remove(&w2, "same").await.unwrap();
}

#[tokio::test]
async fn tmux_session_runs_and_is_discoverable() {
    let dir = TempDir::new().unwrap();
    let key = format!("tkey-{}", std::process::id());
    let other = format!("okey-{}", std::process::id());
    let name = "sess";
    tmux::spawn(
        &key,
        name,
        dir.path(),
        "sh -c 'echo hello-townhall; sleep 30'",
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(tmux::list(&key).await.unwrap().contains(&name.to_string()));
    assert!(!tmux::list(&other)
        .await
        .unwrap()
        .contains(&name.to_string()));
    assert!(tmux::capture(&key, name)
        .await
        .unwrap()
        .contains("hello-townhall"));
    tmux::kill(&key, name).await.unwrap();
    assert!(!tmux::list(&key).await.unwrap().contains(&name.to_string()));
}
