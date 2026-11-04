use axum::http::StatusCode;
use serde_json::json;

mod common;
mod runner_support;
mod upload_support;
use common::{call, Env};
use runner_support::{app_with, create, entries_of, idle, Fake};
use upload_support::{file, upload_ok, GIF, GIF_B64, PNG, PNG_B64};

#[tokio::test]
async fn images_go_to_the_session_as_image_blocks_and_files_in_the_note() {
    let env = Env::new("alpha").await;
    let fake = Fake::install(env.root.path());
    let (_state, app) = app_with(&env, &fake);
    let created = create(&app, "plan a logo change").await;
    let id = created["conversation"]["id"].as_str().unwrap().to_string();
    idle(&app, &id).await;
    let png = upload_ok(&app, &id, file("shot.png", "image/png", PNG)).await;
    let txt = upload_ok(&app, &id, file("notes.txt", "text/plain", b"steps")).await;
    let gif = upload_ok(&app, &id, file("clip.gif", "image/gif", GIF)).await;
    let ids = [&png["id"], &txt["id"], &gif["id"]];
    let body = json!({"text": "match these", "attachments": ids});
    let uri = format!("/api/conversations/{id}/messages");
    let (s, v) = call(&app, "POST", &uri, Some(body)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["attachments"], json!(ids));
    let d = idle(&app, &id).await;
    assert!(entries_of(&d, "error").is_empty(), "{d}");

    let line = &fake.stdin(2)[0];
    assert_eq!(line["type"], "user");
    assert_eq!(line["message"]["role"], "user");
    let image = |media_type: &str, data: &str| json!({"type": "image", "source": {"type": "base64", "media_type": media_type, "data": data}});
    let note = format!(
        "match these\n\nattachments of this message (pass their ids to create_task):\n\
         - {} (shot.png, image/png, image 1 above)\n\
         - {} (notes.txt, text/plain)\n\
         - {} (clip.gif, image/gif, image 2 above)",
        ids[0].as_str().unwrap(),
        ids[1].as_str().unwrap(),
        ids[2].as_str().unwrap()
    );
    assert_eq!(
        line["message"]["content"],
        json!([
            image("image/png", PNG_B64),
            image("image/gif", GIF_B64),
            {"type": "text", "text": note}
        ])
    );
}
