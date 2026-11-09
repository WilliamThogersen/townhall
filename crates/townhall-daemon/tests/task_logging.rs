//! Task, git and store events (see docs/logging.md). The capture is per thread, so every
//! test runs on a current-thread runtime and emits from the awaiting thread.

use townhall_core::events::EventBus;
use townhall_core::model::{NewTask, TaskState};
use townhall_core::store::Store;
use townhall_core::Home;
use townhall_daemon::logging::test_capture;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let home = Home::at(dir.path().join(".townhall"));
    let store = Store::open(&home, EventBus::new()).unwrap();
    (dir, store)
}

fn new_task(name: &str, prompt: &str) -> NewTask {
    NewTask {
        name: name.into(),
        title: format!("do {name}"),
        prompt: prompt.into(),
        repo: "shop".into(),
        base_branch: "main".into(),
        after: vec![],
        attachments: vec![],
        agent: "builder".into(),
        pr_base: None,
        done: Default::default(),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn update_task_queued_to_running_emits_task_state() {
    let (_dir, store) = store();
    let c = store.create_conversation().await.unwrap();
    store.create_task(&c.id, new_task("t1", "x")).await.unwrap();
    let (capture, _guard) = test_capture();
    store
        .update_task("t1", |t| {
            Ok(townhall_core::model::Task {
                state: TaskState::Working,
                ..t
            })
        })
        .await
        .unwrap();
    let events = capture.events("task.state");
    assert_eq!(events.len(), 1, "{:?}", capture.lines());
    assert_eq!(events[0]["from"], "queued");
    assert_eq!(events[0]["to"], "working");
    assert_eq!(events[0]["task"], "t1");
    assert_eq!(events[0]["target"], "townhall::task");
}

#[tokio::test(flavor = "current_thread")]
async fn update_task_without_a_state_change_emits_nothing() {
    let (_dir, store) = store();
    let c = store.create_conversation().await.unwrap();
    store.create_task(&c.id, new_task("t1", "x")).await.unwrap();
    let (capture, _guard) = test_capture();
    store.update_task("t1", Ok).await.unwrap();
    assert!(capture.events("task.state").is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn create_task_logs_prompt_chars_and_never_the_prompt() {
    let (_dir, store) = store();
    let c = store.create_conversation().await.unwrap();
    let (capture, _guard) = test_capture();
    let prompt = "SECRET-PROMPT-TEXT please";
    store
        .create_task(&c.id, new_task("t1", prompt))
        .await
        .unwrap();
    let events = capture.events("task.created");
    assert_eq!(events.len(), 1, "{:?}", capture.lines());
    assert_eq!(events[0]["prompt_chars"], prompt.chars().count());
    assert_eq!(events[0]["repo"], "shop");
    assert_eq!(events[0]["agent_type"], "builder");
    let all = serde_json::to_string(&capture.lines()).unwrap();
    assert!(!all.contains("SECRET-PROMPT-TEXT"), "{all}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_failing_cmd_run_warns_with_exit_code_and_redacted_stderr() {
    let (capture, _guard) = test_capture();
    let err = townhall_core::cmd::run(
        "sh",
        &["-c", "echo 'bad token=abc123 here' >&2; exit 3"],
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(err.kind(), "command");
    let events = capture.events("cmd.run");
    assert_eq!(events.len(), 1, "{:?}", capture.lines());
    let e = &events[0];
    assert_eq!(e["level"], "WARN");
    assert_eq!(e["exit_code"], 3);
    assert_eq!(e["program"], "sh");
    assert!(e["duration_ms"].is_number());
    let stderr = e["stderr"].as_str().unwrap();
    assert!(
        stderr.contains("bad") && !stderr.contains("abc123"),
        "{stderr}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_succeeding_cmd_run_logs_at_debug() {
    let (capture, _guard) = test_capture();
    townhall_core::cmd::run("git", &["--version"], None)
        .await
        .unwrap();
    let events = capture.events("cmd.run");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["level"], "DEBUG");
    assert_eq!(events[0]["target"], "townhall::git");
    assert_eq!(events[0]["args"], "--version");
}

#[tokio::test(flavor = "current_thread")]
async fn a_failing_probe_logs_at_debug_with_exit_code_and_stderr() {
    let (capture, _guard) = test_capture();
    let err = townhall_core::cmd::probe("sh", &["-c", "echo no-such-ref >&2; exit 1"], None)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "command");
    let events = capture.events("cmd.run");
    assert_eq!(events.len(), 1, "{:?}", capture.lines());
    assert_eq!(events[0]["level"], "DEBUG");
    assert_eq!(events[0]["exit_code"], 1);
    assert_eq!(events[0]["stderr"], "no-such-ref");
}

#[test]
fn a_failing_blocking_probe_logs_at_debug() {
    let (capture, _guard) = test_capture();
    townhall_core::cmd::probe_blocking("sh", &["-c", "echo probe-blocking >&2; exit 2"], None)
        .unwrap_err();
    let events = capture.events("cmd.run");
    assert_eq!(events.len(), 1, "{:?}", capture.lines());
    assert_eq!(events[0]["level"], "DEBUG");
    assert_eq!(events[0]["exit_code"], 2);
}

#[tokio::test(flavor = "current_thread")]
async fn an_identical_failure_warns_once_not_on_every_repeat() {
    let (capture, _guard) = test_capture();
    for _ in 0..5 {
        townhall_core::cmd::run("sh", &["-c", "echo repeat-me >&2; exit 4"], None)
            .await
            .unwrap_err();
    }
    let warns: Vec<_> = capture
        .events("cmd.run")
        .into_iter()
        .filter(|e| e["level"] == "WARN")
        .collect();
    assert_eq!(warns.len(), 1, "{:?}", capture.lines());
}
