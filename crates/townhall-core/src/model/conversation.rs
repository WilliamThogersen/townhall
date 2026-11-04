use serde::{Deserialize, Serialize};

use super::{Attachment, Task, ThreadEntry};

pub const NEW_CONVERSATION_TITLE: &str = "new conversation";

/// `thinking` while a turn of townhall's session runs, `yourTurn` when idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversationStatus {
    Thinking,
    #[default]
    YourTurn,
}

/// Townhall's own context window use, for the session panel's meter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsage {
    pub used: u64,
    pub limit: u64,
}

/// `~/.townhall/conversations/<id>/meta.json`, Hive's `Conversation`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    /// `claude --session-id` on the first turn, `--resume` after.
    pub session_id: String,
    /// False until the first turn went through, so that turn uses `--session-id`.
    #[serde(default)]
    pub session_started: bool,
    pub title: String,
    pub status: ConversationStatus,
    /// Names of the tasks it created, oldest first.
    pub tasks: Vec<String>,
    /// Repo name set by hand from the row menu; `None` is auto.
    pub linked_repo: Option<String>,
    /// Under older. Sending a message continues it and clears this.
    pub ended: bool,
    #[serde(default)]
    pub context: Option<ContextUsage>,
    pub created_at: String,
    pub updated_at: String,
}

/// `GET /api/conversations/{id}`: everything the conversation screen needs in one call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDetail {
    pub conversation: Conversation,
    pub entries: Vec<ThreadEntry>,
    pub tasks: Vec<Task>,
    pub attachments: Vec<Attachment>,
}

/// `POST /api/conversations` and `POST /api/conversations/{id}/messages` body.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewMessage {
    pub text: String,
    /// Ids from `POST /api/conversations/{id}/attachments`.
    #[serde(default)]
    pub attachments: Vec<String>,
}

/// `PATCH /api/conversations/{id}` body. `linkedRepo: null` sets the repo back to auto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationPatch {
    #[serde(default, deserialize_with = "super::present")]
    pub linked_repo: Option<Option<String>>,
    #[serde(default)]
    pub title: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Conversation {
        Conversation {
            id: "a1b2c3d4e5f6".into(),
            session_id: "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b".into(),
            session_started: false,
            title: NEW_CONVERSATION_TITLE.into(),
            status: ConversationStatus::YourTurn,
            tasks: vec![],
            linked_repo: None,
            ended: false,
            context: None,
            created_at: "2026-11-04T10:42:05.000Z".into(),
            updated_at: "2026-11-04T10:42:05.000Z".into(),
        }
    }

    #[test]
    fn conversation_shape() {
        let v = serde_json::to_value(sample()).unwrap();
        assert_eq!(
            v,
            json!({
                "id": "a1b2c3d4e5f6",
                "sessionId": "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b",
                "sessionStarted": false,
                "title": "new conversation",
                "status": "yourTurn",
                "tasks": [],
                "linkedRepo": null,
                "ended": false,
                "context": null,
                "createdAt": "2026-11-04T10:42:05.000Z",
                "updatedAt": "2026-11-04T10:42:05.000Z"
            })
        );
        assert_eq!(serde_json::from_value::<Conversation>(v).unwrap(), sample());
    }

    #[test]
    fn patch_tells_null_from_missing() {
        let set: ConversationPatch = serde_json::from_value(json!({"linkedRepo": "shop"})).unwrap();
        assert_eq!(set.linked_repo, Some(Some("shop".into())));
        let auto: ConversationPatch = serde_json::from_value(json!({"linkedRepo": null})).unwrap();
        assert_eq!(auto.linked_repo, Some(None));
        let none: ConversationPatch = serde_json::from_value(json!({"title": "x"})).unwrap();
        assert_eq!(none.linked_repo, None);
    }

    #[test]
    fn message_defaults_attachments() {
        let m: NewMessage = serde_json::from_value(json!({"text": "hi"})).unwrap();
        assert!(m.attachments.is_empty());
        assert!(serde_json::from_value::<NewMessage>(json!({"text": "hi", "repo": "x"})).is_err());
    }
}
