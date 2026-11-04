#![allow(dead_code)]

//! Multipart bodies for the attachment upload route, built by hand, and a conversation
//! to upload into (made through the store, so no turn of claude runs).

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use townhall_daemon::AppState;

use crate::common::{send, Env};
use crate::runner_support::{app_with, Fake};

pub const BOUNDARY: &str = "townhall-test-boundary";
pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRpixels";
pub const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUnBpeGVscw==";
pub const JPEG: &[u8] = b"\xff\xd8\xff\xe0\0\x10JFIFpixels";
pub const GIF: &[u8] = b"GIF89a\x01\0\x01\0pixels";
pub const GIF_B64: &str = "R0lGODlhAQABAHBpeGVscw==";
pub const WEBP: &[u8] = b"RIFF\x10\0\0\0WEBPVP8 pixels";

/// One part of a multipart body.
pub struct Part<'a> {
    pub field: &'a str,
    pub file_name: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub bytes: &'a [u8],
}

/// A `file` part.
pub fn file<'a>(name: &'a str, content_type: &'a str, bytes: &'a [u8]) -> Part<'a> {
    Part {
        field: "file",
        file_name: Some(name),
        content_type: Some(content_type),
        bytes,
    }
}

pub fn body(parts: &[Part]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in parts {
        out.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        let mut disposition = format!("Content-Disposition: form-data; name=\"{}\"", p.field);
        if let Some(name) = p.file_name {
            disposition.push_str(&format!("; filename=\"{name}\""));
        }
        out.extend_from_slice(format!("{disposition}\r\n").as_bytes());
        if let Some(t) = p.content_type {
            out.extend_from_slice(format!("Content-Type: {t}\r\n").as_bytes());
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(p.bytes);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    out
}

pub fn uri(id: &str) -> String {
    format!("/api/conversations/{id}/attachments")
}

pub fn request(id: &str, body: Vec<u8>, origin: Option<&str>) -> Request<Body> {
    let builder = Request::builder().method("POST").uri(uri(id)).header(
        "content-type",
        format!("multipart/form-data; boundary={BOUNDARY}"),
    );
    let builder = match origin {
        Some(o) => builder.header("origin", o),
        None => builder,
    };
    builder.body(Body::from(body)).unwrap()
}

pub async fn upload_parts(app: &Router, id: &str, parts: &[Part<'_>]) -> (StatusCode, Value) {
    send(app, request(id, body(parts), None)).await
}

pub async fn upload(app: &Router, id: &str, part: Part<'_>) -> (StatusCode, Value) {
    upload_parts(app, id, &[part]).await
}

/// Uploads `part`, expects `201` and returns the `Attachment`.
pub async fn upload_ok(app: &Router, id: &str, part: Part<'_>) -> Value {
    let (s, v) = upload(app, id, part).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    v["data"].clone()
}

/// A GET that keeps the headers and the raw body.
pub async fn fetch(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let (parts, body) = res.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes().to_vec();
    (parts.status, parts.headers, bytes)
}

pub fn header<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
    headers
        .get(name)
        .unwrap_or_else(|| panic!("no {name} header"))
        .to_str()
        .unwrap()
}

/// A daemon whose claude is the fake, with one conversation made through the store.
pub struct Setup {
    pub env: Env,
    pub fake: Fake,
    pub state: AppState,
    pub app: Router,
    pub id: String,
}

impl Setup {
    pub async fn new() -> Self {
        let env = Env::new("alpha").await;
        let fake = Fake::install(env.root.path());
        let (state, app) = app_with(&env, &fake);
        let id = new_conversation(&state).await;
        Self {
            env,
            fake,
            state,
            app,
            id,
        }
    }

    /// Where the bytes of attachment `aid` named `name` live.
    pub fn file_path(&self, aid: &str, name: &str) -> std::path::PathBuf {
        self.env
            .home
            .root()
            .join("conversations")
            .join(&self.id)
            .join("files")
            .join(aid)
            .join(name)
    }

    pub async fn list(&self) -> Vec<Value> {
        let (s, v) = crate::common::call(&self.app, "GET", &uri(&self.id), None).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        v["data"].as_array().unwrap().clone()
    }
}

pub async fn new_conversation(state: &AppState) -> String {
    let store = state.ctx().store().unwrap();
    store.create_conversation().await.unwrap().id
}
