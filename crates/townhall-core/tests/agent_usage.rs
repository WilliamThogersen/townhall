use townhall_core::model::{AdviserCall, AgentUsage};
use townhall_core::steps::{transcript_model, transcript_tokens};

mod common;
use common::{new_task, store_env};

#[test]
fn transcript_tokens_count_each_message_once() {
    let jsonl = [
        r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
        r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":10,"output_tokens":1}}}"#,
        r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":10,"output_tokens":5}}}"#,
        "not json",
        r#"{"type":"assistant","message":{"id":"m2","usage":{"input_tokens":20,"output_tokens":7,"cache_read_input_tokens":999}}}"#,
        r#"{"type":"assistant","message":{"id":"m3"}}"#,
    ]
    .join("\n");
    assert_eq!(transcript_tokens(&jsonl), 10 + 5 + 20 + 7);
    assert_eq!(transcript_tokens(""), 0);
}

#[tokio::test]
async fn usage_sums_runs_and_tokens_per_agent_type() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let coder = townhall_core::model::NewTask {
        agent: "coder".into(),
        ..new_task("b")
    };
    for task in [
        new_task("a"),
        coder,
        townhall_core::model::NewTask {
            agent: "coder".into(),
            ..new_task("c")
        },
    ] {
        env.store.create_task(&c.id, task).await.unwrap();
    }
    env.store.set_task_tokens("A", 100).await.unwrap();
    env.store.set_task_tokens("b", 40).await.unwrap();
    env.store.set_task_tokens("b", 50).await.unwrap();
    let usage = env.store.agent_usage().await.unwrap();
    assert_eq!(
        usage,
        vec![
            AgentUsage {
                agent: "builder".into(),
                runs: 1,
                tokens: 100
            },
            AgentUsage {
                agent: "coder".into(),
                runs: 2,
                tokens: 50
            },
        ]
    );
    assert!(env.store.set_task_tokens("nope", 1).await.is_err());
}

#[test]
fn transcript_model_is_the_last_real_model() {
    let jsonl = [
        r#"{"type":"assistant","message":{"id":"m1","model":"claude-a"}}"#,
        "not json",
        r#"{"type":"assistant","message":{"id":"m2","model":"claude-b"}}"#,
        r#"{"type":"assistant","message":{"id":"m3"}}"#,
        r#"{"type":"assistant","message":{"id":"m4","model":"<synthetic>"}}"#,
        r#"{"type":"assistant","message":{"id":"m5","model":""}}"#,
    ]
    .join("\n");
    assert_eq!(transcript_model(&jsonl), Some("claude-b".to_string()));
    assert_eq!(transcript_model(""), None);
    assert_eq!(
        transcript_model(r#"{"message":{"model":"<synthetic>"}}"#),
        None
    );
}

#[tokio::test]
async fn usage_for_agent_reads_the_worktrees_newest_transcript() {
    use std::fs;
    use townhall_core::steps::{project_dir_for, usage_for_agent, TranscriptUsage};
    let projects = tempfile::TempDir::new().unwrap();
    let worktree = tempfile::TempDir::new().unwrap();
    assert_eq!(
        usage_for_agent(projects.path(), worktree.path())
            .await
            .unwrap(),
        TranscriptUsage::default()
    );
    let dir = project_dir_for(projects.path(), worktree.path());
    fs::create_dir_all(&dir).unwrap();
    let line = r#"{"type":"assistant","message":{"id":"m1","model":"claude-x","usage":{"input_tokens":3,"output_tokens":4}}}"#;
    fs::write(dir.join("s.jsonl"), line).unwrap();
    assert_eq!(
        usage_for_agent(projects.path(), worktree.path())
            .await
            .unwrap(),
        TranscriptUsage {
            tokens: 7,
            model: Some("claude-x".into())
        }
    );
    let gone = worktree.path().join("gone");
    assert_eq!(
        usage_for_agent(projects.path(), &gone).await.unwrap(),
        TranscriptUsage::default()
    );
}

#[tokio::test]
async fn agent_runs_list_one_types_tasks_newest_first() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    let coder = townhall_core::model::NewTask {
        agent: "coder".into(),
        ..new_task("x")
    };
    for task in [new_task("a"), coder, new_task("b"), new_task("c")] {
        env.store.create_task(&c.id, task).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let runs = env.store.agent_runs("BUILDER", 10).await.unwrap();
    let names: Vec<&str> = runs.iter().map(|r| r.task.as_str()).collect();
    assert_eq!(names, ["c", "b", "a"]);
    assert_eq!(runs[0].model, None);
    assert_eq!(runs[0].tokens, 0);
    assert_eq!(env.store.agent_runs("builder", 2).await.unwrap().len(), 2);
    assert!(env.store.agent_runs("nobody", 10).await.unwrap().is_empty());
}

#[tokio::test]
async fn task_model_and_tokens_are_stored_on_the_run() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    env.store.create_task(&c.id, new_task("a")).await.unwrap();
    env.store.set_task_model("A", "claude-x").await.unwrap();
    env.store.set_task_tokens("a", 12).await.unwrap();
    let runs = env.store.agent_runs("builder", 10).await.unwrap();
    assert_eq!(runs[0].model.as_deref(), Some("claude-x"));
    assert_eq!(runs[0].tokens, 12);
    assert!(env.store.set_task_model("nope", "m").await.is_err());
}

#[tokio::test]
async fn conversation_turns_count_as_runs_of_the_default_type() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    env.store.create_task(&c.id, new_task("a")).await.unwrap();
    env.store.set_task_tokens("a", 100).await.unwrap();
    let at = "2026-11-09T10:00:00.000Z";
    env.store
        .record_turn(&c.id, "done", 30, at, at)
        .await
        .unwrap();
    env.store
        .record_turn(&c.id, "failed", 0, at, at)
        .await
        .unwrap();
    let usage = env.store.agent_usage().await.unwrap();
    assert_eq!(
        usage,
        vec![
            AgentUsage {
                agent: "builder".into(),
                runs: 1,
                tokens: 100
            },
            AgentUsage {
                agent: "default".into(),
                runs: 2,
                tokens: 30
            },
        ]
    );
    assert!(env
        .store
        .record_turn("nope", "done", 1, at, at)
        .await
        .is_err());
}

#[tokio::test]
async fn turns_are_listed_with_their_conversation_and_matched_ignoring_case() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    env.store.create_task(&c.id, new_task("a")).await.unwrap();
    env.store
        .record_turn(
            &c.id,
            "done",
            30,
            "2099-01-01T00:00:00.000Z",
            "2099-01-01T00:00:05.000Z",
        )
        .await
        .unwrap();
    let runs = env.store.agent_runs("DEFAULT", 10).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].kind, "turn");
    assert_eq!(runs[0].conversation_id, c.id);
    assert_eq!(runs[0].task, c.title);
    assert_eq!((runs[0].state.as_str(), runs[0].tokens), ("done", 30));
    assert_eq!(
        runs[0].ended_at.as_deref(),
        Some("2099-01-01T00:00:05.000Z")
    );
    let builder = env.store.agent_runs("builder", 10).await.unwrap();
    assert_eq!(builder.len(), 1);
    assert_eq!(builder[0].kind, "task");
    assert_eq!(builder[0].conversation_id, c.id);
    assert!(env.store.agent_runs("default", 0).await.unwrap().is_empty());
}

fn call(conversation: &str, task: Option<&str>, tokens: u64, at: &str) -> AdviserCall {
    AdviserCall {
        conversation_id: conversation.into(),
        task: task.map(String::from),
        agent: "adviser".into(),
        repo: "shop".into(),
        question: "which approach?".into(),
        answer: Some("the second".into()),
        state: "done".into(),
        model: Some("claude-fable-5-2".into()),
        tokens,
        created_at: at.into(),
        ended_at: at.into(),
    }
}

#[tokio::test]
async fn adviser_calls_count_as_runs_of_the_adviser() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    env.store.create_task(&c.id, new_task("a")).await.unwrap();
    env.store.set_task_tokens("a", 100).await.unwrap();
    let at = "2099-01-01T00:00:00.000Z";
    env.store
        .record_adviser_call(call(&c.id, Some("a"), 400, at))
        .await
        .unwrap();
    env.store
        .record_adviser_call(call(&c.id, None, 50, "2099-01-02T00:00:00.000Z"))
        .await
        .unwrap();
    let usage = env.store.agent_usage().await.unwrap();
    let adviser = usage.iter().find(|u| u.agent == "adviser").unwrap();
    assert_eq!((adviser.runs, adviser.tokens), (2, 450));
    let builder = usage.iter().find(|u| u.agent == "builder").unwrap();
    assert_eq!((builder.runs, builder.tokens), (1, 100));

    let runs = env.store.agent_runs("ADVISER", 10).await.unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].kind, "consult");
    assert_eq!(runs[0].task, c.title);
    assert_eq!(runs[1].task, "a");
    assert_eq!(
        (runs[1].tokens, runs[1].state.as_str(), runs[1].model.as_deref()),
        (400, "done", Some("claude-fable-5-2"))
    );
    assert_eq!(env.store.agent_runs("builder", 10).await.unwrap().len(), 1);

    assert!(env
        .store
        .record_adviser_call(call("nope", None, 1, at))
        .await
        .is_err());
}
