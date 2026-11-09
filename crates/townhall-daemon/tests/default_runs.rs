use axum::http::StatusCode;
use serde_json::Value;

mod common;
mod runner_support;
use common::{call, Env};
use runner_support::{app_with, create, idle, message, Fake};

async fn data(app: &axum::Router, uri: &str) -> Value {
    let (s, v) = call(app, "GET", uri, None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["data"].clone()
}

#[tokio::test]
async fn every_conversation_turn_is_a_run_of_the_default_type() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);

    let created = create(&app, "plan a settings page").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;

    let usage = data(&app, "/api/library/agents/usage").await;
    let default = usage
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["agent"] == "default")
        .unwrap_or_else(|| panic!("no default row in {usage}"));
    assert_eq!(
        (default["runs"].as_u64(), default["tokens"].as_u64()),
        (Some(1), Some(5040))
    );

    let (s, _) = message(&app, &id, "and a CRASH").await;
    assert_eq!(s, StatusCode::OK);
    idle(&app, &id).await;

    let runs = data(&app, "/api/library/agents/DEFAULT/runs").await;
    let runs = runs.as_array().unwrap();
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert!(runs
        .iter()
        .all(|r| r["kind"] == "turn" && r["conversationId"] == id.as_str()));
    let mut states: Vec<&str> = runs.iter().map(|r| r["state"].as_str().unwrap()).collect();
    states.sort();
    assert_eq!(states, ["done", "failed"]);

    let series = data(&app, "/api/library/agents/default/usage?range=24h").await;
    assert_eq!(series["runs"], 2);
    assert_eq!(series["tokens"], 5040);
    assert_eq!(series["byConversation"][0]["key"], id.as_str());
}
