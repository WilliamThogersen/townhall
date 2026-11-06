use std::time::Duration;

use axum::http::StatusCode;
use axum::Router;
use serde_json::{json, Value};
use townhall_core::model::{AgentEvent, Conversation, EntryBody};
use townhall_core::store::Store;

mod common;
use common::{call, Env};

const TEXT_ERROR: &str = "search needs 1 to 200 characters";
const LIMIT_ERROR: &str = "limit needs to be a whole number from 1 to 200";

fn you(text: &str) -> EntryBody {
    EntryBody::You {
        text: text.into(),
        attachments: vec![],
        reply_to: None,
    }
}

fn townhall(text: &str) -> EntryBody {
    EntryBody::Townhall { text: text.into() }
}

/// Lets the millisecond clock move, so `updatedAt` orders the writes.
async fn tick() {
    tokio::time::sleep(Duration::from_millis(3)).await;
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A conversation titled `title` with `entries` appended in order; returns its id.
async fn conversation(store: &Store, title: &str, entries: Vec<EntryBody>) -> String {
    let id = store.create_conversation().await.unwrap().id;
    let title = title.to_string();
    store
        .update_conversation(&id, move |c| Ok(Conversation { title, ..c }))
        .await
        .unwrap();
    for entry in entries {
        store.append_entry(&id, entry).await.unwrap();
    }
    tick().await;
    id
}

async fn search(app: &Router, query: &str) -> (StatusCode, Value) {
    call(app, "GET", &format!("/api/search?{query}"), None).await
}

async fn hits(app: &Router, text: &str) -> Vec<Value> {
    let (s, v) = search(app, &format!("q={}", encode(text))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["data"].as_array().unwrap().clone()
}

fn ids(hits: &[Value]) -> Vec<&str> {
    hits.iter()
        .map(|h| h["conversationId"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn a_title_match_wins_and_has_no_entry_or_snippet() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let id = conversation(
        store,
        "Login redirect loop",
        vec![you("the login page loops"), townhall("on it")],
    )
    .await;
    let updated = store.conversation(&id).await.unwrap().updated_at;

    let found = hits(&app, "LOGIN").await;
    assert_eq!(
        found,
        [json!({
            "conversationId": id, "title": "Login redirect loop", "match": "title",
            "entryId": null, "snippet": null, "updatedAt": updated
        })]
    );
}

#[tokio::test]
async fn a_message_match_names_the_newest_matching_entry() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let id = conversation(
        store,
        "shop work",
        vec![
            you("the checkout button is broken"),
            townhall("i will look at the\n\nCHECKOUT   flow"),
            you("thanks"),
        ],
    )
    .await;

    let found = hits(&app, "checkout").await;
    assert_eq!(found.len(), 1);
    let hit = &found[0];
    assert_eq!(hit["conversationId"], id.as_str());
    assert_eq!(hit["title"], "shop work");
    assert_eq!(hit["match"], "message");
    assert_eq!(hit["entryId"], 2);
    assert_eq!(hit["snippet"], "i will look at the CHECKOUT flow");
    let keys: Vec<&str> = hit
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys.len(), 6, "{hit}");
}

#[tokio::test]
async fn only_your_and_townhalls_messages_are_searched() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    conversation(
        store,
        "plain",
        vec![
            you("hello"),
            EntryBody::Error {
                message: "the zebra session exited".into(),
            },
            EntryBody::Agent {
                agent: "zebra-fix".into(),
                event: AgentEvent::Failed,
                text: "zebra tests failed".into(),
            },
            EntryBody::TasksCreated {
                tasks: vec!["zebra".into()],
            },
        ],
    )
    .await;
    assert!(hits(&app, "zebra").await.is_empty());
}

#[tokio::test]
async fn hits_come_newest_activity_first_and_stop_at_the_limit() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let oldest = conversation(store, "pricing", vec![you("update the pricing page")]).await;
    let middle = conversation(store, "other", vec![you("pricing copy")]).await;
    let newest = conversation(store, "Pricing tiers", vec![]).await;
    conversation(store, "unrelated", vec![you("dark theme")]).await;

    assert_eq!(
        ids(&hits(&app, "pricing").await),
        [&newest, &middle, &oldest]
    );
    store.append_entry(&oldest, townhall("done")).await.unwrap();
    assert_eq!(
        ids(&hits(&app, "pricing").await),
        [&oldest, &newest, &middle]
    );

    let (s, v) = search(&app, "q=pricing&limit=2").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let limited = v["data"].as_array().unwrap();
    assert_eq!(ids(limited), [&oldest, &newest]);
}

#[tokio::test]
async fn wildcards_are_plain_text_and_case_folds_beyond_ascii() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let percent = conversation(store, "a", vec![you("we hit 100% of the budget")]).await;
    conversation(store, "b", vec![you("1000 users, a-b testing")]).await;
    let danish = conversation(store, "c", vec![townhall("ÆBLEKAGE til Søren")]).await;

    assert_eq!(ids(&hits(&app, "100%").await), [&percent]);
    assert!(hits(&app, "a_b").await.is_empty());
    assert_eq!(ids(&hits(&app, "æblekage til SØREN").await), [&danish]);
}

#[tokio::test]
async fn a_long_message_gets_one_line_cut_around_the_match() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    let store = state.ctx().store().unwrap();
    let filler = "lorem ipsum dolor sit amet ".repeat(8);
    let text = format!("{filler}\nthe flaky checkout test\n{filler}");
    conversation(store, "long", vec![townhall(&text)]).await;

    let snippet = hits(&app, "flaky").await[0]["snippet"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        snippet.starts_with('…') && snippet.ends_with('…'),
        "{snippet}"
    );
    assert!(snippet.contains("the flaky checkout test"), "{snippet}");
    assert!(!snippet.contains('\n'), "{snippet}");
    assert!(snippet.chars().count() <= 82, "{snippet}");
}

#[tokio::test]
async fn no_match_is_an_empty_list() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    conversation(state.ctx().store().unwrap(), "x", vec![you("hello")]).await;
    let (s, v) = search(&app, "q=nothing").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v, json!({"success": true, "data": [], "error": null}));
}

#[tokio::test]
async fn the_text_must_be_1_to_200_characters() {
    let env = Env::new("alpha").await;
    let (state, app) = env.start();
    conversation(state.ctx().store().unwrap(), "x", vec![you("hello")]).await;
    let longest = "ø".repeat(200);
    for query in [
        String::new(),
        "q=".into(),
        "q=%20%0A%20".into(),
        format!("q={}", "a".repeat(201)),
    ] {
        let (s, v) = search(&app, &query).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{query}: {v}");
        assert_eq!(v["error"], TEXT_ERROR);
    }
    let (s, v) = search(&app, &format!("q={}", encode(&longest))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = search(&app, "q=%20hello%20").await;
    assert_eq!(
        (s, v["data"][0]["entryId"].clone()),
        (StatusCode::OK, json!(1))
    );
}

#[tokio::test]
async fn the_limit_must_be_1_to_200() {
    let env = Env::new("alpha").await;
    let (_state, app) = env.start();
    for limit in ["0", "201", "-3", "ten", "1.5"] {
        let (s, v) = search(&app, &format!("q=x&limit={limit}")).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{limit}: {v}");
        assert_eq!(v["error"], LIMIT_ERROR);
    }
    for limit in ["", "1", "200"] {
        let (s, v) = search(&app, &format!("q=x&limit={limit}")).await;
        assert_eq!(s, StatusCode::OK, "{limit}: {v}");
    }
}
