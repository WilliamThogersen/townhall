use serde::{Deserialize, Serialize};

/// Uploads above this are refused (`413`).
pub const MAX_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttachmentKind {
    /// Goes to townhall's session as image input.
    Image,
    /// Stored with the conversation; tasks get its path.
    File,
}

impl AttachmentKind {
    pub fn of_mime(mime: &str) -> Self {
        if mime.starts_with("image/") {
            Self::Image
        } else {
            Self::File
        }
    }
}

/// One entry of `conversations/<id>/attachments.json`; the bytes are in `files/<attachment id>/<name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub conversation_id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub kind: AttachmentKind,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_follows_mime() {
        assert_eq!(AttachmentKind::of_mime("image/png"), AttachmentKind::Image);
        assert_eq!(AttachmentKind::of_mime("text/plain"), AttachmentKind::File);
        let a = Attachment {
            id: "x".into(),
            conversation_id: "c".into(),
            name: "shot.png".into(),
            mime: "image/png".into(),
            size: 3,
            kind: AttachmentKind::Image,
            created_at: "2026-11-04T10:42:05.000Z".into(),
        };
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["conversationId"], "c");
        assert_eq!(v["kind"], "image");
        assert_eq!(v["createdAt"], "2026-11-04T10:42:05.000Z");
    }
}
