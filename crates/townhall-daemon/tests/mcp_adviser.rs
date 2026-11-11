use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use townhall_core::model::{NewTask, Task, TaskState};
use townhall_core::ContextOptions;
use townhall_daemon::router;

mod common;
mod mcp_support;
use common::{call, Env};
use mcp_support::Mcp;

const SCRIPT: &str = r#"#!/bin/sh
dir='@DIR@'
n=$(( $(cat "$dir/count" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$dir/count"
printf '%s\0' "$@" > "$dir/argv-$n"
pwd -P > "$dir/cwd-$n"
IFS= read -r first || exit 4
printf '%s\n' "$first" > "$dir/stdin-$n"
echo '{"type":"system","subtype":"init","session_id":"fake","tools":[]}'
case "$first" in
  *BOOM*)
    echo '{"type":"result","subtype":"error_during_execution","is_error":true,"result":"overloaded"}'
    exit 1 ;;
  *USER*)
    echo '{"type":"result","subtype":"success","is_error":false,"result":"NEEDS THE USER: which brand colour?","usage":{"input_tokens":100,"output_tokens":9}}'
    cat >> "$dir/stdin-$n"
    exit 0 ;;
esac
echo '{"type":"result","subtype":"success","is_error":false,"result":"use the second option","usage":{"input_tokens":1000,"output_tokens":34}}'
cat >> "$dir/stdin-$n"
"#;

struct Fake {
    dir: PathBuf,
}

impl Fake {
    fn install(parent: &Path) -> Self {
        let dir = parent.join("fake-claude");
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let script = dir.join("claude");
        std::fs::write(&script, SCRIPT.replace("@DIR@", &dir.to_string_lossy())).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir }
    }

    fn runs(&self) -> usize {
        std::fs::read_to_string(self.dir.join("count"))
            .map(|s| s.trim().parse().unwrap())
            .unwrap_or(0)
    }

    fn argv(&self, n: usize) -> Vec<String> {
        let text = std::fs::read_to_string(self.dir.join(format!("argv-{n}"))).unwrap();
        text.split_terminator('\0').map(String::from).collect()
    }

    fn cwd(&self, n: usize) -> PathBuf {
        let text = std::fs::read_to_string(self.dir.join(format!("cwd-{n}"))).unwrap();
        PathBuf::from(text.trim())
    }

    fn asked(&self, n: usize) -> String {
        std::fs::read_to_string(self.dir.join(format!("stdin-{n}"))).unwrap()
    }
}

fn value_of(argv: &[String], flag: &str) -> Option<String> {
    let prefix = format!("{flag}=");
    argv.iter()
        .find_map(|a| a.strip_prefix(&prefix))
        .map(String::from)
}

async fn with_fake() -> (Mcp, Fake) {
    let env = Env::new("shop").await;
    let fake = Fake::install(env.root.path());
    let options = ContextOptions {
        claude_bin: fake.dir.join("claude").to_string_lossy().into_owned(),
        ..ContextOptions::default()
    };
    let state = env.state_with(options);
    let app = router(state.clone());
    state.ctx().repos().await.unwrap();
    let id = state
        .ctx()
        .store()
        .unwrap()
        .create_conversation()
        .await
        .unwrap()
        .id;
    (
        Mcp {
            env,
            state,
            app,
            id,
        },
        fake,
    )
}

async fn running(m: &Mcp, name: &str, agent: &str) -> PathBuf {
    let new = NewTask {
        name: name.into(),
        title: "fix".into(),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: agent.into(),
        pr_base: None,
        done: Default::default(),
    };
    m.store().create_task(&m.id, new).await.unwrap();
    let change = |t: Task| {
        Ok(Task {
            state: TaskState::Working,
            ..t
        })
    };
    m.store().update_task(name, change).await.unwrap();
    let ws = m.env.workspace(&m.env.startup);
    let path = townhall_core::worktree::create(&ws, name, "main")
        .await
        .unwrap()
        .path;
    path.canonicalize().unwrap()
}

async fn task_tool(m: &Mcp, task: &str, arguments: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                      "params": {"name": "ask_adviser", "arguments": arguments}});
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

#[tokio::test]
async fn both_endpoints_list_ask_adviser_and_grant_it() {
    let (m, _fake) = with_fake().await;
    running(&m, "build-it", "builder").await;
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let (_, answer) = call(
        &m.app,
        "POST",
        "/api/tasks/build-it/mcp",
        Some(body.clone()),
    )
    .await;
    let names: Vec<&str> = answer["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"ask_adviser"), "{names:?}");
    let listed = m.rpc(body).await;
    let conversation: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(conversation.last(), Some(&"ask_adviser"));
    let tool = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "ask_adviser")
        .unwrap();
    assert_eq!(tool["inputSchema"]["required"], json!(["question"]));
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert!(townhall_core::mcp::task_allowed_tools()
        .contains(&"mcp__townhall__ask_adviser".to_string()));
    assert!(townhall_core::mcp::allowed_tools().contains(&"mcp__townhall__ask_adviser".to_string()));
}

#[tokio::test]
async fn a_task_agent_gets_the_answer_of_the_advisers_model_read_only_in_its_worktree() {
    let (m, fake) = with_fake().await;
    let worktree = running(&m, "build-it", "builder").await;
    let got = payload(
        &task_tool(
            &m,
            "build-it",
            json!({"question": "which approach?", "context": "two designs, A and B"}),
        )
        .await,
    );
    assert_eq!(got["answer"], "use the second option");
    assert_eq!(got["needsUser"], false);
    assert_eq!(fake.runs(), 1);
    let argv = fake.argv(1);
    assert_eq!(value_of(&argv, "--model").as_deref(), Some("fable"));
    assert_eq!(value_of(&argv, "--effort").as_deref(), Some("high"));
    assert_eq!(
        value_of(&argv, "--permission-mode").as_deref(),
        Some("plan")
    );
    assert_eq!(
        value_of(&argv, "--tools").as_deref(),
        Some("Read,Grep,Glob,WebSearch,WebFetch")
    );
    assert!(value_of(&argv, "--append-system-prompt")
        .unwrap()
        .starts_with("You are the adviser"));
    assert!(value_of(&argv, "--disallowedTools")
        .unwrap()
        .split(',')
        .all(|t| ["Bash", "Edit", "MultiEdit", "Write", "NotebookEdit"].contains(&t)));
    assert_eq!(fake.cwd(1), worktree);
    let sent = fake.asked(1);
    assert!(sent.contains("which approach?") && sent.contains("two designs, A and B"));
}

#[tokio::test]
async fn the_call_is_recorded_as_a_run_of_the_adviser_with_its_tokens() {
    let (m, _fake) = with_fake().await;
    running(&m, "build-it", "builder").await;
    task_tool(&m, "build-it", json!({"question": "which approach?"})).await;
    let runs = m.store().agent_runs("adviser", 10).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].kind, "consult");
    assert_eq!(runs[0].task, "build-it");
    assert_eq!(
        (
            runs[0].state.as_str(),
            runs[0].tokens,
            runs[0].model.as_deref()
        ),
        ("done", 1034, Some("fable"))
    );
    let usage = m.store().agent_usage().await.unwrap();
    let adviser = usage.iter().find(|u| u.agent == "adviser").unwrap();
    assert_eq!((adviser.runs, adviser.tokens), (1, 1034));
    let (question, answer): (String, String) = m
        .store()
        .call(|conn| {
            Ok(
                conn.query_row("select question, answer from adviser_calls", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?,
            )
        })
        .await
        .unwrap();
    assert_eq!(question, "which approach?");
    assert_eq!(answer, "use the second option");
}

#[tokio::test]
async fn an_answer_that_needs_the_user_says_so() {
    let (m, _fake) = with_fake().await;
    running(&m, "build-it", "builder").await;
    let got = payload(&task_tool(&m, "build-it", json!({"question": "USER taste?"})).await);
    assert_eq!(got["needsUser"], true);
    assert!(got["answer"].as_str().unwrap().contains("brand colour"));
}

#[tokio::test]
async fn the_conversation_agent_asks_in_its_repo() {
    let (m, fake) = with_fake().await;
    m.note_repo("shop").await;
    let got = m
        .ok("ask_adviser", json!({"question": "split the task?"}))
        .await;
    assert_eq!(got["answer"], "use the second option");
    assert_eq!(fake.cwd(1), m.env.startup);
    let runs = m.store().agent_runs("adviser", 10).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].conversation_id, m.id);
}

#[tokio::test]
async fn an_edited_adviser_changes_how_it_runs() {
    let (m, fake) = with_fake().await;
    running(&m, "build-it", "builder").await;
    let own = "name = \"adviser\"\nadapter = \"claude\"\nmodel = \"haiku\"\neffort = \"low\"\ntools = [\"Read\"]\nprompt = \"be rude\"\n";
    m.store().save_agent_type(own).await.unwrap();
    task_tool(&m, "build-it", json!({"question": "q"})).await;
    let argv = fake.argv(1);
    assert_eq!(value_of(&argv, "--model").as_deref(), Some("haiku"));
    assert_eq!(value_of(&argv, "--effort").as_deref(), Some("low"));
    assert_eq!(value_of(&argv, "--tools").as_deref(), Some("Read"));
    assert_eq!(
        value_of(&argv, "--append-system-prompt").as_deref(),
        Some("be rude")
    );
}

#[tokio::test]
async fn without_an_adviser_type_it_fails_clearly_and_runs_nothing() {
    let (m, fake) = with_fake().await;
    running(&m, "build-it", "builder").await;
    m.store().delete_agent_type("adviser").await.unwrap();
    let text = refusal(&task_tool(&m, "build-it", json!({"question": "q"})).await);
    assert!(text.contains("no adviser type"), "{text}");
    assert!(text.contains("ask the user"), "{text}");
    assert_eq!(fake.runs(), 0);
}

#[tokio::test]
async fn a_failed_run_is_an_error_with_the_reason_and_is_still_recorded() {
    let (m, _fake) = with_fake().await;
    running(&m, "build-it", "builder").await;
    let text = refusal(&task_tool(&m, "build-it", json!({"question": "BOOM"})).await);
    assert!(text.contains("the adviser could not answer"), "{text}");
    assert!(text.contains("overloaded"), "{text}");
    let runs = m.store().agent_runs("adviser", 10).await.unwrap();
    assert_eq!((runs.len(), runs[0].state.as_str()), (1, "failed"));
}

#[tokio::test]
async fn bad_input_and_the_adviser_asking_itself_are_refused_before_anything_runs() {
    let (m, fake) = with_fake().await;
    running(&m, "build-it", "builder").await;
    running(&m, "think-it", "adviser").await;
    let blank = refusal(&task_tool(&m, "build-it", json!({"question": "  "})).await);
    assert!(blank.contains("question"), "{blank}");
    let long = "x".repeat(30_000);
    let too_long = refusal(&task_tool(&m, "build-it", json!({"question": long})).await);
    assert!(too_long.contains("longer than"), "{too_long}");
    let itself = refusal(&task_tool(&m, "think-it", json!({"question": "q"})).await);
    assert!(itself.contains("cannot ask itself"), "{itself}");
    assert_eq!(fake.runs(), 0);
}

#[tokio::test]
async fn a_task_whose_worktree_is_gone_says_so() {
    let (m, fake) = with_fake().await;
    let new = NewTask {
        name: "ghost".into(),
        title: "fix".into(),
        prompt: "the brief".into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "builder".into(),
        pr_base: None,
        done: Default::default(),
    };
    m.store().create_task(&m.id, new).await.unwrap();
    let text = refusal(&task_tool(&m, "ghost", json!({"question": "q"})).await);
    assert!(text.contains("worktree"), "{text}");
    assert_eq!(fake.runs(), 0);
}
