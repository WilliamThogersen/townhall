use axum::http::StatusCode;
use serde_json::{json, Value};
use townhall_core::model::MAX_ATTACHMENT_BYTES;
use townhall_daemon::routes::UPLOAD_BODY_LIMIT;

mod common;
mod runner_support;
mod upload_support;
use common::{call, send};
use upload_support::{
    file, request, upload, upload_ok, upload_parts, Part, Setup, GIF, JPEG, PNG, WEBP,
};

const MAX: usize = MAX_ATTACHMENT_BYTES as usize;

fn is_id(v: &Value) -> bool {
    let id = v.as_str().unwrap();
    id.len() == 12 && id.chars().all(|c| c.is_ascii_hexdigit())
}

#[tokio::test]
async fn uploads_images_and_files_into_the_conversation_folder() {
    let t = Setup::new().await;
    let png = upload_ok(&t.app, &t.id, file("shot.png", "image/png", PNG)).await;
    assert!(is_id(&png["id"]), "{png}");
    assert_eq!(png["conversationId"], t.id.as_str());
    assert_eq!(
        (&png["name"], &png["mime"], &png["kind"]),
        (&json!("shot.png"), &json!("image/png"), &json!("image"))
    );
    assert_eq!(png["size"], PNG.len());
    assert!(png["createdAt"].as_str().unwrap().ends_with('Z'));
    let jpeg = upload_ok(&t.app, &t.id, file("photo.jpg", "image/jpeg", JPEG)).await;
    assert_eq!(
        (&jpeg["mime"], &jpeg["kind"]),
        (&json!("image/jpeg"), &json!("image"))
    );
    let text = upload_ok(&t.app, &t.id, file("notes.txt", "text/plain", b"hello")).await;
    assert_eq!(
        (&text["name"], &text["mime"], &text["kind"], &text["size"]),
        (
            &json!("notes.txt"),
            &json!("text/plain"),
            &json!("file"),
            &json!(5)
        )
    );
    let on_disk = std::fs::read(t.file_path(png["id"].as_str().unwrap(), "shot.png")).unwrap();
    assert_eq!(on_disk, PNG);
    let on_disk = std::fs::read(t.file_path(text["id"].as_str().unwrap(), "notes.txt")).unwrap();
    assert_eq!(on_disk, b"hello");
}

#[tokio::test]
async fn an_image_type_without_image_bytes_is_a_file() {
    let t = Setup::new().await;
    let fake = upload_ok(&t.app, &t.id, file("fake.png", "image/png", b"just text")).await;
    assert_eq!(
        (&fake["kind"], &fake["mime"], &fake["name"]),
        (
            &json!("file"),
            &json!("application/octet-stream"),
            &json!("fake.png")
        )
    );
    let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";
    let svg = upload_ok(&t.app, &t.id, file("logo.svg", "image/svg+xml", svg)).await;
    assert_eq!(
        (&svg["kind"], &svg["mime"]),
        (&json!("file"), &json!("application/octet-stream"))
    );
}

#[tokio::test]
async fn images_are_told_by_their_bytes_and_named_for_their_type() {
    let t = Setup::new().await;
    let cases: [(&str, &str, &[u8], &str, &str); 5] = [
        (
            "clip",
            "application/octet-stream",
            GIF,
            "image/gif",
            "clip.gif",
        ),
        ("anim.webp", "", WEBP, "image/webp", "anim.webp"),
        ("shot.txt", "text/plain", PNG, "image/png", "shot.png"),
        ("photo.JPEG", "image/png", JPEG, "image/jpeg", "photo.JPEG"),
        ("photo.jpg", "image/jpeg", PNG, "image/png", "photo.png"),
    ];
    for (name, given, bytes, mime, saved) in cases {
        let part = Part {
            content_type: (!given.is_empty()).then_some(given),
            ..file(name, given, bytes)
        };
        let a = upload_ok(&t.app, &t.id, part).await;
        assert_eq!(
            (&a["kind"], &a["mime"], &a["name"]),
            (&json!("image"), &json!(mime), &json!(saved)),
            "{name}"
        );
    }
}

#[tokio::test]
async fn other_types_come_from_the_part_then_the_extension() {
    let t = Setup::new().await;
    let by_part = upload_ok(&t.app, &t.id, file("rows.bin", "Text/CSV", b"a,b")).await;
    assert_eq!(by_part["mime"], "text/csv");
    let part = Part {
        content_type: None,
        ..file("data.json", "", b"{}")
    };
    assert_eq!(
        upload_ok(&t.app, &t.id, part).await["mime"],
        "application/json"
    );
    let unknown = file("blob", "application/octet-stream", b"\x00\x01");
    assert_eq!(
        upload_ok(&t.app, &t.id, unknown).await["mime"],
        "application/octet-stream"
    );
}

#[tokio::test]
async fn over_20_mb_is_413_and_nothing_is_kept() {
    let t = Setup::new().await;
    let big = vec![b'a'; MAX + 1];
    let (s, v) = upload(
        &t.app,
        &t.id,
        file("big.bin", "application/octet-stream", &big),
    )
    .await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE, "{v}");
    assert_eq!(
        v,
        json!({"success": false, "data": null, "error": "big.bin is over 20 MB"})
    );
    let huge = vec![b'a'; UPLOAD_BODY_LIMIT + 1];
    let (s, v) = upload(
        &t.app,
        &t.id,
        file("huge.bin", "application/octet-stream", &huge),
    )
    .await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE, "{v}");
    assert_eq!(v["success"], false);
    assert!(t.list().await.is_empty());
    let folder = t.env.home.root().join("conversations").join(&t.id);
    assert!(
        !folder.join("files").exists()
            || std::fs::read_dir(folder.join("files")).unwrap().count() == 0
    );
    let exact = vec![b'a'; MAX];
    let a = upload_ok(
        &t.app,
        &t.id,
        file("exact.bin", "application/octet-stream", &exact),
    )
    .await;
    assert_eq!(a["size"], MAX);
    assert_eq!(t.list().await.len(), 1);
}

#[tokio::test]
async fn a_foreign_origin_is_403_before_anything_else() {
    let t = Setup::new().await;
    let body = || upload_support::body(&[file("shot.png", "image/png", PNG)]);
    for id in [t.id.as_str(), "abc123"] {
        let (s, v) = send(&t.app, request(id, body(), Some("https://evil.example"))).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
        assert_eq!(
            v,
            json!({"success": false, "data": null, "error": "origin not allowed"})
        );
    }
    assert!(t.list().await.is_empty());
    let (s, v) = send(
        &t.app,
        request(&t.id, body(), Some("http://localhost:5173")),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
}

#[tokio::test]
async fn unknown_conversations_are_404() {
    let t = Setup::new().await;
    let (s, v) = upload(&t.app, "abc123", file("shot.png", "image/png", PNG)).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["error"], "conversation `abc123` not found");
    let (s, _) = call(&t.app, "GET", "/api/conversations/abc123/attachments", None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert!(!t
        .env
        .home
        .root()
        .join("conversations")
        .join("abc123")
        .exists());
}

#[tokio::test]
async fn a_text_field_named_file_is_not_a_file() {
    let t = Setup::new().await;
    let text = Part {
        field: "file",
        file_name: None,
        content_type: None,
        bytes: b"hello",
    };
    let (s, v) = upload(&t.app, &t.id, text).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"], "send one file in a field named `file`");
    assert!(t.list().await.is_empty());
}

#[tokio::test]
async fn bad_uploads_are_400() {
    let t = Setup::new().await;
    let note = Part {
        field: "note",
        file_name: None,
        content_type: None,
        bytes: b"hi",
    };
    let bodies: [&[Part]; 4] = [
        &[file("empty.txt", "text/plain", b"")],
        &[note],
        &[],
        &[
            file("a.txt", "text/plain", b"a"),
            file("b.txt", "text/plain", b"b"),
        ],
    ];
    let errors = [
        "empty.txt is empty",
        "send one file in a field named `file`",
        "send one file in a field named `file`",
        "send one file in a field named `file`",
    ];
    for (parts, error) in bodies.iter().zip(errors) {
        let (s, v) = upload_parts(&t.app, &t.id, parts).await;
        assert_eq!(
            (s, v["error"].as_str().unwrap()),
            (StatusCode::BAD_REQUEST, error)
        );
    }
    let uri = upload_support::uri(&t.id);
    let (s, v) = call(&t.app, "POST", &uri, Some(json!({"file": "x"}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"], "send the file as multipart/form-data");
    let garbage = request(&t.id, b"--nope\r\nnot a part".to_vec(), None);
    let (s, v) = send(&t.app, garbage).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"], "the upload is not valid multipart form data");
    assert!(t.list().await.is_empty());
}

#[tokio::test]
async fn hostile_names_are_cleaned() {
    let t = Setup::new().await;
    let long = format!("{}.txt", "n".repeat(300));
    let cases: [(&str, &[u8], &str); 8] = [
        ("../../etc/passwd", b"root", "passwd"),
        ("a/b.png", PNG, "b.png"),
        ("..\\..\\win.txt", b"w", "win.txt"),
        ("tab\there.txt", b"t", "tabhere.txt"),
        ("...", b"d", "file"),
        ("../", PNG, "file.png"),
        ("\u{202e}gnp.exe", b"x", "gnp.exe"),
        (" .hidden.txt. ", b"h", "hidden.txt"),
    ];
    for (raw, bytes, want) in cases {
        let a = upload_ok(&t.app, &t.id, file(raw, "text/plain", bytes)).await;
        assert_eq!(a["name"], want, "{raw:?}");
        let path = t.file_path(a["id"].as_str().unwrap(), want);
        assert_eq!(std::fs::read(path).unwrap(), bytes, "{raw:?}");
    }
    let a = upload_ok(&t.app, &t.id, file(&long, "text/plain", b"l")).await;
    let name = a["name"].as_str().unwrap();
    assert!(name.len() <= 120 && name.ends_with("nnn.txt"), "{name}");
    for raw in ["a\0b.txt", "a\nb.txt", "a\rb.txt"] {
        let (s, v) = upload(&t.app, &t.id, file(raw, "text/plain", b"x")).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{raw:?}: {v}");
    }
    assert_eq!(t.list().await.len(), cases.len() + 1);
}
