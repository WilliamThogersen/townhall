use serde_json::json;
use townhall_core::model::{Conversation, Repo, Task, TaskState};

pub fn conversation(id: &str, title: &str, updated: &str) -> Conversation {
    crate::state::tests::conversation(id, title, updated)
}

pub fn task(name: &str, state: TaskState) -> Task {
    let state = serde_json::to_value(state).unwrap();
    serde_json::from_value(json!({
        "name": name,
        "title": format!("title of {name}"),
        "prompt": "p",
        "repo": "shop",
        "baseBranch": "main",
        "after": [],
        "state": state,
        "conversationId": "c",
        "attachments": [],
        "createdAt": "2026-11-12T09:00:00.000Z",
        "pr": null,
        "agent": "Coder",
        "blocked": null,
        "prBase": null
    }))
    .unwrap()
}

pub fn repo(name: &str, base_branch: &str) -> Repo {
    serde_json::from_value(json!({
        "name": name,
        "path": format!("/x/{name}"),
        "baseBranch": base_branch
    }))
    .unwrap()
}
