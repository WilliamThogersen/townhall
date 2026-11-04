use std::time::{Duration, Instant};

use axum::http::StatusCode;
use serde_json::Value;

mod common;
mod runner_support;
use common::{call, Env};
use runner_support::{
    alive, app_with, create, entries_of, gone, idle, kinds, message, turns_started, values_of, Fake,
};

const LOST: &str = "claude no longer has this conversation's history, so townhall started a fresh session: earlier context is gone";

async fn stop(app: &axum::Router, id: &str) -> Value {
    let started = Instant::now();
    let (s, v) = call(app, "POST", &format!("/api/conversations/{id}/stop"), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "{:?}",
        started.elapsed()
    );
    v["data"].clone()
}

fn id_of(created: &Value) -> String {
    created["conversation"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn stop_ends_a_turn_whose_children_hold_its_output() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = id_of(&create(&app, "KIDS").await);
    turns_started(&fake, 1).await;
    assert_eq!(stop(&app, &id).await["status"], "yourTurn");
    assert!(gone(&fake.pid("pid", 1)).await, "claude is gone");
    assert!(gone(&fake.pid("kid", 1)).await, "and so is its child");
    let (s, v) = message(&app, &id, "again").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    idle(&app, &id).await;
}

#[tokio::test]
async fn stop_gives_up_on_a_child_that_left_the_group() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = id_of(&create(&app, "ESCAPE").await);
    turns_started(&fake, 1).await;
    let kid = fake.pid("kid", 1);
    assert_eq!(stop(&app, &id).await["status"], "yourTurn");
    assert!(gone(&fake.pid("pid", 1)).await);
    assert!(alive(&kid), "it left the group and outlived the kill");
    std::process::Command::new("kill")
        .args(["-9", &kid])
        .status()
        .unwrap();
    let d = idle(&app, &id).await;
    assert!(
        entries_of(&d, "error").is_empty(),
        "a stop is no error: {d}"
    );
}

#[tokio::test]
async fn a_lost_session_starts_fresh_once() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let created = create(&app, "hello").await;
    let (id, old) = (
        id_of(&created),
        created["conversation"]["sessionId"].clone(),
    );
    idle(&app, &id).await;
    message(&app, &id, "STALE now").await;
    let d = idle(&app, &id).await;
    assert_eq!(fake.turns(), 3);
    assert_eq!(
        values_of(&fake.argv(2), "--resume"),
        [old.as_str().unwrap()]
    );
    let fresh = d["conversation"]["sessionId"].as_str().unwrap();
    assert_ne!(fresh, old.as_str().unwrap());
    assert_eq!(values_of(&fake.argv(3), "--session-id"), [fresh]);
    assert_eq!(d["conversation"]["sessionStarted"], true);
    let prompt = values_of(&fake.argv(3), "--append-system-prompt").join(" ");
    assert!(
        prompt.contains("history of this conversation was lost"),
        "{prompt}"
    );
    assert_eq!(
        kinds(&d),
        ["you", "repo", "townhall", "you", "error", "townhall"]
    );
    assert_eq!(entries_of(&d, "error")[0]["message"], LOST);
    assert_eq!(
        fake.stdin(3)[0]["message"]["content"][0]["text"],
        "STALE now"
    );
}

#[tokio::test]
async fn a_lost_session_is_retried_only_once() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let id = id_of(&create(&app, "hello").await);
    idle(&app, &id).await;
    message(&app, &id, "ALWAYS").await;
    let d = idle(&app, &id).await;
    assert_eq!(fake.turns(), 3, "one resume, one fresh try, no more");
    let errors: Vec<Value> = entries_of(&d, "error")
        .into_iter()
        .map(|e| e["message"].clone())
        .collect();
    assert_eq!(
        errors,
        [
            Value::from(LOST),
            Value::from("claude failed: No conversation found with session ID: any")
        ]
    );
    assert_eq!(d["conversation"]["sessionStarted"], false);
}
