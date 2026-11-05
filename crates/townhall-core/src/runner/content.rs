//! The content blocks of one user turn: the image attachments first, as stream-json image
//! blocks (the API reads images before text best), then the text with a note that lists
//! every attachment by id, so the session can pass them to `create_task`.

use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use crate::model::{Attachment, AttachmentKind};
use crate::redact::redact;
use crate::store::Store;
use crate::Result;

/// Images sent per message; the rest are only listed.
pub const MAX_IMAGES: usize = 10;
/// The base64 of one image: 5 MB, which every platform takes (a raw file of about 3.75 MB).
pub const MAX_IMAGE_BASE64: usize = 5 * 1024 * 1024;

const HEADING: &str = "attachments of this message (pass their ids to create_task):";
const TOO_MANY: &str = "not shown: at most 10 images per message";
const TOO_BIG: &str = "not shown: over 5 MB";
const NOT_AN_IMAGE: &str = "not shown: not a png, jpeg, gif or webp image";
const UNREADABLE: &str = "not shown: its file cannot be read";

/// How one attachment reaches the session.
enum Shown {
    /// The n-th image block, from 1.
    Image(usize),
    File,
    /// An image that went in as a note only, and why.
    Listed(&'static str),
}

/// The media type the bytes start with, for the four the API reads as images.
fn image_type(bytes: &[u8]) -> Option<&'static str> {
    let webp = bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP";
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else {
        webp.then_some("image/webp")
    }
}

fn fits(raw_len: u64) -> bool {
    let encoded = raw_len.div_ceil(3).saturating_mul(4);
    encoded <= MAX_IMAGE_BASE64 as u64
}

/// The image block of an image attachment, or the note on why it is only listed.
async fn image_block(
    a: &Attachment,
    path: &Path,
    sent: usize,
) -> std::result::Result<Value, &'static str> {
    if sent >= MAX_IMAGES {
        return Err(TOO_MANY);
    }
    if !fits(a.size) {
        return Err(TOO_BIG);
    }
    let bytes = tokio::fs::read(path).await.map_err(|e| {
        tracing::warn!(
            target: "townhall::runner", event = "attachment.unreadable", attachment = %a.id,
            path = %path.display(), error = %redact(&e.to_string()), "cannot read attachment"
        );
        UNREADABLE
    })?;
    if !fits(u64::try_from(bytes.len()).unwrap_or(u64::MAX)) {
        return Err(TOO_BIG);
    }
    let media_type = image_type(&bytes).ok_or(NOT_AN_IMAGE)?;
    let source =
        json!({ "type": "base64", "media_type": media_type, "data": STANDARD.encode(&bytes) });
    Ok(json!({ "type": "image", "source": source }))
}

fn note_line(a: &Attachment, shown: &Shown) -> String {
    let what = format!("- {} ({}, {}", a.id, a.name, a.mime);
    match shown {
        Shown::Image(n) => format!("{what}, image {n} above)"),
        Shown::File => format!("{what})"),
        Shown::Listed(why) => format!("{what}, {why})"),
    }
}

fn with_note(text: &str, lines: &[String]) -> String {
    if lines.is_empty() {
        return text.to_string();
    }
    let note = format!("{HEADING}\n{}", lines.join("\n"));
    if text.trim().is_empty() {
        note
    } else {
        format!("{text}\n\n{note}")
    }
}

/// The image blocks, then the text with the note. Each id counts once; an unknown id is an
/// error, an image that cannot go in as one is listed with the reason.
pub async fn user_content(
    store: &Store,
    conversation_id: &str,
    text: &str,
    attachment_ids: &[String],
) -> Result<Vec<Value>> {
    let mut blocks = Vec::new();
    let mut lines = Vec::with_capacity(attachment_ids.len());
    for (i, id) in attachment_ids.iter().enumerate() {
        if attachment_ids[..i].contains(id) {
            continue;
        }
        let (a, path) = store.attachment(conversation_id, id).await?;
        let shown = match a.kind {
            AttachmentKind::File => Shown::File,
            AttachmentKind::Image => match image_block(&a, &path, blocks.len()).await {
                Ok(block) => {
                    blocks.push(block);
                    Shown::Image(blocks.len())
                }
                Err(why) => Shown::Listed(why),
            },
        };
        lines.push(note_line(&a, &shown));
    }
    blocks.push(json!({ "type": "text", "text": with_note(text, &lines) }));
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use crate::Home;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDRpixels";
    const JPEG: &[u8] = b"\xff\xd8\xff\xe0\0\x10JFIF";
    const WEBP: &[u8] = b"RIFF\x10\0\0\0WEBPVP8 ";

    struct Fixture {
        _dir: tempfile::TempDir,
        store: Store,
        id: String,
    }

    impl Fixture {
        async fn new() -> Self {
            let dir = tempfile::TempDir::new().unwrap();
            let home = Home::at(dir.path().join(".townhall"));
            let store = Store::open(&home, EventBus::new()).unwrap();
            let id = store.create_conversation().await.unwrap().id;
            Self {
                _dir: dir,
                store,
                id,
            }
        }

        async fn save(&self, name: &str, mime: &str, bytes: &[u8]) -> Attachment {
            let saved = self
                .store
                .save_attachment(&self.id, name, mime, bytes.to_vec());
            saved.await.unwrap()
        }

        async fn content(&self, text: &str, ids: &[String]) -> Vec<Value> {
            user_content(&self.store, &self.id, text, ids)
                .await
                .unwrap()
        }
    }

    fn image(media_type: &str, bytes: &[u8]) -> Value {
        use base64::Engine;
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        json!({"type": "image", "source": {"type": "base64", "media_type": media_type, "data": data}})
    }

    fn text_of(blocks: &[Value]) -> &str {
        let last = blocks.last().unwrap();
        assert_eq!(last["type"], "text");
        last["text"].as_str().unwrap()
    }

    #[tokio::test]
    async fn text_alone_is_one_text_block() {
        let f = Fixture::new().await;
        let plain = f.content("hi", &[]).await;
        assert_eq!(plain, [json!({"type": "text", "text": "hi"})]);
        let err = user_content(&f.store, &f.id, "x", &["nope".into()]).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn images_come_first_and_every_attachment_is_in_the_note() {
        let f = Fixture::new().await;
        let png = f.save("shot.png", "image/png", PNG).await;
        let doc = f.save("notes.txt", "text/plain", b"n").await;
        let webp = f.save("pic.webp", "image/webp", WEBP).await;
        let ids = [
            png.id.clone(),
            doc.id.clone(),
            webp.id.clone(),
            png.id.clone(),
        ];
        let blocks = f.content("look", &ids).await;
        assert_eq!(blocks.len(), 3, "{blocks:?}");
        assert_eq!(blocks[0], image("image/png", PNG));
        assert_eq!(blocks[1], image("image/webp", WEBP));
        let want = format!(
            "look\n\n{HEADING}\n- {} (shot.png, image/png, image 1 above)\n- {} (notes.txt, text/plain)\n- {} (pic.webp, image/webp, image 2 above)",
            png.id, doc.id, webp.id
        );
        assert_eq!(text_of(&blocks), want);
        let only = f.content(" ", std::slice::from_ref(&doc.id)).await;
        assert_eq!(
            text_of(&only),
            format!("{HEADING}\n- {} (notes.txt, text/plain)", doc.id)
        );
    }

    #[tokio::test]
    async fn the_bytes_decide_what_is_sent_as_an_image() {
        let f = Fixture::new().await;
        let fake = f.save("fake.png", "image/png", b"not an image").await;
        let svg = f.save("logo.svg", "image/svg+xml", b"<svg/>").await;
        let mislabeled = f.save("photo.jpg", "image/jpeg", PNG).await;
        let jpeg = f.save("photo.jpeg", "image/jpeg", JPEG).await;
        let ids = [
            fake.id.clone(),
            svg.id.clone(),
            mislabeled.id.clone(),
            jpeg.id.clone(),
        ];
        let blocks = f.content("x", &ids).await;
        assert_eq!(
            blocks[..2],
            [image("image/png", PNG), image("image/jpeg", JPEG)]
        );
        let text = text_of(&blocks);
        let not = "not shown: not a png, jpeg, gif or webp image";
        assert!(
            text.contains(&format!("- {} (fake.png, image/png, {not})", fake.id)),
            "{text}"
        );
        assert!(
            text.contains(&format!("- {} (logo.svg, image/svg+xml, {not})", svg.id)),
            "{text}"
        );
        assert!(
            text.contains("(photo.jpg, image/jpeg, image 1 above)"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn too_many_or_too_big_images_are_only_listed() {
        let f = Fixture::new().await;
        let mut ids = vec![];
        for n in 0..=MAX_IMAGES {
            ids.push(f.save(&format!("s{n}.png"), "image/png", PNG).await.id);
        }
        let blocks = f.content("many", &ids).await;
        assert_eq!(blocks.len(), MAX_IMAGES + 1);
        let last = format!(
            "- {} (s10.png, image/png, not shown: at most 10 images per message)",
            ids[10]
        );
        assert!(text_of(&blocks).ends_with(&last), "{}", text_of(&blocks));

        let fits = [PNG, &vec![0; MAX_IMAGE_BASE64 / 4 * 3 - PNG.len()]].concat();
        let over = [fits.as_slice(), b"x"].concat();
        let fits = f.save("fits.png", "image/png", &fits).await;
        let over = f.save("over.png", "image/png", &over).await;
        let blocks = f.content("big", &[fits.id.clone(), over.id.clone()]).await;
        assert_eq!(blocks.len(), 2);
        let data = blocks[0]["source"]["data"].as_str().unwrap();
        assert_eq!(data.len(), MAX_IMAGE_BASE64);
        let note = format!("- {} (over.png, image/png, not shown: over 5 MB)", over.id);
        assert!(text_of(&blocks).ends_with(&note), "{}", text_of(&blocks));
    }

    #[tokio::test]
    async fn an_image_whose_file_is_gone_is_listed() {
        let f = Fixture::new().await;
        let png = f.save("shot.png", "image/png", PNG).await;
        let (_, path) = f.store.attachment(&f.id, &png.id).await.unwrap();
        std::fs::remove_file(path).unwrap();
        let blocks = f.content("gone", std::slice::from_ref(&png.id)).await;
        assert_eq!(blocks.len(), 1);
        let note = format!(
            "- {} (shot.png, image/png, not shown: its file cannot be read)",
            png.id
        );
        assert!(text_of(&blocks).ends_with(&note), "{}", text_of(&blocks));
    }
}
