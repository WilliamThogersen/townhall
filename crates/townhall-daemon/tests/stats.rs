use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use serde_json::{json, Value};

mod common;
use common::{call, Env};

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

/// Commits `subjects` on `townhall/<name>`, each adding a file of `lines` lines, merges the
/// branch into main at `when` and returns the merge's `YYYY-MM-DD`.
fn ship(dir: &Path, name: &str, subjects: &[&str], lines: usize, when: u64) -> String {
    let branch = format!("townhall/{name}");
    git(dir, when, &["checkout", "-q", "-b", &branch]);
    for (i, subject) in subjects.iter().enumerate() {
        let file = format!("{name}-{i}.txt");
        std::fs::write(dir.join(&file), "x\n".repeat(lines)).unwrap();
        git(dir, when, &["add", &file]);
        git(dir, when, &["commit", "-q", "-m", subject]);
    }
    git(dir, when, &["checkout", "-q", "main"]);
    git(dir, when, &["merge", "-q", "--no-ff", "--no-edit", &branch]);
    git(
        dir,
        when,
        &["log", "-1", "--format=%cd", "--date=format:%Y-%m-%d"],
    )
}

fn day(date: &str, fixes: u32, features: u32, lines: u64, agents: u32) -> Value {
    json!({ "date": date, "fixes": fixes, "features": features, "lines": lines, "agents": agents })
}

async fn add_repo(app: &axum::Router, path: &Path) {
    let (s, v) = call(app, "POST", "/api/repos", Some(json!({"path": path}))).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
}

#[tokio::test]
async fn stats_sum_every_repo_and_skip_the_broken_ones() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let beta = env.repo("beta").await;
    let gone = env.repo("gone").await;
    let renamed = env.repo("renamed").await;
    for repo in [&beta, &gone, &renamed] {
        add_repo(&app, repo).await;
    }
    let earlier = ship(&env.startup, "a", &["fix: a"], 1, now() - 2 * DAY);
    assert_eq!(ship(&beta, "b", &["feat: b"], 2, now() - 2 * DAY), earlier);
    let later = ship(&beta, "c", &["fix: c", "feat: c"], 1, now() - DAY);
    ship(&gone, "d", &["fix: d"], 1, now() - DAY);
    ship(&renamed, "e", &["fix: e"], 1, now() - DAY);
    std::fs::remove_dir_all(&gone).unwrap();
    git(&renamed, now(), &["branch", "-q", "-m", "main", "trunk"]);

    let (s, v) = call(&app, "GET", "/api/stats", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let both = json!({"days": [day(&earlier, 1, 1, 3, 2), day(&later, 1, 1, 2, 1)]});
    assert_eq!(v["data"], both);

    let (s, v) = call(&app, "GET", "/api/stats?repo=BETA", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let beta_only = json!({"days": [day(&earlier, 0, 1, 2, 1), day(&later, 1, 1, 2, 1)]});
    assert_eq!(v["data"], beta_only);

    let (s, v) = call(&app, "GET", "/api/stats?repo=", None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &both), "{v}");
    let (s, v) = call(&app, "GET", "/api/stats?repo=gone", None).await;
    assert_eq!(
        (s, &v["data"]),
        (StatusCode::OK, &json!({"days": []})),
        "{v}"
    );
}

#[tokio::test]
async fn an_unknown_repo_is_404() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/stats?repo=ghost", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["error"], json!("repo `ghost` not found"));
}

#[tokio::test]
async fn a_repo_without_townhall_merges_has_no_days() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/stats", None).await;
    assert_eq!(
        (s, &v["data"]),
        (StatusCode::OK, &json!({"days": []})),
        "{v}"
    );
}

#[tokio::test]
async fn a_second_call_within_the_minute_does_not_run_git() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let date = ship(&env.startup, "first", &["feat: first"], 1, now() - DAY);
    let (_, v) = call(&app, "GET", "/api/stats", None).await;
    let first = json!({"days": [day(&date, 0, 1, 1, 1)]});
    assert_eq!(v["data"], first);

    ship(&env.startup, "second", &["fix: second"], 4, now() - DAY);
    let (s, v) = call(&app, "GET", "/api/stats", None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &first), "{v}");
    let (_, v) = call(&app, "GET", "/api/stats?repo=alpha", None).await;
    assert_eq!(v["data"], first, "the same cache serves a named repo");
}
