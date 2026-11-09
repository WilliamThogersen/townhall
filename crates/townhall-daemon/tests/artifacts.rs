//! The artifact routes: the list for the web app and the bytes of one artifact, served so a
//! browser can neither run nor sniff them.

use axum::http::StatusCode;
use serde_json::json;
use townhall_core::model::{ArtifactKind, AuthorRole, NewArtifact, NewTask};

mod common;
mod mcp_support;
mod runner_support;
mod upload_support;
use common::call;
use mcp_support::Mcp;
use upload_support::{fetch, header, PNG};

async fn setup() -> Mcp {
    let m = Mcp::new().await;
    for name in ["build-it", "test-it"] {
        let new = NewTask {
            name: name.into(),
            title: "fix".into(),
            prompt: "the brief".into(),
            repo: "shop".into(),
            base_branch: "main".into(),
            after: vec![],
            attachments: vec![],
            agent: "builder".into(),
            pr_base: None,
            done: Default::default(),
        };
        m.store().create_task(&m.id, new).await.unwrap();
    }
    m
}

fn new(name: &str, kind: ArtifactKind, bytes: &[u8]) -> NewArtifact {
    NewArtifact {
        name: name.into(),
        kind,
        author_role: AuthorRole::Tester,
        caption: Some("what it shows".into()),
        bytes: bytes.to_vec(),
    }
}

fn uri(task: &str, id: &str) -> String {
    format!("/api/tasks/{task}/artifacts/{id}")
}

#[tokio::test]
async fn the_list_gives_every_artifact_of_a_task_oldest_first() {
    let m = setup().await;
    let first = m
        .store()
        .add_artifact("build-it", new("a.log", ArtifactKind::Log, b"one"))
        .await
        .unwrap();
    let second = m
        .store()
        .add_artifact("build-it", new("b.png", ArtifactKind::Screenshot, PNG))
        .await
        .unwrap();
    let (s, v) = call(&m.app, "GET", "/api/tasks/build-it/artifacts", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"], json!([first, second]));
    assert_eq!(v["data"][1]["authorRole"], "tester");
    assert_eq!(v["data"][1]["conversationId"], m.id);
    let (_, empty) = call(&m.app, "GET", "/api/tasks/test-it/artifacts", None).await;
    assert_eq!(empty["data"], json!([]));
    let (s, v) = call(&m.app, "GET", "/api/tasks/ghost/artifacts", None).await;
    assert_eq!(
        (s, v["success"].clone()),
        (StatusCode::NOT_FOUND, json!(false))
    );
}

#[tokio::test]
async fn an_image_is_served_inline_and_sandboxed() {
    let m = setup().await;
    let a = m
        .store()
        .add_artifact("build-it", new("shot", ArtifactKind::Screenshot, PNG))
        .await
        .unwrap();
    let (s, h, body) = fetch(&m.app, &uri("build-it", &a.id)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, PNG);
    assert_eq!(header(&h, "content-type"), "image/png");
    assert_eq!(header(&h, "content-length"), PNG.len().to_string());
    assert!(header(&h, "content-disposition").starts_with("inline; filename=\"shot.png\""));
    assert_eq!(header(&h, "x-content-type-options"), "nosniff");
    assert_eq!(
        header(&h, "content-security-policy"),
        "sandbox; default-src 'none'"
    );
}

#[tokio::test]
async fn text_is_served_as_text_never_as_html() {
    let m = setup().await;
    let html = b"<html><script>alert(document.cookie)</script></html>";
    let note = m
        .store()
        .add_artifact("build-it", new("page.html", ArtifactKind::Note, html))
        .await
        .unwrap();
    let (s, h, body) = fetch(&m.app, &uri("build-it", &note.id)).await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, &html[..]));
    assert_eq!(header(&h, "content-type"), "text/markdown; charset=utf-8");
    assert_eq!(header(&h, "x-content-type-options"), "nosniff");
    assert_eq!(
        header(&h, "content-security-policy"),
        "sandbox; default-src 'none'"
    );
    let log = m
        .store()
        .add_artifact("build-it", new("run.log", ArtifactKind::Log, b"ok"))
        .await
        .unwrap();
    let (_, h, _) = fetch(&m.app, &uri("build-it", &log.id)).await;
    assert_eq!(header(&h, "content-type"), "text/plain; charset=utf-8");
}

#[tokio::test]
async fn a_row_with_an_unsafe_type_is_a_download() {
    let m = setup().await;
    let a = m
        .store()
        .add_artifact("build-it", new("x.log", ArtifactKind::Log, b"ok"))
        .await
        .unwrap();
    m.store()
        .call({
            let id = a.id.clone();
            move |conn| {
                conn.execute(
                    "update artifacts set mime = 'text/html' where id = ?1",
                    [id],
                )?;
                Ok(())
            }
        })
        .await
        .unwrap();
    let (s, h, _) = fetch(&m.app, &uri("build-it", &a.id)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(header(&h, "content-type"), "application/octet-stream");
    assert!(header(&h, "content-disposition").starts_with("attachment;"));
}

#[tokio::test]
async fn unknown_and_foreign_artifacts_are_404_and_bad_ids_are_400() {
    let m = setup().await;
    let a = m
        .store()
        .add_artifact("build-it", new("a.log", ArtifactKind::Log, b"x"))
        .await
        .unwrap();
    for bad in [
        uri("ghost", &a.id),
        uri("build-it", "000000000000"),
        uri("test-it", &a.id),
    ] {
        let (s, v) = call(&m.app, "GET", &bad, None).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{bad}: {v}");
        assert_eq!(v["success"], false);
    }
    for bad in ["..%2F..%2Ftownhall.db", "%2E%2E", "a.b"] {
        let (s, _, body) = fetch(&m.app, &uri("build-it", bad)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}");
        assert!(!body.starts_with(b"SQLite"), "{bad}");
    }
}

#[tokio::test]
async fn a_swapped_file_is_never_followed() {
    let m = setup().await;
    let a = m
        .store()
        .add_artifact("build-it", new("a.log", ArtifactKind::Log, b"mine"))
        .await
        .unwrap();
    let (_, path) = m.store().artifact("build-it", &a.id).await.unwrap();
    let secret = m.env.root.path().join("secret.txt");
    std::fs::write(&secret, "secret").unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&secret, &path).unwrap();
    let (s, _, body) = fetch(&m.app, &uri("build-it", &a.id)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_ne!(body, b"secret");
    std::fs::remove_file(&path).unwrap();
    let (s, v) = call(&m.app, "GET", &uri("build-it", &a.id), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(
        v["error"],
        format!("the file of artifact `{}` is gone", a.id)
    );
}
