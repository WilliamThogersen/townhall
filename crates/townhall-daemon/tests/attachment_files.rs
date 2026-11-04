use axum::http::StatusCode;
use serde_json::json;

mod common;
mod runner_support;
mod upload_support;
use common::call;
use upload_support::{fetch, file, header, new_conversation, upload_ok, Setup, PNG};

fn file_uri(id: &str, aid: &str) -> String {
    format!("/api/conversations/{id}/attachments/{aid}")
}

#[tokio::test]
async fn list_gives_every_attachment_in_upload_order() {
    let t = Setup::new().await;
    assert!(t.list().await.is_empty());
    let a = upload_ok(&t.app, &t.id, file("shot.png", "image/png", PNG)).await;
    let b = upload_ok(&t.app, &t.id, file("notes.txt", "text/plain", b"n")).await;
    assert_eq!(t.list().await, [a.clone(), b]);
    let other = new_conversation(&t.state).await;
    let (s, v) = call(&t.app, "GET", &upload_support::uri(&other), None).await;
    assert_eq!((s, v["data"].clone()), (StatusCode::OK, json!([])));
    let (s, v) = call(&t.app, "GET", &format!("/api/conversations/{}", t.id), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["data"]["attachments"][0], a);
}

#[tokio::test]
async fn an_image_is_served_inline_with_its_type() {
    let t = Setup::new().await;
    let a = upload_ok(&t.app, &t.id, file("shot.png", "image/png", PNG)).await;
    let (s, h, body) = fetch(&t.app, &file_uri(&t.id, a["id"].as_str().unwrap())).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, PNG);
    assert_eq!(header(&h, "content-type"), "image/png");
    assert_eq!(header(&h, "content-length"), PNG.len().to_string());
    assert_eq!(
        header(&h, "content-disposition"),
        "inline; filename=\"shot.png\"; filename*=UTF-8''shot.png"
    );
    assert_eq!(header(&h, "x-content-type-options"), "nosniff");
    assert_eq!(header(&h, "content-security-policy"), "sandbox");
}

#[tokio::test]
async fn other_files_are_downloads_with_a_safe_name() {
    let t = Setup::new().await;
    let html = b"<script>alert(1)</script>";
    let page = upload_ok(&t.app, &t.id, file("page.html", "text/html", html)).await;
    let (s, h, body) = fetch(&t.app, &file_uri(&t.id, page["id"].as_str().unwrap())).await;
    assert_eq!((s, body.as_slice()), (StatusCode::OK, &html[..]));
    assert_eq!(header(&h, "content-type"), "text/html");
    assert_eq!(
        header(&h, "content-disposition"),
        "attachment; filename=\"page.html\"; filename*=UTF-8''page.html"
    );
    assert_eq!(header(&h, "x-content-type-options"), "nosniff");
    let fake = upload_ok(&t.app, &t.id, file("fake.png", "image/png", b"text")).await;
    let (_, h, _) = fetch(&t.app, &file_uri(&t.id, fake["id"].as_str().unwrap())).await;
    assert_eq!(header(&h, "content-type"), "application/octet-stream");
    assert!(header(&h, "content-disposition").starts_with("attachment;"));
    let odd = upload_ok(&t.app, &t.id, file("résumé 100%;.txt", "text/plain", b"cv")).await;
    let (_, h, _) = fetch(&t.app, &file_uri(&t.id, odd["id"].as_str().unwrap())).await;
    assert_eq!(
        header(&h, "content-disposition"),
        "attachment; filename=\"r_sum_ 100_;.txt\"; filename*=UTF-8''r%C3%A9sum%C3%A9%20100%25%3B.txt"
    );
}

#[tokio::test]
async fn unknown_or_foreign_attachments_are_404() {
    let t = Setup::new().await;
    let a = upload_ok(&t.app, &t.id, file("shot.png", "image/png", PNG)).await;
    let aid = a["id"].as_str().unwrap();
    let other = new_conversation(&t.state).await;
    for uri in [
        file_uri("abc123", aid),
        file_uri(&t.id, "000000000000"),
        file_uri(&other, aid),
    ] {
        let (s, v) = call(&t.app, "GET", &uri, None).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{uri}: {v}");
        assert_eq!(v["success"], false);
    }
}

#[tokio::test]
async fn a_download_never_leaves_the_attachment_folder() {
    let t = Setup::new().await;
    let a = upload_ok(&t.app, &t.id, file("notes.txt", "text/plain", b"mine")).await;
    let aid = a["id"].as_str().unwrap();
    for bad in ["..%2F..%2Ftownhall.db", "%2E%2E", "a.b"] {
        let (s, _, body) = fetch(&t.app, &file_uri(&t.id, bad)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad}");
        assert!(!body.starts_with(b"SQLite"), "{bad}");
    }
    let path = t.file_path(aid, "notes.txt");
    let secret = t.env.root.path().join("secret.txt");
    std::fs::write(&secret, "secret").unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&secret, &path).unwrap();
    let (s, _, body) = fetch(&t.app, &file_uri(&t.id, aid)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_ne!(body, b"secret");
    std::fs::remove_file(&path).unwrap();
    let (s, v) = call(&t.app, "GET", &file_uri(&t.id, aid), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(
        v["error"],
        format!("the file of attachment `{aid}` is gone")
    );
}

#[tokio::test]
async fn the_store_hands_tasks_the_absolute_path_of_the_bytes() {
    let t = Setup::new().await;
    let a = upload_ok(&t.app, &t.id, file("shot", "image/png", PNG)).await;
    let aid = a["id"].as_str().unwrap();
    let store = t.state.ctx().store().unwrap();
    let (record, path) = store.attachment(&t.id, aid).await.unwrap();
    assert_eq!(record.name, "shot.png");
    assert!(path.is_absolute(), "{}", path.display());
    assert_eq!(path, t.file_path(aid, "shot.png"));
    assert_eq!(std::fs::read(&path).unwrap(), PNG);
}
