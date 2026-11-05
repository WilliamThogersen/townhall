use townhall_core::model::AgentUsage;
use townhall_core::steps::transcript_tokens;

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

#[tokio::test]
async fn tokens_for_agent_read_the_worktrees_newest_transcript() {
    use std::fs;
    use townhall_core::steps::{project_dir_for, tokens_for_agent};
    let projects = tempfile::TempDir::new().unwrap();
    let worktree = tempfile::TempDir::new().unwrap();
    assert_eq!(
        tokens_for_agent(projects.path(), worktree.path())
            .await
            .unwrap(),
        0
    );
    let dir = project_dir_for(projects.path(), worktree.path());
    fs::create_dir_all(&dir).unwrap();
    let line = r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":3,"output_tokens":4}}}"#;
    fs::write(dir.join("s.jsonl"), line).unwrap();
    assert_eq!(
        tokens_for_agent(projects.path(), worktree.path())
            .await
            .unwrap(),
        7
    );
    let gone = worktree.path().join("gone");
    assert_eq!(tokens_for_agent(projects.path(), &gone).await.unwrap(), 0);
}
