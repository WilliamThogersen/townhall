mod common;
mod runner_support;
use common::{install_hook, Env};
use runner_support::{app_with, create, entries_of, idle, message, Fake};

fn sent(fake: &Fake, turn: usize) -> String {
    let line = &fake.stdin(turn)[0];
    let content = &line["message"]["content"];
    match content.as_str() {
        Some(text) => text.to_string(),
        None => content
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[tokio::test]
async fn a_before_turn_hook_adds_context_under_a_from_plugin_line() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let store = state.ctx().store().unwrap();
    let answer =
        r#"cat > seen.json; echo '{"additionalContext":"The user is on call this week."}'"#;
    let data = install_hook(store, &env.home, "message.before_turn", answer).await;
    let created = create(&app, "plan a logo change").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    let d = idle(&app, &id).await;
    assert!(entries_of(&d, "error").is_empty(), "{d}");
    let text = sent(&fake, 1);
    assert!(text.starts_with("plan a logo change"), "{text}");
    assert!(
        text.contains("From plugin gate@acme") && text.contains("The user is on call this week."),
        "{text}"
    );
    let seen: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("seen.json")).unwrap()).unwrap();
    assert_eq!(seen["event"], "message.before_turn");
    assert_eq!(seen["text"], "plan a logo change");
    assert_eq!(seen["kind"], "you");
    assert_eq!(seen["conversationId"], id.as_str());
    let entries = state.ctx().store().unwrap().entries(&id).await.unwrap();
    let thread = format!("{entries:?}");
    assert!(
        !thread.contains("on call"),
        "the thread keeps the user's own words: {thread}"
    );
}

#[tokio::test]
async fn a_before_turn_hook_can_never_block_a_message() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let store = state.ctx().store().unwrap();
    install_hook(
        store,
        &env.home,
        "message.before_turn",
        "echo no >&2; exit 2",
    )
    .await;
    let created = create(&app, "plan a logo change").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    assert_eq!(sent(&fake, 1), "plan a logo change");
    let (status, _) = message(&app, &id, "and another").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    idle(&app, &id).await;
    assert_eq!(sent(&fake, 2), "and another");
}

#[tokio::test]
async fn with_no_plugin_the_message_is_sent_unchanged() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let created = create(&app, "plan a logo change").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    assert_eq!(sent(&fake, 1), "plan a logo change");
}
