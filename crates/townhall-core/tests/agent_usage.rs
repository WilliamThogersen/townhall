use townhall_core::model::AgentUsage;
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
