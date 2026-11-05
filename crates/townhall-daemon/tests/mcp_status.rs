//! `task_status` over the MCP endpoint: this conversation's tasks with their latest entries,
//! one task with its steps and, once ended, its agent's last message from a synthetic
//! transcript in a temp projects folder (never `~/.claude`).

use std::path::Path;
use std::sync::OnceLock;

use serde_json::{json, Value};
use tempfile::TempDir;
use townhall_core::model::{
    AgentEvent, ArtifactKind, AuthorRole, EntryBody, NewArtifact, NewTask, Task, TaskState,
};
use townhall_core::steps::{project_dir_for, CLAUDE_PROJECTS_ENV};
use townhall_core::{mcp, worktree};

mod common;
mod mcp_support;
use common::unique;
use mcp_support::{task, Mcp};

static PROJECTS: OnceLock<TempDir> = OnceLock::new();

/// Every test in this binary reads transcripts from one temp folder.
fn projects() -> &'static Path {
    PROJECTS
        .get_or_init(|| {
            let dir = TempDir::new().unwrap();
            std::env::set_var(CLAUDE_PROJECTS_ENV, dir.path());
            dir
        })
        .path()
}

fn agent_line(agent: &str, event: AgentEvent, text: &str) -> EntryBody {
    EntryBody::Agent {
        agent: agent.into(),
        event,
        text: text.into(),
    }
}

/// `count` finished `Bash` calls, then a turn that ended saying `last`.
fn transcript(count: usize, last: &str) -> String {
    let mut lines: Vec<Value> = (0..count)
        .flat_map(|i| {
            let id = format!("t{i}");
            [
                json!({"type": "assistant", "timestamp": "2026-11-04T10:00:00.000Z",
                       "message": {"id": format!("m{i}"), "role": "assistant", "content": [
                           {"type": "tool_use", "id": id, "name": "Bash", "input": {"command": format!("step {i}")}}]}}),
                json!({"type": "user", "timestamp": "2026-11-04T10:00:01.000Z",
                       "message": {"role": "user", "content": [
                           {"type": "tool_result", "tool_use_id": id, "content": "ok"}]}}),
            ]
        })
        .collect();
    lines.push(
        json!({"type": "assistant", "timestamp": "2026-11-04T10:01:00.000Z",
                      "message": {"id": "end", "role": "assistant", "stop_reason": "end_turn",
                                  "content": [{"type": "text", "text": last}]}}),
    );
    let text: Vec<String> = lines.iter().map(Value::to_string).collect();
    text.join("\n") + "\n"
}

async fn set_state(m: &Mcp, name: &str, state: TaskState) {
    let change = move |t: Task| Ok(Task { state, ..t });
    m.store().update_task(name, change).await.unwrap();
}

#[tokio::test]
async fn without_a_name_every_task_shows_its_last_three_entries() {
    projects();
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let (first, second) = (unique("login"), unique("docs"));
    m.ok("create_task", task(&first)).await;
    let mut after = task(&second);
    after["after"] = json!([first]);
    m.ok("create_task", after).await;
    let lines = [
        agent_line(&first, AgentEvent::Sent, "one"),
        agent_line(&second, AgentEvent::Sent, "other"),
        EntryBody::NeedsYou {
            agent: first.clone(),
            question: "go? [y/n]".into(),
            choices: vec!["y".into(), "n".into()],
        },
        EntryBody::Townhall { text: "ok".into() },
        agent_line(&first, AgentEvent::Sent, "y"),
        agent_line(&first, AgentEvent::Ended, "ended"),
    ];
    for body in lines {
        m.store().append_entry(&m.id, body).await.unwrap();
    }

    let all = m.ok("task_status", json!({})).await;
    let tasks = all["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2, "{all}");
    let one = &tasks[0];
    assert_eq!(one["name"], first.as_str());
    assert_eq!(one["title"], format!("do {first}"));
    assert_eq!(
        (&one["repo"], &one["state"], &one["after"]),
        (&json!("shop"), &json!("queued"), &json!([]))
    );
    assert_eq!(one["branch"], format!("townhall/{first}"));
    let kinds: Vec<(&str, &str)> = one["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["kind"].as_str().unwrap(),
                e["text"].as_str().unwrap_or("?"),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [("needsYou", "?"), ("agent", "y"), ("agent", "ended")]
    );
    assert_eq!(tasks[1]["after"], json!([first]));
    assert_eq!(tasks[1]["entries"][0]["text"], "other");
    assert_eq!(tasks[1]["entries"].as_array().unwrap().len(), 1);
    assert!(one.get("steps").is_none(), "steps only for a named task");
}

#[tokio::test]
async fn a_named_task_shows_its_last_steps_and_once_ended_its_last_message() {
    let projects = projects();
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let name = unique("fix");
    m.ok("create_task", task(&name)).await;
    let ws = m.state.ctx().startup().clone();
    let wt = worktree::create(&ws, &name, "main").await.unwrap();
    let dir = project_dir_for(projects, &wt.path);
    std::fs::create_dir_all(&dir).unwrap();
    let said = format!("Done: fixed the login. {}", "x".repeat(2500));
    std::fs::write(dir.join("s.jsonl"), transcript(23, &said)).unwrap();

    let queued = m
        .ok("task_status", json!({"name": name.to_uppercase()}))
        .await;
    assert_eq!(queued["name"], name.as_str());
    let steps = queued["steps"].as_array().unwrap();
    assert_eq!(steps.len(), mcp::MAX_STEPS, "{queued}");
    assert_eq!(
        (&steps[0]["id"], &steps[19]["id"]),
        (&json!("t3"), &json!("t22"))
    );
    assert_eq!(steps[19]["result"], "ok");
    assert_eq!(queued["lastMessage"], Value::Null, "not ended yet");

    set_state(&m, &name, TaskState::Ended).await;
    let ended = m.ok("task_status", json!({"name": name})).await;
    let last = ended["lastMessage"].as_str().unwrap();
    assert!(last.starts_with("Done: fixed the login. xxx"), "{last}");
    assert_eq!(last.chars().count(), mcp::MAX_MESSAGE_CHARS);
    assert!(last.ends_with('\u{2026}'));
    set_state(&m, &name, TaskState::Merged).await;
    let merged = m.ok("task_status", json!({"name": name})).await;
    assert_eq!(merged["lastMessage"], ended["lastMessage"]);
    worktree::remove(&ws, &name).await.unwrap();
}

#[tokio::test]
async fn another_conversations_task_or_an_unknown_one_is_a_tool_error() {
    projects();
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let mine = unique("mine");
    m.ok("create_task", task(&mine)).await;
    let other = m.store().create_conversation().await.unwrap().id;
    let theirs = unique("theirs");
    let new = NewTask {
        name: theirs.clone(),
        title: "not yours".into(),
        prompt: "p".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "builder".into(),
    };
    m.store().create_task(&other, new).await.unwrap();

    let text = m.refused("task_status", json!({"name": theirs})).await;
    assert!(text.contains("another conversation"), "{text}");
    assert!(
        text.contains(&mine) && !text.contains("not yours"),
        "{text}"
    );
    let text = m.refused("task_status", json!({"name": "nope"})).await;
    assert!(text.contains("no task named `nope`"), "{text}");
    let text = m.refused("task_status", json!({"task": mine})).await;
    assert!(text.contains("invalid arguments"), "{text}");
    let all = m.ok("task_status", Value::Null).await;
    let names: Vec<&str> = all["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, [mine.as_str()]);
}

#[tokio::test]
async fn every_task_lists_the_names_roles_and_urls_of_its_proof() {
    projects();
    let m = Mcp::new().await;
    m.note_repo("shop").await;
    let (with, without) = (unique("proved"), unique("bare"));
    m.ok("create_task", task(&with)).await;
    m.ok("create_task", task(&without)).await;
    let add = |name: &str, kind: ArtifactKind, role: AuthorRole| NewArtifact {
        name: name.into(),
        kind,
        author_role: role,
        caption: Some("long caption the planner does not need".into()),
        bytes: b"x".to_vec(),
    };
    let log = add("run.log", ArtifactKind::Log, AuthorRole::Tester);
    let note = add("notes.md", ArtifactKind::Note, AuthorRole::Coder);
    let first = m.store().add_artifact(&with, log).await.unwrap();
    let second = m.store().add_artifact(&with, note).await.unwrap();
    let base = m.state.ctx().base_url().to_string();
    let url = |id: &str| format!("{base}/api/tasks/{with}/artifacts/{id}");
    let expected = json!([
        {"name": "run.log", "kind": "log", "authorRole": "tester", "url": url(&first.id)},
        {"name": "notes.md", "kind": "note", "authorRole": "coder", "url": url(&second.id)},
    ]);

    let all = m.ok("task_status", json!({})).await;
    let tasks = all["tasks"].as_array().unwrap();
    assert_eq!(tasks[0]["artifacts"], expected, "{all}");
    assert_eq!(tasks[1]["artifacts"], json!([]));
    let one = m.ok("task_status", json!({"name": with})).await;
    assert_eq!(one["artifacts"], expected, "{one}");
}
