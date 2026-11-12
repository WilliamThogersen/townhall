use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use futures_util::StreamExt;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use townhall_core::model::{Conversation, ConversationStatus, ServerEvent};
use townhall_daemon::{router, start_background, AppState};

mod common;
mod runner_support;
use common::{send, Env};
use runner_support::{app_with, create, entries_of, idle, Fake};

async fn serve(app: axum::Router) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn events_request(addr: SocketAddr, origin: Option<&'static str>) -> Request<()> {
    let mut req = format!("ws://{addr}/api/events")
        .into_client_request()
        .unwrap();
    if let Some(o) = origin {
        req.headers_mut()
            .insert("origin", HeaderValue::from_static(o));
    }
    req
}

#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_origin_is_refused_before_the_upgrade() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let addr = serve(app.clone()).await;
    let refused =
        tokio_tungstenite::connect_async(events_request(addr, Some("https://evil.example")))
            .await
            .unwrap_err();
    match refused {
        WsError::Http(res) => assert_eq!(res.status(), StatusCode::FORBIDDEN),
        other => panic!("expected a 403, got {other}"),
    }
    let req = Request::get("/api/events")
        .header("origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    let (s, v) = send(&app, req).await;
    assert_eq!(
        (s, v["success"].as_bool()),
        (StatusCode::FORBIDDEN, Some(false))
    );
    let (ok, _) = tokio_tungstenite::connect_async(events_request(addr, Some("tauri://localhost")))
        .await
        .unwrap();
    drop(ok);
}

#[tokio::test]
async fn a_plain_get_is_told_to_upgrade_in_contract_copy() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let req = Request::get("/api/events").body(Body::empty()).unwrap();
    let (s, v) = send(&app, req).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"], "this route needs a websocket upgrade");
}

async fn next_event(
    ws: &mut (impl StreamExt<Item = Result<Message, WsError>> + Unpin),
) -> ServerEvent {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("an event in time")
            .expect("the socket is open")
            .unwrap();
        if let Message::Text(text) = frame {
            return serde_json::from_str(text.as_str()).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_arrive_in_order_over_the_socket() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let addr = serve(app.clone()).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(events_request(addr, None))
        .await
        .unwrap();
    let id = create(&app, "hello").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (mut ids, mut kinds, mut statuses) = (vec![], vec![], vec![]);
    while kinds.last().map(String::as_str) != Some("townhall")
        || statuses.last() != Some(&ConversationStatus::YourTurn)
    {
        match next_event(&mut ws).await {
            ServerEvent::EntryAppended {
                conversation_id,
                entry,
            } => {
                assert_eq!(conversation_id, id);
                ids.push(entry.id);
                let v = serde_json::to_value(&entry).unwrap();
                kinds.push(v["kind"].as_str().unwrap().to_string());
            }
            ServerEvent::ConversationUpdated { conversation } => statuses.push(conversation.status),
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(ids, [1, 2, 3]);
    assert_eq!(kinds, ["you", "repo", "townhall"]);
    let thinking = statuses
        .iter()
        .position(|s| *s == ConversationStatus::Thinking)
        .expect("a thinking update");
    assert!(statuses[..thinking]
        .iter()
        .all(|s| *s == ConversationStatus::YourTurn));
    ws.close(None).await.unwrap();
    let rest = tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .unwrap();
    assert!(
        matches!(rest, None | Some(Ok(Message::Close(_)))),
        "{rest:?}"
    );
}

#[tokio::test]
async fn a_conversation_left_thinking_is_fixed_at_start() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let store = state.ctx().store().unwrap();
    let stuck = store.create_conversation().await.unwrap();
    let thinking = |c: Conversation| {
        Ok(Conversation {
            status: ConversationStatus::Thinking,
            ..c
        })
    };
    store
        .update_conversation(&stuck.id, thinking)
        .await
        .unwrap();
    let background = start_background(&state).await;
    let d = idle(&app, &stuck.id).await;
    let errors = entries_of(&d, "error");
    assert_eq!(errors.len(), 1, "{d}");
    let message = errors[0]["message"].as_str().unwrap();
    assert!(message.starts_with("townhall restarted"), "{message}");
    assert_eq!(fake.turns(), 0);
    background.stop();
}

#[tokio::test]
async fn stopping_the_background_ends_running_turns() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (state, app) = app_with(&env, &fake);
    let background = start_background(&state).await;
    let id = create(&app, "SLOW").await["conversation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    runner_support::turns_started(&fake, 1).await;
    background.stop();
    let options = townhall_core::ContextOptions {
        claude_bin: fake.bin(),
        ..townhall_core::ContextOptions::default()
    };
    let store = state.ctx().store().unwrap().clone();
    let restarted = AppState::with_store(env.workspace(&env.startup), options, store);
    let again = start_background(&restarted).await;
    let d: Value = idle(&router(restarted.clone()), &id).await;
    assert!(entries_of(&d, "error").is_empty(), "{d}");
    assert_eq!(fake.turns(), 2, "the interrupted reply was resumed");
    again.stop();
}
