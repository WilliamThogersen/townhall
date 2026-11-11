//! The run routes: one run with its steps in the definition's order, and a conversation's runs.

use axum::http::StatusCode;
use serde_json::json;
use townhall_core::store::{NewRun, StepUpdate};

mod common;
mod mcp_support;
mod runner_support;
use common::call;
use mcp_support::Mcp;

const SNAPSHOT: &str = r#"
name = "standard"
work = "build"

[[step]]
id = "plan"
agent = "planner"

[[step]]
id = "build"
agent = "builder"
after = ["plan"]

[[step]]
id = "review"
agent = "reviewer"
after = ["build"]
when = { paths = ["**/auth/**"] }

[[step]]
id = "land"
agent = "lander"
after = ["review"]
"#;

async fn seed(m: &Mcp, id: &str) {
    let new = NewRun {
        id: id.into(),
        workflow: "standard".into(),
        snapshot: SNAPSHOT.into(),
        conversation_id: m.id.clone(),
        repo: "shop".into(),
        base_branch: "main".into(),
        input: "fix login".into(),
    };
    m.store().create_run(new).await.unwrap();
}

async fn step(m: &Mcp, run: &str, id: &str, round: u32, update: StepUpdate<'_>) {
    assert!(m.store().claim_run_step(run, id, round).await.unwrap());
    m.store()
        .set_run_step(run, id, round, update)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_run_lists_every_step_of_its_definition_with_the_newest_round_of_each() {
    let m = Mcp::new().await;
    seed(&m, "fix-login").await;
    step(&m, "fix-login", "plan", 1, StepUpdate::state("done")).await;
    let building = StepUpdate {
        task: Some("fix-login-build"),
        ..StepUpdate::state("started")
    };
    step(&m, "fix-login", "build", 1, building).await;
    let skipped = StepUpdate {
        note: Some("no changed file matches"),
        ..StepUpdate::state("skipped")
    };
    step(&m, "fix-login", "review", 1, skipped).await;
    step(&m, "fix-login", "build", 2, StepUpdate::state("started")).await;

    let (s, v) = call(&m.app, "GET", "/api/runs/fix-login", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let run = &v["data"];
    assert_eq!(run["id"], "fix-login");
    assert_eq!(run["workflow"], "standard");
    assert_eq!(run["conversationId"], m.id);
    assert_eq!(run["state"], "running");
    let steps = run["steps"].as_array().unwrap();
    let shown: Vec<_> = steps
        .iter()
        .map(|s| {
            (
                s["id"].as_str().unwrap(),
                s["state"].as_str().unwrap(),
                s["round"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("plan", "done", 1),
            ("build", "started", 2),
            ("review", "skipped", 1),
            ("land", "pending", 0),
        ]
    );
    assert_eq!(steps[0]["agent"], "planner");
    assert_eq!(steps[2]["note"], "no changed file matches");
    assert_eq!(steps[3]["task"], json!(null));
}

#[tokio::test]
async fn an_unknown_run_is_404() {
    let m = Mcp::new().await;
    let (s, v) = call(&m.app, "GET", "/api/runs/ghost", None).await;
    assert_eq!(
        (s, v["success"].clone()),
        (StatusCode::NOT_FOUND, json!(false))
    );
}

#[tokio::test]
async fn the_runs_of_a_conversation_come_newest_first() {
    let m = Mcp::new().await;
    seed(&m, "first").await;
    seed(&m, "second").await;
    let (s, v) = call(
        &m.app,
        "GET",
        &format!("/api/runs?conversation={}", m.id),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let ids: Vec<_> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["second", "first"]);
    assert_eq!(v["data"][0]["steps"].as_array().unwrap().len(), 4);
    let (_, none) = call(&m.app, "GET", "/api/runs?conversation=other", None).await;
    assert_eq!(none["data"], json!([]));
}

#[tokio::test]
async fn listing_runs_needs_a_conversation() {
    let m = Mcp::new().await;
    let (s, v) = call(&m.app, "GET", "/api/runs", None).await;
    assert_eq!(
        (s, v["success"].clone()),
        (StatusCode::BAD_REQUEST, json!(false))
    );
}
