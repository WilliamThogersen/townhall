use serde::{Deserialize, Serialize};

/// One line of `events.jsonl`: an id (1, 2, 3 ... per conversation), a time and the body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEntry {
    pub id: u64,
    pub at: String,
    #[serde(flatten)]
    pub body: EntryBody,
}

/// How the repo was chosen. `named`, `code` and `only` come from repo inference (2.4),
/// `picked` is an answer to a repo ask, `linked` is the row menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepoRule {
    Named,
    Code,
    Only,
    Picked,
    Linked,
}

/// The one-line agent events: `→` sent, `✓` passed, `×` failed, `○` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentEvent {
    Sent,
    Passed,
    Failed,
    Ended,
    Allowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EntryBody {
    /// Your message, with the ids of its attachments and the entry it replies to.
    You {
        text: String,
        attachments: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reply_to: Option<u64>,
    },
    /// Townhall's reply, Markdown.
    Townhall { text: String },
    /// `~ repo shop · develop, it has src/search/query.ts`.
    Repo {
        repo: String,
        base_branch: String,
        rule: RepoRule,
        reason: String,
    },
    /// Townhall cannot tell the repo: one button per candidate, plus `both` / `all` when `offer_all`.
    RepoAsk {
        question: String,
        candidates: Vec<String>,
        offer_all: bool,
    },
    /// The tasks one reply created, by name.
    TasksCreated { tasks: Vec<String> },
    /// An agent waits for you: its question verbatim and direct answers.
    NeedsYou {
        agent: String,
        question: String,
        choices: Vec<String>,
    },
    Agent {
        agent: String,
        event: AgentEvent,
        text: String,
    },
    /// A system event for the planner, not a message from you: a task of the
    /// conversation ended, failed or asked something. It starts a planner turn.
    Event { text: String },
    /// An agent handed its task back to another agent type, with why.
    Handoff {
        task: String,
        from: String,
        to: String,
        reason: String,
    },
    Plugin { plugin: String, text: String },
    /// Townhall's own failure, for example its session exited.
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn round_trip(body: EntryBody, expected: Value) {
        let entry = ThreadEntry {
            id: 3,
            at: "2026-11-04T10:42:05.000Z".into(),
            body,
        };
        let v = serde_json::to_value(&entry).unwrap();
        let mut want = json!({"id": 3, "at": "2026-11-04T10:42:05.000Z"});
        want.as_object_mut()
            .unwrap()
            .extend(expected.as_object().unwrap().clone());
        assert_eq!(v, want);
        assert_eq!(serde_json::from_value::<ThreadEntry>(v).unwrap(), entry);
    }

    #[test]
    fn message_and_error_bodies_have_a_stable_shape() {
        round_trip(
            EntryBody::You {
                text: "hi".into(),
                attachments: vec!["a1".into()],
                reply_to: None,
            },
            json!({"kind": "you", "text": "hi", "attachments": ["a1"]}),
        );
        round_trip(
            EntryBody::You {
                text: "yes".into(),
                attachments: vec![],
                reply_to: Some(2),
            },
            json!({"kind": "you", "text": "yes", "attachments": [], "replyTo": 2}),
        );
        round_trip(
            EntryBody::Townhall { text: "ok".into() },
            json!({"kind": "townhall", "text": "ok"}),
        );
        round_trip(
            EntryBody::Event {
                text: "task fix ended".into(),
            },
            json!({"kind": "event", "text": "task fix ended"}),
        );
        round_trip(
            EntryBody::Plugin {
                plugin: "lint@acme".into(),
                text: "created by plugin lint@acme".into(),
            },
            json!({"kind": "plugin", "plugin": "lint@acme", "text": "created by plugin lint@acme"}),
        );
        round_trip(
            EntryBody::Error {
                message: "claude exited with 1".into(),
            },
            json!({"kind": "error", "message": "claude exited with 1"}),
        );
    }

    #[test]
    fn repo_bodies_have_a_stable_shape() {
        round_trip(
            EntryBody::Repo {
                repo: "shop".into(),
                base_branch: "develop".into(),
                rule: RepoRule::Code,
                reason: "it has src/search/query.ts".into(),
            },
            json!({"kind": "repo", "repo": "shop", "baseBranch": "develop", "rule": "code", "reason": "it has src/search/query.ts"}),
        );
        round_trip(
            EntryBody::RepoAsk {
                question: "which one?".into(),
                candidates: vec!["shop".into(), "admin".into()],
                offer_all: true,
            },
            json!({"kind": "repoAsk", "question": "which one?", "candidates": ["shop", "admin"], "offerAll": true}),
        );
    }

    #[test]
    fn task_and_agent_bodies_have_a_stable_shape() {
        round_trip(
            EntryBody::TasksCreated {
                tasks: vec!["fix-login".into()],
            },
            json!({"kind": "tasksCreated", "tasks": ["fix-login"]}),
        );
        round_trip(
            EntryBody::NeedsYou {
                agent: "fix-login".into(),
                question: "run migrations?".into(),
                choices: vec!["y".into(), "n".into()],
            },
            json!({"kind": "needsYou", "agent": "fix-login", "question": "run migrations?", "choices": ["y", "n"]}),
        );
        round_trip(
            EntryBody::Agent {
                agent: "fix-login".into(),
                event: AgentEvent::Sent,
                text: "y".into(),
            },
            json!({"kind": "agent", "agent": "fix-login", "event": "sent", "text": "y"}),
        );
        round_trip(
            EntryBody::Handoff {
                task: "fix-login".into(),
                from: "shipper".into(),
                to: "coder".into(),
                reason: "ci fails".into(),
            },
            json!({"kind": "handoff", "task": "fix-login", "from": "shipper", "to": "coder", "reason": "ci fails"}),
        );
    }
}
