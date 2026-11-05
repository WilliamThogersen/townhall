use axum::http::StatusCode;
use serde_json::json;

mod common;
mod runner_support;
use common::{call, Env};
use runner_support::{app_with, create, entries_of, idle, Fake};

#[tokio::test]
async fn a_reply_is_stored_with_its_entry_and_the_session_gets_the_quote() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let created = create(&app, "plan a login fix").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    let d = idle(&app, &id).await;
    let said = entries_of(&d, "townhall")[0]["id"].as_u64().unwrap();

    let uri = format!("/api/conversations/{id}/messages");
    let body = json!({"text": "make it faster", "replyTo": said});
    let (s, v) = call(&app, "POST", &uri, Some(body)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["replyTo"], said);
    let d = idle(&app, &id).await;
    assert!(entries_of(&d, "error").is_empty(), "{d}");

    let yours = entries_of(&d, "you");
    assert_eq!(yours[1]["text"], "make it faster");
    assert_eq!(yours[1]["replyTo"], said);
    assert!(yours[0].get("replyTo").is_none(), "{}", yours[0]);

    let line = &fake.stdin(2)[0];
    assert_eq!(
        line["message"]["content"],
        json!([{"type": "text", "text": "> replying to townhall: \"on it.\"\n\nmake it faster"}])
    );
}

#[tokio::test]
async fn a_reply_to_nothing_or_to_an_entry_without_words_is_refused() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let created = create(&app, "plan a login fix").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    let d = idle(&app, &id).await;
    let turns = fake.turns();
    let uri = format!("/api/conversations/{id}/messages");

    let (s, v) = call(
        &app,
        "POST",
        &uri,
        Some(json!({"text": "x", "replyTo": 999})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert!(v.to_string().contains("no entry 999"), "{v}");

    let repo = entries_of(&d, "repo");
    assert_eq!(repo.len(), 1, "{d}");
    let body = json!({"text": "x", "replyTo": repo[0]["id"]});
    let (s, v) = call(&app, "POST", &uri, Some(body)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert!(v.to_string().contains("cannot be replied to"), "{v}");
    assert_eq!(fake.turns(), turns);
    assert_eq!(entries_of(&idle(&app, &id).await, "you").len(), 1);
}

#[tokio::test]
async fn a_new_conversation_has_nothing_to_reply_to() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let body = json!({"text": "plan a login fix", "replyTo": 1});
    let (s, v) = call(&app, "POST", "/api/conversations", Some(body)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
}
