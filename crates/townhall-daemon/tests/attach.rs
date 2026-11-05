use std::net::SocketAddr;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Error as WsError;

mod common;
mod runner_support;
use common::{send, Env};
use runner_support::{app_with, Fake};

async fn serve(app: axum::Router) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn attach_request(addr: SocketAddr, origin: Option<&'static str>) -> Request<()> {
    let mut req = format!("ws://{addr}/api/agents/ghost/attach")
        .into_client_request()
        .unwrap();
    if let Some(o) = origin {
        req.headers_mut()
            .insert("origin", HeaderValue::from_static(o));
    }
    req
}

fn refused_status(error: WsError) -> StatusCode {
    match error {
        WsError::Http(res) => res.status(),
        other => panic!("expected an http refusal, got {other}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_foreign_origin_is_refused_before_the_upgrade() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let addr = serve(app.clone()).await;
    let refused =
        tokio_tungstenite::connect_async(attach_request(addr, Some("https://evil.example")))
            .await
            .unwrap_err();
    assert_eq!(refused_status(refused), StatusCode::FORBIDDEN);
    let req = Request::get("/api/agents/ghost/attach")
        .header("origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    let (s, v) = send(&app, req).await;
    assert_eq!(
        (s, v["success"].as_bool(), v["error"].as_str()),
        (
            StatusCode::FORBIDDEN,
            Some(false),
            Some("origin not allowed")
        )
    );
}

#[tokio::test]
async fn a_plain_get_is_told_to_upgrade() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let req = Request::get("/api/agents/ghost/attach")
        .body(Body::empty())
        .unwrap();
    let (s, v) = send(&app, req).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["error"], "this route needs a websocket upgrade");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_allowed_origin_still_gets_404_for_an_agent_that_does_not_exist() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let addr = serve(app).await;
    let refused = tokio_tungstenite::connect_async(attach_request(addr, Some("tauri://localhost")))
        .await
        .unwrap_err();
    assert_eq!(refused_status(refused), StatusCode::NOT_FOUND);
}
