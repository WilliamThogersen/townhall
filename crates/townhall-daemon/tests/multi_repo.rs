use std::path::Path;

use axum::http::StatusCode;
use axum::Router;
use serde_json::{json, Value};
use townhall_core::agent::{self, LaunchOptions, ShellAdapter};

mod common;
use common::{call, settle, unique, Env};

fn names_and_repos(v: &Value) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["name"].as_str().unwrap().into(),
                a["repo"].as_str().unwrap().into(),
            )
        })
        .collect();
    rows.sort();
    rows
}

/// Starts `a` in the startup repo and `b` in beta; `b` echoes what it reads.
async fn start_one_per_repo(env: &Env, beta: &Path, a: &str, b: &str) {
    common::start_shell(&env.workspace(&env.startup), a, "sleep 30").await;
    common::start_shell(&env.workspace(beta), b, "sh -c 'echo up; cat'").await;
}

/// Both are listed with their repo, and `b` is reached by its name alone.
async fn listed_and_reached_by_name(app: &Router, a: &str, b: &str) {
    let (s, v) = call(app, "GET", "/api/agents", None).await;
    assert_eq!(s, StatusCode::OK);
    let mut want = vec![
        (a.to_string(), "alpha".to_string()),
        (b.to_string(), "beta".to_string()),
    ];
    want.sort();
    assert_eq!(names_and_repos(&v), want);
    let states: Vec<&Value> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| &a["state"])
        .collect();
    assert!(
        states.iter().all(|s| **s == json!({"kind": "working"})),
        "{v}"
    );

    let send = format!("/api/agents/{b}/send");
    let (s, _) = call(app, "POST", &send, Some(json!({"text": "hello-beta"}))).await;
    assert_eq!(s, StatusCode::OK);
    settle().await;
    let (s, v) = call(app, "GET", &format!("/api/agents/{b}/output"), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(v["data"].as_str().unwrap().contains("hello-beta"), "{v}");
}

/// An unknown `?repo=` and a repo with a live agent.
async fn refused_across_repos(app: &Router, b: &str) {
    let ghost = format!("/api/agents/{b}/output?repo=ghost");
    let (s, _) = call(app, "GET", &ghost, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, v) = call(app, "DELETE", "/api/repos/beta", None).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert!(v["error"].as_str().unwrap().contains("1 running agent"));
}

#[tokio::test]
async fn agents_are_listed_and_resolved_across_repos() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let beta = env.repo("beta").await;
    call(&app, "POST", "/api/repos", Some(json!({"path": beta}))).await;
    let (a, b) = (unique("a"), unique("b"));

    start_one_per_repo(&env, &beta, &a, &b).await;
    settle().await;
    listed_and_reached_by_name(&app, &a, &b).await;
    refused_across_repos(&app, &b).await;

    for name in [&a, &b] {
        let (s, v) = call(&app, "DELETE", &format!("/api/agents/{name}"), None).await;
        assert_eq!(s, StatusCode::OK, "{v}");
    }
    let (s, v) = call(&app, "DELETE", "/api/repos/beta", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn a_name_in_two_repos_needs_the_repo_query() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let beta = env.repo("beta").await;
    call(&app, "POST", "/api/repos", Some(json!({"path": beta}))).await;
    let twin = unique("twin");
    let opts = LaunchOptions {
        command: Some("sleep 30".into()),
        ..LaunchOptions::default()
    };
    for repo in [&env.startup, &beta] {
        agent::start(&env.workspace(repo), &twin, "main", &ShellAdapter, &opts)
            .await
            .unwrap();
    }
    let output = format!("/api/agents/{twin}/output");
    let (s, v) = call(&app, "GET", &output, None).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert!(v["error"].as_str().unwrap().contains("pass ?repo=<name>"));
    let (s, _) = call(&app, "GET", &format!("{output}?repo=beta"), None).await;
    assert_eq!(s, StatusCode::OK);

    let (s, _) = call(
        &app,
        "DELETE",
        &format!("/api/agents/{twin}?repo=alpha"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&app, "GET", &output, None).await;
    assert_eq!(s, StatusCode::OK, "only beta has it now");
    let (s, _) = call(&app, "DELETE", &format!("/api/agents/{twin}"), None).await;
    assert_eq!(s, StatusCode::OK);
    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(v["data"], json!([]));
}

#[tokio::test]
async fn a_repo_whose_folder_is_gone_does_not_break_the_list() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let gone = env.repo("gone").await;
    call(&app, "POST", "/api/repos", Some(json!({"path": gone}))).await;
    std::fs::remove_dir_all(&gone).unwrap();
    let (s, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!((s, &v["data"]), (StatusCode::OK, &json!([])), "{v}");
    let (s, v) = call(&app, "DELETE", "/api/repos/gone", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn a_name_in_another_case_is_never_acted_on() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    let lower = unique("c");
    let upper = lower.to_uppercase();
    common::start_shell(&env.workspace(&env.startup), &lower, "sleep 30").await;

    let output = format!("/api/agents/{upper}/output");
    let (s, v) = call(&app, "GET", &output, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    let (s, v) = call(&app, "DELETE", &format!("/api/agents/{upper}"), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    let (_, v) = call(&app, "GET", "/api/agents", None).await;
    assert_eq!(
        v["data"][0]["name"],
        json!(lower),
        "the other agent is untouched"
    );
    assert_eq!(v["data"][0]["alive"], true);

    let (s, _) = call(&app, "DELETE", &format!("/api/agents/{lower}"), None).await;
    assert_eq!(s, StatusCode::OK);
}
