//! The content blocks of one user turn. Text only for now: unit 2.6 adds the images of
//! image attachments here (`Store::attachment` gives the record and its file).

use serde_json::{json, Value};

use crate::model::Attachment;
use crate::store::Store;
use crate::Result;

fn listing(attachments: &[Attachment]) -> String {
    let lines: Vec<String> = attachments
        .iter()
        .map(|a| format!("- {} ({}, {})", a.id, a.name, a.mime))
        .collect();
    format!(
        "attachments of this message (pass their ids to create_task):\n{}",
        lines.join("\n")
    )
}

/// The text, then a note listing the attachments by id so the session can hand them on.
pub async fn user_content(
    store: &Store,
    conversation_id: &str,
    text: &str,
    attachment_ids: &[String],
) -> Result<Vec<Value>> {
    let mut found = Vec::with_capacity(attachment_ids.len());
    for id in attachment_ids {
        found.push(store.attachment(conversation_id, id).await?.0);
    }
    let note = (!found.is_empty()).then(|| listing(&found));
    let text = match note {
        Some(note) if text.trim().is_empty() => note,
        Some(note) => format!("{text}\n\n{note}"),
        None => text.to_string(),
    };
    Ok(vec![json!({ "type": "text", "text": text })])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::Home;

    #[tokio::test]
    async fn text_and_an_attachment_note() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Store::open(&Home::at(dir.path().join(".townhall")), EventBus::new()).unwrap();
        let c = store.create_conversation().await.unwrap();
        let plain = user_content(&store, &c.id, "hi", &[]).await.unwrap();
        assert_eq!(plain, [json!({"type": "text", "text": "hi"})]);
        let a = store
            .save_attachment(&c.id, "shot.png", "image/png", vec![1, 2])
            .await
            .unwrap();
        let with = user_content(&store, &c.id, "look", std::slice::from_ref(&a.id))
            .await
            .unwrap();
        let text = with[0]["text"].as_str().unwrap();
        assert!(text.starts_with("look\n\nattachments"), "{text}");
        assert!(text.contains(&format!("- {} (shot.png, image/png)", a.id)));
        let only = user_content(&store, &c.id, " ", &[a.id]).await.unwrap();
        assert!(only[0]["text"].as_str().unwrap().starts_with("attachments"));
        assert!(user_content(&store, &c.id, "x", &["nope".into()])
            .await
            .is_err());
    }
}
