use serde::{Deserialize, Serialize};

use super::{AgentInfo, Conversation, Task, ThreadEntry};

/// One message on the `/api/events` websocket, tagged by `type`.
/// Store writes publish the first three; the agent watcher (2.5) publishes `agentUpdated`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ServerEvent {
    ConversationUpdated {
        conversation: Conversation,
    },
    EntryAppended {
        conversation_id: String,
        entry: ThreadEntry,
    },
    TaskUpdated {
        task: Task,
    },
    AgentUpdated {
        agent: AgentInfo,
    },
    /// Sent by the socket itself when it fell behind the bus: refetch everything.
    Resync,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentState, EntryBody};
    use serde_json::json;

    #[test]
    fn events_are_tagged_by_type() {
        let entry = ThreadEntry {
            id: 1,
            at: "2026-11-04T10:42:05.000Z".into(),
            body: EntryBody::Townhall { text: "ok".into() },
        };
        let e = ServerEvent::EntryAppended {
            conversation_id: "c1".into(),
            entry,
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(
            v,
            json!({
                "type": "entryAppended",
                "conversationId": "c1",
                "entry": {"id": 1, "at": "2026-11-04T10:42:05.000Z", "kind": "townhall", "text": "ok"}
            })
        );
        assert_eq!(serde_json::from_value::<ServerEvent>(v).unwrap(), e);
        assert_eq!(
            serde_json::to_value(ServerEvent::Resync).unwrap(),
            json!({"type": "resync"})
        );
        let agent = AgentInfo {
            name: "a".into(),
            repo: "r".into(),
            kind: None,
            branch: "townhall/a".into(),
            worktree: "/w".into(),
            alive: false,
            state: AgentState::Ended,
        };
        let v = serde_json::to_value(ServerEvent::AgentUpdated { agent }).unwrap();
        assert_eq!(v["type"], "agentUpdated");
        assert_eq!(v["agent"]["state"]["kind"], "ended");
    }
}
