use axum::http::StatusCode;
use serde_json::json;
use townhall_core::model::{NewPermissionDecision, PermissionVerdict};

mod common;
use common::{call, Env};

fn decision(pattern: &str, verdict: PermissionVerdict) -> NewPermissionDecision {
    NewPermissionDecision {
        task: "fix-login".into(),
        agent_type: "builder".into(),
        command: "Bash: git status".into(),
        pattern: pattern.into(),
        verdict,
        reason: "read-only".into(),
    }
}

#[tokio::test]
async fn decisions_and_suggestions_are_listed() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let (s, v) = call(&app, "GET", "/api/permissions", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"], json!({"decisions": [], "suggestions": []}));
    let store = state.ctx().store().unwrap();
    for _ in 0..3 {
        let allowed = decision("Bash(git status *)", PermissionVerdict::Allow);
        store.record_permission_decision(allowed).await.unwrap();
    }
    let denied = decision("Bash(rm *)", PermissionVerdict::HardDeny);
    store.record_permission_decision(denied).await.unwrap();
    let (s, v) = call(&app, "GET", "/api/permissions", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let decisions = v["data"]["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 4);
    assert_eq!(decisions[0]["verdict"], "hard-deny");
    assert_eq!(decisions[0]["agentType"], "builder");
    assert_eq!(decisions[0]["command"], "Bash: git status");
    assert_eq!(
        v["data"]["suggestions"],
        json!([{"agentType": "builder", "tool": "Bash(git status *)", "approvals": 3}])
    );
}

#[tokio::test]
async fn accepting_a_suggestion_saves_the_tool_on_the_type_and_clears_it() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    for _ in 0..3 {
        let allowed = decision("Bash(git status *)", PermissionVerdict::Allow);
        store.record_permission_decision(allowed).await.unwrap();
    }
    let uri = "/api/permissions/suggestions/accept";
    let body = json!({"agentType": "builder", "tool": "Bash(git status *)"});
    let (s, v) = call(&app, "POST", uri, Some(body.clone())).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"],
        json!({"agentType": "builder", "tool": "Bash(git status *)", "added": true})
    );
    let saved = store.agent_type("builder").await.unwrap().unwrap();
    assert!(saved.def.tools.contains(&"Bash(git status *)".to_string()));
    let (_, v) = call(&app, "GET", "/api/permissions", None).await;
    assert_eq!(v["data"]["suggestions"], json!([]));
    let (s, v) = call(&app, "POST", uri, Some(body)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["added"], false);
}

#[tokio::test]
async fn accepting_a_bad_tool_or_an_unknown_type_saves_nothing() {
    let env = Env::new("shop").await;
    let (_state, app) = env.start();
    let uri = "/api/permissions/suggestions/accept";
    let bad = json!({"agentType": "builder", "tool": "--evil"});
    let (s, v) = call(&app, "POST", uri, Some(bad)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    let unknown = json!({"agentType": "nobody", "tool": "Bash(ls *)"});
    let (s, v) = call(&app, "POST", uri, Some(unknown)).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
}

#[tokio::test]
async fn a_suggestion_goes_away_once_the_type_lists_the_tool() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    for _ in 0..3 {
        let allowed = decision("Bash(git status *)", PermissionVerdict::Allow);
        store.record_permission_decision(allowed).await.unwrap();
    }
    let source = "name = \"builder\"\nadapter = \"claude\"\ntools = [\"Bash(git status *)\"]\n";
    store.save_agent_type(source).await.unwrap();
    let (_, v) = call(&app, "GET", "/api/permissions", None).await;
    assert_eq!(v["data"]["suggestions"], json!([]));
}
