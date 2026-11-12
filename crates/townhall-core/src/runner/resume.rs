use crate::model::{Conversation, EntryBody, ThreadEntry};

pub const RESUME_NOTE: &str = "townhall restarted mid-reply (automatic, the user did not write this): continue where you left off";

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Plan {
    Resume(u64),
    Restarted,
}

pub(super) fn plan(conversation: &Conversation, entries: &[ThreadEntry]) -> Plan {
    let trigger = entries.iter().rev().find(|e| {
        matches!(
            e.body,
            EntryBody::You { .. } | EntryBody::Plugin { .. } | EntryBody::Event { .. }
        )
    });
    match trigger {
        Some(entry) if conversation.session_started && !conversation.ended => {
            Plan::Resume(entry.id)
        }
        _ => Plan::Restarted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ConversationStatus;

    fn conversation(started: bool) -> Conversation {
        Conversation {
            id: "a1b2c3d4e5f6".into(),
            session_id: "0b8e7f8a-3c1d-4f7e-9a2b-1c2d3e4f5a6b".into(),
            session_started: started,
            title: "t".into(),
            status: ConversationStatus::Thinking,
            tasks: vec![],
            linked_repo: None,
            ended: false,
            context: None,
            created_at: "2026-11-04T10:42:05.000Z".into(),
            updated_at: "2026-11-04T10:42:05.000Z".into(),
        }
    }

    fn entry(id: u64, body: EntryBody) -> ThreadEntry {
        ThreadEntry {
            id,
            at: "2026-11-04T10:42:05.000Z".into(),
            body,
        }
    }

    fn you(id: u64) -> ThreadEntry {
        let body = EntryBody::You {
            text: "hi".into(),
            attachments: vec![],
            reply_to: None,
        };
        entry(id, body)
    }

    fn reply(id: u64) -> ThreadEntry {
        entry(id, EntryBody::Townhall { text: "ok".into() })
    }

    fn event(id: u64) -> ThreadEntry {
        entry(
            id,
            EntryBody::Event {
                text: "task a ended".into(),
            },
        )
    }

    #[test]
    fn a_started_session_resumes_the_turn_of_the_last_message() {
        let entries = [you(1), reply(2), you(3), reply(4)];
        assert_eq!(plan(&conversation(true), &entries), Plan::Resume(3));
    }

    #[test]
    fn a_task_event_is_a_turn_too() {
        let entries = [you(1), reply(2), event(3), event(4)];
        assert_eq!(plan(&conversation(true), &entries), Plan::Resume(4));
    }

    #[test]
    fn no_started_session_keeps_the_restart_message() {
        assert_eq!(plan(&conversation(false), &[you(1)]), Plan::Restarted);
    }

    #[test]
    fn a_thread_without_a_trigger_keeps_the_restart_message() {
        assert_eq!(plan(&conversation(true), &[reply(1)]), Plan::Restarted);
        assert_eq!(plan(&conversation(true), &[]), Plan::Restarted);
    }

    #[test]
    fn an_ended_conversation_is_not_resumed() {
        let ended = Conversation {
            ended: true,
            ..conversation(true)
        };
        assert_eq!(plan(&ended, &[you(1)]), Plan::Restarted);
    }
}
