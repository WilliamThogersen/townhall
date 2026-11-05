use serde::{Deserialize, Serialize};

use crate::files::mime::image_extensions;

pub const MAX_ARTIFACT_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_ARTIFACTS_PER_TASK: usize = 50;
pub const MAX_CAPTION_CHARS: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    Screenshot,
    Log,
    Note,
    Report,
}

impl ArtifactKind {
    pub const ALL: [Self; 4] = [Self::Screenshot, Self::Log, Self::Note, Self::Report];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthorRole {
    Coder,
    Tester,
    Reviewer,
    Other,
}

impl AuthorRole {
    pub const ALL: [Self; 4] = [Self::Coder, Self::Tester, Self::Reviewer, Self::Other];
}

/// One piece of proof on a task. Bytes live in
/// `artifacts/<conversation id>/<task>/<id>-<name>` under the data folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub conversation_id: String,
    pub task: String,
    pub name: String,
    pub kind: ArtifactKind,
    pub mime: String,
    pub size: u64,
    pub author_role: AuthorRole,
    pub caption: Option<String>,
    pub created_at: String,
}

pub const LOG_MIME: &str = "text/plain; charset=utf-8";
pub const NOTE_MIME: &str = "text/markdown; charset=utf-8";

impl Artifact {
    pub fn shows_inline(&self) -> bool {
        self.mime == LOG_MIME
            || self.mime == NOTE_MIME
            || (self.kind == ArtifactKind::Screenshot && image_extensions(&self.mime).is_some())
    }
}

/// An artifact to store: its bytes and the fields the agent chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewArtifact {
    pub name: String,
    pub kind: ArtifactKind,
    pub author_role: AuthorRole,
    pub caption: Option<String>,
    pub bytes: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_are_camel_case() {
        let a = Artifact {
            id: "x".into(),
            conversation_id: "c".into(),
            task: "fix-login".into(),
            name: "shot.png".into(),
            kind: ArtifactKind::Screenshot,
            mime: "image/png".into(),
            size: 3,
            author_role: AuthorRole::Tester,
            caption: None,
            created_at: "2026-11-04T10:42:05.000Z".into(),
        };
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["conversationId"], "c");
        assert_eq!(v["kind"], "screenshot");
        assert_eq!(v["authorRole"], "tester");
        assert_eq!(v["caption"], serde_json::Value::Null);
        assert_eq!(serde_json::from_value::<Artifact>(v).unwrap(), a);
    }
}
