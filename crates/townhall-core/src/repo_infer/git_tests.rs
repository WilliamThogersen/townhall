use std::process::Stdio;
use std::time::Duration;

use tempfile::TempDir;
use tokio::process::Command;
use tokio::time::Instant;

use super::{first_line, grep_first, is_work_tree, ls_files, Grep};
use crate::cmd::run;
use crate::Error;

fn soon() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

async fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    run("git", &["init", "-b", "main"], Some(dir.path()))
        .await
        .unwrap();
    std::fs::write(dir.path().join("a.txt"), "needle here\n").unwrap();
    run("git", &["add", "a.txt"], Some(dir.path()))
        .await
        .unwrap();
    dir
}

#[tokio::test]
async fn work_tree_check() {
    let git = repo().await;
    let plain = TempDir::new().unwrap();
    assert!(is_work_tree(git.path(), soon()).await.unwrap());
    assert!(!is_work_tree(plain.path(), soon()).await.unwrap());
    assert!(!is_work_tree(&plain.path().join("gone"), soon())
        .await
        .unwrap());
    assert!(!is_work_tree(git.path(), Instant::now()).await.unwrap());
}

#[tokio::test]
async fn ls_files_lists_tracked_paths_or_runs_out_of_time() {
    let git = repo().await;
    let files = ls_files(git.path(), soon()).await.unwrap();
    assert_eq!(files, Some(vec!["a.txt".to_string()]));
    assert_eq!(ls_files(git.path(), Instant::now()).await.unwrap(), None);
    let plain = TempDir::new().unwrap();
    let err = ls_files(plain.path(), soon()).await.unwrap_err();
    assert!(matches!(err, Error::Command { .. }), "{err:?}");
}

#[tokio::test]
async fn grep_hit_miss_out_of_time_and_failure() {
    let git = repo().await;
    let found = grep_first(git.path(), "needle", soon()).await.unwrap();
    assert_eq!(found, Grep::Hit("a.txt".to_string()));
    let missed = grep_first(git.path(), "-absent-", soon()).await.unwrap();
    assert_eq!(missed, Grep::Miss);
    let late = grep_first(git.path(), "needle", Instant::now())
        .await
        .unwrap();
    assert_eq!(late, Grep::TimedOut);
    let plain = TempDir::new().unwrap();
    let err = grep_first(plain.path(), "needle", soon())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Command { .. }), "{err:?}");
}

#[tokio::test]
async fn first_line_without_a_newline() {
    let mut child = Command::new("sh")
        .args(["-c", "printf abc"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    assert_eq!(
        first_line(&mut stdout).await.unwrap().as_deref(),
        Some("abc")
    );
    child.wait().await.unwrap();
}
