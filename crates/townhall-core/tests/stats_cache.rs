use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tempfile::TempDir;
use townhall_core::stats::{base_sha, CacheKey, ShippedCache, FRESH_FOR, REUSE_FOR};

const DAY: u64 = 86_400;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// `git args` in `dir` with author and committer date `when` (unix seconds, UTC).
fn git(dir: &Path, when: u64, args: &[&str]) -> String {
    let stamp = format!("@{when} +0000");
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_DATE", &stamp)
        .env("GIT_COMMITTER_DATE", &stamp)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {stderr}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), now() - 10 * DAY, &["init", "-q", "-b", "main"]);
    let init = ["commit", "-q", "--allow-empty", "-m", "init"];
    git(dir.path(), now() - 10 * DAY, &init);
    dir
}

/// One `fix:` commit on `townhall/<name>`, merged into main at `when`.
fn ship(dir: &Path, name: &str, when: u64) {
    let branch = format!("townhall/{name}");
    git(dir, when, &["checkout", "-q", "-b", &branch]);
    std::fs::write(dir.join(format!("{name}.txt")), "1\n").unwrap();
    git(dir, when, &["add", "."]);
    git(dir, when, &["commit", "-q", "-m", &format!("fix: {name}")]);
    git(dir, when, &["checkout", "-q", "main"]);
    git(dir, when, &["merge", "-q", "--no-ff", "--no-edit", &branch]);
}

fn key(dir: &Path) -> CacheKey {
    CacheKey {
        repo: "shop".into(),
        path: dir.to_path_buf(),
        base_branch: "main".into(),
        since_days: 730,
    }
}

fn agents(days: &[townhall_core::stats::ShippedDay]) -> u32 {
    days.iter().map(|d| d.agents).sum()
}

#[test]
fn the_defaults_are_a_minute_fresh_and_an_hour_of_reuse() {
    assert_eq!(FRESH_FOR, Duration::from_secs(60));
    assert_eq!(REUSE_FOR, Duration::from_secs(3_600));
}

#[tokio::test]
async fn base_sha_is_the_commit_the_branch_points_at() {
    let dir = repo();
    let head = git(dir.path(), now(), &["rev-parse", "HEAD"]);
    assert_eq!(base_sha(dir.path(), "main").await.unwrap(), head);
    let err = base_sha(dir.path(), "release").await.unwrap_err();
    assert!(err.to_string().contains("`release`"), "{err}");
    let err = base_sha(dir.path(), "--all").await.unwrap_err();
    assert!(err.to_string().contains("`--all`"), "{err}");
}

#[tokio::test]
async fn a_fresh_entry_is_served_without_running_git() {
    let dir = repo();
    ship(dir.path(), "a", now() - DAY);
    let cache = ShippedCache::default();
    assert_eq!(agents(&cache.shipped(&key(dir.path())).await.unwrap()), 1);
    ship(dir.path(), "b", now() - DAY);
    std::fs::remove_dir_all(dir.path().join(".git")).unwrap();
    let days = cache.shipped(&key(dir.path())).await.unwrap();
    assert_eq!(agents(&days), 1, "served from the cache: {days:?}");
}

#[tokio::test]
async fn a_stale_entry_is_counted_again_when_the_base_branch_moved() {
    let dir = repo();
    ship(dir.path(), "a", now() - 2 * DAY);
    let cache = ShippedCache::new(Duration::ZERO, REUSE_FOR);
    let first = cache.shipped(&key(dir.path())).await.unwrap();
    assert_eq!(agents(&first), 1);
    assert_eq!(cache.shipped(&key(dir.path())).await.unwrap(), first);
    ship(dir.path(), "b", now() - DAY);
    let days = cache.shipped(&key(dir.path())).await.unwrap();
    assert_eq!((days.len(), agents(&days)), (2, 2), "{days:?}");
}

#[tokio::test]
async fn a_repo_that_is_gone_is_an_error_once_its_entry_is_stale() {
    let dir = repo();
    ship(dir.path(), "a", now() - DAY);
    let cache = ShippedCache::new(Duration::ZERO, Duration::ZERO);
    assert_eq!(agents(&cache.shipped(&key(dir.path())).await.unwrap()), 1);
    let path = dir.path().to_path_buf();
    drop(dir);
    let err = cache.shipped(&key(&path)).await.unwrap_err();
    assert!(err.to_string().contains("not found"), "{err}");
}
