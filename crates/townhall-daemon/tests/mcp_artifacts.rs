//! `add_artifact` and `list_artifacts` on a task agent's MCP endpoint: scoped to the agent's
//! own conversation, append only, with the size and count limits and safe names.

use serde_json::{json, Value};
use townhall_core::model::{NewTask, MAX_ARTIFACTS_PER_TASK, MAX_ARTIFACT_BYTES};

mod common;
mod mcp_support;
use common::call;
use mcp_support::Mcp;

const PNG_BASE64: &str = "iVBORw0KGgpyZXN0";

async fn make_task(m: &Mcp, conversation: &str, name: &str) {
    let new = NewTask {
        name: name.into(),
        title: "fix".into(),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "builder".into(),
    };
    m.store().create_task(conversation, new).await.unwrap();
}

async fn agent_tool(m: &Mcp, task: &str, name: &str, arguments: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                      "params": {"name": name, "arguments": arguments}});
    let (_, answer) = call(
        &m.app,
        "POST",
        &format!("/api/tasks/{task}/mcp"),
        Some(body),
    )
    .await;
    answer["result"].clone()
}

fn payload(result: &Value) -> Value {
    assert!(result.get("isError").is_none(), "{result}");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn refusal(result: &Value) -> String {
    assert_eq!(result["isError"], true, "{result}");
    result["content"][0]["text"].as_str().unwrap().to_string()
}

fn note(task: &str, name: &str, content: &str) -> Value {
    json!({"task": task, "name": name, "kind": "note", "content": content, "author_role": "coder"})
}

async fn two_tasks() -> Mcp {
    let m = Mcp::new().await;
    make_task(&m, &m.id, "build-it").await;
    make_task(&m, &m.id, "test-it").await;
    m
}

#[tokio::test]
async fn a_task_agent_gets_the_task_tools_with_the_artifact_tools() {
    let m = two_tasks().await;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let (_, answer) = call(&m.app, "POST", "/api/tasks/build-it/mcp", Some(body)).await;
    let names: Vec<&str> = answer["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["hand_back", "open_pr", "add_artifact", "list_artifacts"]
    );
    let allowed = townhall_core::mcp::task_allowed_tools();
    assert!(allowed.contains(&"mcp__townhall__add_artifact".to_string()));
    assert!(allowed.contains(&"mcp__townhall__list_artifacts".to_string()));
}

#[tokio::test]
async fn an_added_artifact_is_listed_with_its_url() {
    let m = two_tasks().await;
    let mut args = note("build-it", "proof.md", "all green");
    args["caption"] = json!("the suite");
    args["author_role"] = json!("tester");
    let added = payload(&agent_tool(&m, "build-it", "add_artifact", args).await);
    let id = added["id"].as_str().unwrap();
    let base = m.state.ctx().base_url().to_string();
    assert_eq!(
        added["url"],
        format!("{base}/api/tasks/build-it/artifacts/{id}")
    );
    let listed = payload(
        &agent_tool(
            &m,
            "build-it",
            "list_artifacts",
            json!({"task": "build-it"}),
        )
        .await,
    );
    let rows = listed["artifacts"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], id);
    assert_eq!(rows[0]["name"], "proof.md");
    assert_eq!(rows[0]["kind"], "note");
    assert_eq!(rows[0]["author_role"], "tester");
    assert_eq!(rows[0]["caption"], "the suite");
    assert_eq!(rows[0]["url"], added["url"]);
    assert!(rows[0]["created"].as_str().unwrap().starts_with("20"));
}

#[tokio::test]
async fn base64_carries_a_screenshot_and_a_bad_one_is_refused() {
    let m = two_tasks().await;
    let shot = json!({"task": "build-it", "name": "login", "kind": "screenshot",
                      "content": PNG_BASE64, "encoding": "base64", "author_role": "tester"});
    let added = payload(&agent_tool(&m, "build-it", "add_artifact", shot.clone()).await);
    assert_eq!(added["name"], "login.png");
    assert_eq!(added["mime"], "image/png");
    let mut broken = shot.clone();
    broken["content"] = json!("not base64 !!");
    let text = refusal(&agent_tool(&m, "build-it", "add_artifact", broken).await);
    assert!(text.contains("base64"), "{text}");
    let mut text_shot = shot;
    text_shot["encoding"] = json!("text");
    text_shot["content"] = json!("<svg onload=alert(1)>");
    let text = refusal(&agent_tool(&m, "build-it", "add_artifact", text_shot).await);
    assert!(text.contains("image"), "{text}");
}

#[tokio::test]
async fn an_agent_adds_only_to_its_own_task_but_reads_its_siblings() {
    let m = two_tasks().await;
    let text =
        refusal(&agent_tool(&m, "test-it", "add_artifact", note("build-it", "a.md", "x")).await);
    assert!(text.contains("your own task `test-it`"), "{text}");
    assert!(m.store().artifacts("build-it").await.unwrap().is_empty());
    payload(
        &agent_tool(
            &m,
            "build-it",
            "add_artifact",
            note("build-it", "a.md", "x"),
        )
        .await,
    );
    let seen =
        payload(&agent_tool(&m, "test-it", "list_artifacts", json!({"task": "build-it"})).await);
    assert_eq!(seen["artifacts"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn another_conversation_is_out_of_reach() {
    let m = two_tasks().await;
    let other = m.store().create_conversation().await.unwrap().id;
    make_task(&m, &other, "elsewhere").await;
    m.store()
        .add_artifact(
            "elsewhere",
            townhall_core::model::NewArtifact {
                name: "secret.md".into(),
                kind: townhall_core::model::ArtifactKind::Note,
                author_role: townhall_core::model::AuthorRole::Coder,
                caption: None,
                bytes: b"hidden".to_vec(),
            },
        )
        .await
        .unwrap();
    let read = refusal(
        &agent_tool(
            &m,
            "build-it",
            "list_artifacts",
            json!({"task": "elsewhere"}),
        )
        .await,
    );
    assert!(
        read.contains("no task named `elsewhere` in this conversation"),
        "{read}"
    );
    assert!(read.contains("build-it"), "{read}");
    let write = refusal(
        &agent_tool(
            &m,
            "build-it",
            "add_artifact",
            note("elsewhere", "a.md", "x"),
        )
        .await,
    );
    assert!(write.contains("your own task `build-it`"), "{write}");
    let back = refusal(
        &agent_tool(
            &m,
            "elsewhere",
            "list_artifacts",
            json!({"task": "build-it"}),
        )
        .await,
    );
    assert!(
        back.contains("no task named `build-it` in this conversation"),
        "{back}"
    );
    let ghost =
        refusal(&agent_tool(&m, "build-it", "list_artifacts", json!({"task": "ghost"})).await);
    assert!(
        ghost.contains("no task named `ghost` in this conversation"),
        "{ghost}"
    );
}

#[tokio::test]
async fn names_that_climb_out_are_cut_to_a_file_name() {
    let m = two_tasks().await;
    let added = payload(
        &agent_tool(
            &m,
            "build-it",
            "add_artifact",
            note("build-it", "../../../../etc/cron.d/x", "y"),
        )
        .await,
    );
    assert_eq!(added["name"], "x");
    let (_, path) = m
        .store()
        .artifact("build-it", added["id"].as_str().unwrap())
        .await
        .unwrap();
    assert!(
        path.starts_with(m.env.home.root().join("artifacts")),
        "{path:?}"
    );
    let again =
        payload(&agent_tool(&m, "build-it", "add_artifact", note("build-it", "x", "z")).await);
    assert_ne!(again["id"], added["id"]);
    assert_eq!(m.store().artifacts("build-it").await.unwrap().len(), 2);
}

#[tokio::test]
async fn the_limits_are_told_to_the_agent() {
    let m = two_tasks().await;
    let big = "a".repeat(usize::try_from(MAX_ARTIFACT_BYTES).unwrap() + 1);
    let text = refusal(
        &agent_tool(
            &m,
            "build-it",
            "add_artifact",
            note("build-it", "big.md", &big),
        )
        .await,
    );
    assert!(text.contains("at most"), "{text}");
    let empty =
        refusal(&agent_tool(&m, "build-it", "add_artifact", note("build-it", "e.md", "")).await);
    assert!(!empty.is_empty());
    let bad_kind = json!({"task": "build-it", "name": "a", "kind": "video", "content": "x", "author_role": "coder"});
    let text = refusal(&agent_tool(&m, "build-it", "add_artifact", bad_kind).await);
    assert!(text.contains("invalid arguments"), "{text}");
    let extra = json!({"task": "build-it", "name": "a", "kind": "note", "content": "x", "author_role": "coder", "overwrite": true});
    refusal(&agent_tool(&m, "build-it", "add_artifact", extra).await);
    for i in 0..MAX_ARTIFACTS_PER_TASK {
        payload(
            &agent_tool(
                &m,
                "build-it",
                "add_artifact",
                note("build-it", &format!("n{i}.md"), "x"),
            )
            .await,
        );
    }
    let full = refusal(
        &agent_tool(
            &m,
            "build-it",
            "add_artifact",
            note("build-it", "late.md", "x"),
        )
        .await,
    );
    assert!(full.contains("50"), "{full}");
}

fn base64_of(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len() / 3 * 4 + 4);
    for chunk in bytes.as_chunks::<3>().0 {
        let n = u32::from(chunk[0]) << 16 | u32::from(chunk[1]) << 8 | u32::from(chunk[2]);
        for shift in [18, 12, 6, 0] {
            out.push(char::from(ALPHABET[(n >> shift & 63) as usize]));
        }
    }
    out
}

fn big_png() -> Vec<u8> {
    let mut bytes = vec![0u8; usize::try_from(MAX_ARTIFACT_BYTES).unwrap() / 3 * 3];
    bytes[..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    bytes
}

fn screenshot(content: &str) -> Value {
    json!({"task": "build-it", "name": "big", "kind": "screenshot",
           "content": content, "encoding": "base64", "author_role": "tester"})
}

#[tokio::test]
async fn a_screenshot_just_under_the_cap_is_accepted() {
    let m = two_tasks().await;
    let packed = base64_of(&big_png());
    let added = payload(&agent_tool(&m, "build-it", "add_artifact", screenshot(&packed)).await);
    assert_eq!(added["mime"], "image/png");
    assert_eq!(added["size"], big_png().len());
}

#[tokio::test]
async fn a_wrapped_screenshot_just_under_the_cap_is_accepted() {
    let m = two_tasks().await;
    let packed = base64_of(&big_png());
    let wrapped: Vec<&str> = packed
        .as_bytes()
        .chunks(76)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect();
    let wrapped = wrapped.join("\n");
    assert!(wrapped.len() > usize::try_from(MAX_ARTIFACT_BYTES).unwrap() / 3 * 4 + 8);
    let added = payload(&agent_tool(&m, "build-it", "add_artifact", screenshot(&wrapped)).await);
    assert_eq!(added["size"], big_png().len());
}

#[tokio::test]
async fn a_text_log_full_of_escapes_gets_the_tools_answer_not_a_bare_400() {
    let m = two_tasks().await;
    let unit = "line \"quoted\"\t\u{1b}[31mred\u{1b}[0m\r\n";
    let cap = usize::try_from(MAX_ARTIFACT_BYTES).unwrap();
    let log = unit.repeat(cap / unit.len());
    assert!(log.len() <= cap);
    let added = payload(
        &agent_tool(
            &m,
            "build-it",
            "add_artifact",
            note("build-it", "run.log", &log),
        )
        .await,
    );
    assert_eq!(added["size"], log.len());
    let over = unit.repeat(cap / unit.len() + 2);
    assert!(over.len() > cap);
    let text = refusal(
        &agent_tool(
            &m,
            "build-it",
            "add_artifact",
            note("build-it", "over.log", &over),
        )
        .await,
    );
    assert!(text.contains("at most 5120 KB"), "{text}");
}

#[tokio::test]
async fn agents_have_no_way_to_replace_or_delete() {
    let m = two_tasks().await;
    for name in ["delete_artifact", "update_artifact", "remove_artifact"] {
        let text = refusal(&agent_tool(&m, "build-it", name, json!({})).await);
        assert!(text.contains("unknown tool"), "{text}");
    }
}
