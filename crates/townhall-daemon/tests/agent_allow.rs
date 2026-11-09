use axum::http::StatusCode;
use serde_json::json;
use townhall_core::model::{AgentEvent, EntryBody, NewTask};

mod common;
use common::{call, Env};

fn new_task(name: &str, agent: &str) -> NewTask {
    NewTask {
        name: name.into(),
        title: format!("do {name}"),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: agent.into(),
        pr_base: None,
        done: Default::default(),
    }
}

#[tokio::test]
async fn an_approved_tool_joins_the_tasks_agent_type_and_the_thread_says_so() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let c = store.create_conversation().await.unwrap();
    store
        .create_task(&c.id, new_task("fix-login", "builder"))
        .await
        .unwrap();
    let uri = "/api/agents/fix-login/allow";
    let (s, v) = call(&app, "POST", uri, Some(json!({"tool": "Bash(gh pr *)"}))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["data"],
        json!({"agentType": "builder", "tool": "Bash(gh pr *)", "added": true})
    );
    let saved = store.agent_type("builder").await.unwrap().unwrap();
    assert!(saved.def.tools.contains(&"Bash(gh pr *)".to_string()));
    let entries = store.entries(&c.id).await.unwrap();
    let last = entries.last().unwrap();
    assert_eq!(
        last.body,
        EntryBody::Agent {
            agent: "fix-login".into(),
            event: AgentEvent::Allowed,
            text: "Bash(gh pr *) to builder".into()
        }
    );
    let before = entries.len();
    let (s, v) = call(&app, "POST", uri, Some(json!({"tool": " Bash(gh pr *) "}))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["added"], false);
    assert_eq!(store.entries(&c.id).await.unwrap().len(), before);
}

#[tokio::test]
async fn a_tool_that_fails_validation_or_an_unknown_task_saves_nothing() {
    let env = Env::new("shop").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let c = store.create_conversation().await.unwrap();
    store
        .create_task(&c.id, new_task("fix-login", "builder"))
        .await
        .unwrap();
    let before = store.agent_type("builder").await.unwrap().unwrap();
    for bad in ["", "--evil", "Bash git"] {
        let (s, v) = call(
            &app,
            "POST",
            "/api/agents/fix-login/allow",
            Some(json!({"tool": bad})),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad:?} {v}");
    }
    let (s, _) = call(
        &app,
        "POST",
        "/api/agents/ghost/allow",
        Some(json!({"tool": "Read"})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(store.agent_type("builder").await.unwrap().unwrap(), before);
}
