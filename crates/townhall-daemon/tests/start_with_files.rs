use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;

mod common;
mod runner_support;
mod upload_support;
use common::{send, Env};
use runner_support::{app_with, Fake};
use upload_support::{body, file, Part, BOUNDARY, PNG};

fn text(value: &'static str) -> Part<'static> {
    Part {
        field: "text",
        file_name: None,
        content_type: None,
        bytes: value.as_bytes(),
    }
}

fn start_request(parts: &[Part]) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/conversations")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body(parts)))
        .unwrap()
}

async fn start(parts: &[Part<'_>]) -> (StatusCode, Value) {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    send(&app, start_request(parts)).await
}

#[tokio::test]
async fn starts_a_conversation_with_its_files_in_the_first_message() {
    let (s, v) = start(&[
        text("match these"),
        file("shot.png", "image/png", PNG),
        file("notes.txt", "text/plain", b"steps"),
    ])
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let data = &v["data"];
    let attached = data["attachments"].as_array().unwrap();
    assert_eq!(attached.len(), 2, "{v}");
    assert_eq!(attached[0]["name"], "shot.png");
    assert_eq!(attached[0]["kind"], "image");
    assert_eq!(attached[1]["name"], "notes.txt");
    let you = data["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "you")
        .unwrap_or_else(|| panic!("{v}"));
    assert_eq!(you["text"], "match these");
    assert_eq!(
        you["attachments"],
        serde_json::json!([attached[0]["id"], attached[1]["id"]])
    );
}

#[tokio::test]
async fn starts_with_files_and_no_text() {
    let (s, v) = start(&[file("shot.png", "image/png", PNG)]).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["data"]["attachments"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn refuses_a_start_with_neither_text_nor_files() {
    let (s, v) = start(&[text("   ")]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
}

#[tokio::test]
async fn refuses_an_empty_file_and_creates_nothing() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let (s, v) = send(
        &app,
        start_request(&[text("hi"), file("empty.png", "image/png", b"")]),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    let (_, list) = common::call(&app, "GET", "/api/conversations", None).await;
    assert_eq!(list["data"].as_array().unwrap().len(), 0, "{list}");
}

#[tokio::test]
async fn refuses_an_unknown_field() {
    let other = Part {
        field: "other",
        file_name: None,
        content_type: None,
        bytes: b"x",
    };
    let (s, v) = start(&[text("hi"), other]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
}
