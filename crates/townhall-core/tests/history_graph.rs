use townhall_core::model::{Checks, Pr, Task, TaskState, Verify, VerifyPhase};
use townhall_core::store::{GraphQuery, NodeKind};

mod common;
use common::{new_task, store_env, StoreEnv};

fn query(kind: Option<NodeKind>, node: &str) -> GraphQuery {
    GraphQuery {
        kind,
        node: node.into(),
        since_ms: None,
        limit: 20,
        deep: false,
    }
}

async fn finish(env: &StoreEnv, name: &str, state: TaskState) {
    env.store
        .update_task(name, move |t| Ok(Task { state, ..t }))
        .await
        .unwrap();
}

async fn task_in(env: &StoreEnv, conversation: &str, name: &str, agent: &str) {
    let mut new = new_task(name);
    new.agent = agent.into();
    env.store.create_task(conversation, new).await.unwrap();
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[tokio::test]
async fn creating_and_ending_a_task_records_its_structural_edges_and_outcome() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    task_in(&env, &c.id, "alpha", "builder").await;
    env.store
        .update_task("alpha", |t| {
            Ok(Task {
                state: TaskState::Merged,
                pr: Some(Pr {
                    number: 9,
                    url: None,
                    checks: Checks::Passed,
                }),
                ..t
            })
        })
        .await
        .unwrap();

    let by_branch = env
        .store
        .history_graph(query(Some(NodeKind::Branch), "townhall/alpha"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_branch.tasks.len(), 1);
    let task = &by_branch.tasks[0];
    assert_eq!(task.name, "alpha");
    assert_eq!(task.conversation, c.id);
    assert_eq!(task.agent, "builder");
    assert_eq!(task.outcome.as_deref(), Some("merged"));
    assert_eq!(task.branch, "townhall/alpha");
    assert_eq!(task.pr.as_deref(), Some("shop#9"));

    for (kind, node) in [
        (NodeKind::Conversation, c.id.as_str()),
        (NodeKind::Agent, "builder"),
        (NodeKind::Pr, "shop#9"),
        (NodeKind::Outcome, "merged"),
        (NodeKind::Task, "alpha"),
    ] {
        let found = env
            .store
            .history_graph(query(Some(kind), node))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.tasks.len(), 1, "{kind:?} {node}");
    }
}

#[tokio::test]
async fn outcomes_follow_the_task_state() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    for (name, state, outcome) in [
        ("a-failed", TaskState::Error, "failed"),
        ("a-asking", TaskState::NeedsYou, "needs-user"),
        ("a-ended", TaskState::Ended, "ended"),
        ("a-review", TaskState::InReview, "in-review"),
    ] {
        task_in(&env, &c.id, name, "builder").await;
        finish(&env, name, state).await;
        let found = env
            .store
            .history_graph(query(Some(NodeKind::Task), name))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.tasks[0].outcome.as_deref(), Some(outcome), "{name}");
    }
    task_in(&env, &c.id, "a-queued", "builder").await;
    let queued = env
        .store
        .history_graph(query(Some(NodeKind::Task), "a-queued"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(queued.tasks[0].outcome, None);
}

#[tokio::test]
async fn files_and_fingerprints_recorded_at_the_end_are_found_by_path_suffix_and_glob() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    task_in(&env, &c.id, "alpha", "builder").await;
    task_in(&env, &c.id, "beta", "reviewer").await;
    finish(&env, "alpha", TaskState::Ended).await;
    finish(&env, "beta", TaskState::Error).await;
    env.store
        .record_task_graph(
            "alpha",
            strings(&["src/insights/mod.rs", "src/lib.rs"]),
            strings(&["00aa00aa00aa00aa"]),
        )
        .await
        .unwrap();
    env.store
        .record_task_graph("beta", strings(&["src/insights/tests.rs"]), vec![])
        .await
        .unwrap();

    let exact = env
        .store
        .history_graph(query(None, "src/insights/mod.rs"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exact.kind, NodeKind::File);
    assert_eq!(exact.tasks.len(), 1);
    assert_eq!(exact.tasks[0].name, "alpha");

    let suffix = env
        .store
        .history_graph(query(None, "insights/mod.rs"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(suffix.tasks.len(), 1);

    let glob = env
        .store
        .history_graph(query(None, "src/insights/*"))
        .await
        .unwrap()
        .unwrap();
    let mut names: Vec<&str> = glob.tasks.iter().map(|t| t.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["alpha", "beta"]);

    let print = env
        .store
        .history_graph(query(None, "00aa00aa00aa00aa"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(print.kind, NodeKind::Fingerprint);
    assert_eq!(print.tasks[0].name, "alpha");

    let mut deep = query(Some(NodeKind::Task), "alpha");
    deep.deep = true;
    let deep = env.store.history_graph(deep).await.unwrap().unwrap();
    assert_eq!(deep.tasks[0].files, ["src/insights/mod.rs", "src/lib.rs"]);
    assert_eq!(deep.tasks[0].fingerprints, ["00aa00aa00aa00aa"]);
    let shallow = env
        .store
        .history_graph(query(None, "src/lib.rs"))
        .await
        .unwrap()
        .unwrap();
    assert!(shallow.tasks[0].files.is_empty());
    let own = env
        .store
        .history_graph(query(Some(NodeKind::Task), "alpha"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(own.tasks[0].files.len(), 2);
}

#[tokio::test]
async fn recording_twice_replaces_and_unknown_nodes_are_none() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    task_in(&env, &c.id, "alpha", "builder").await;
    finish(&env, "alpha", TaskState::Ended).await;
    env.store
        .record_task_graph("alpha", strings(&["a.rs", "b.rs"]), vec![])
        .await
        .unwrap();
    env.store
        .record_task_graph("alpha", strings(&["b.rs"]), vec![])
        .await
        .unwrap();
    let mut deep = query(Some(NodeKind::Task), "alpha");
    deep.deep = true;
    let found = env.store.history_graph(deep).await.unwrap().unwrap();
    assert_eq!(found.tasks[0].files, ["b.rs"]);
    assert!(env
        .store
        .history_graph(query(None, "nothing/here.rs"))
        .await
        .unwrap()
        .is_none());
    assert!(env
        .store
        .history_graph(query(Some(NodeKind::Task), "nope"))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn limit_and_since_narrow_the_tasks_newest_first() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    for name in ["t-1", "t-2", "t-3"] {
        task_in(&env, &c.id, name, "builder").await;
        finish(&env, name, TaskState::Ended).await;
        env.store
            .record_task_graph(name, strings(&["shared.rs"]), vec![])
            .await
            .unwrap();
    }
    let mut q = query(None, "shared.rs");
    q.limit = 2;
    let found = env.store.history_graph(q).await.unwrap().unwrap();
    assert_eq!(found.tasks.len(), 2);
    assert_eq!(found.matched, 3);
    assert_eq!(found.tasks[0].name, "t-3");
    let mut future = query(None, "shared.rs");
    future.since_ms = Some(32_503_680_000_000);
    let none = env.store.history_graph(future).await.unwrap().unwrap();
    assert!(none.tasks.is_empty());
}

#[tokio::test]
async fn ended_tasks_without_a_graph_record_are_listed_until_recorded() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    task_in(&env, &c.id, "alpha", "builder").await;
    task_in(&env, &c.id, "beta", "builder").await;
    finish(&env, "alpha", TaskState::Merged).await;
    let open = env.store.unrecorded_graph_tasks().await.unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].name, "alpha");
    env.store
        .record_task_graph("alpha", vec![], vec![])
        .await
        .unwrap();
    assert!(env.store.unrecorded_graph_tasks().await.unwrap().is_empty());
}

#[tokio::test]
async fn agent_stats_count_outcomes_retries_handbacks_reasons_and_areas() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap();
    task_in(&env, &c.id, "good", "builder").await;
    task_in(&env, &c.id, "bad-1", "builder").await;
    task_in(&env, &c.id, "bad-2", "builder").await;
    task_in(&env, &c.id, "rev", "reviewer").await;
    finish(&env, "good", TaskState::Merged).await;
    env.store
        .update_task("bad-1", |t| {
            Ok(Task {
                state: TaskState::Error,
                done: townhall_core::model::Done {
                    verify: Some(Verify {
                        phase: VerifyPhase::Failed,
                        retries: 2,
                        output: None,
                    }),
                    ..t.done
                },
                ..t
            })
        })
        .await
        .unwrap();
    finish(&env, "bad-2", TaskState::Error).await;
    finish(&env, "rev", TaskState::Ended).await;
    for text in [
        "task bad-1 failed: API error 529 overloaded",
        "task bad-2 failed: API error 529 overloaded",
    ] {
        env.store
            .append_entry(
                &c.id,
                townhall_core::model::EntryBody::Event { text: text.into() },
            )
            .await
            .unwrap();
    }
    for (task, files) in [
        ("bad-1", vec!["src/store/a.rs", "src/store/b.rs"]),
        ("bad-2", vec!["src/store/c.rs", "docs/x.md"]),
        ("good", vec!["src/mcp/m.rs"]),
    ] {
        env.store
            .record_task_graph(task, strings(&files), vec![])
            .await
            .unwrap();
    }

    let stats = env.store.agent_stats(None).await.unwrap();
    let builder = stats.iter().find(|s| s.agent == "builder").unwrap();
    assert_eq!(builder.tasks, 3);
    assert_eq!(builder.outcomes.get("merged"), Some(&1));
    assert_eq!(builder.outcomes.get("api-error"), Some(&2));
    assert!((builder.avg_retries - 2.0 / 3.0).abs() < 1e-9);
    assert_eq!(builder.failures.len(), 1);
    assert_eq!(builder.failures[0].0, "API error <n> overloaded");
    assert_eq!(builder.failures[0].1, 2);
    assert_eq!(builder.areas[0], ("src/store".to_string(), 3));
    let reviewer = stats.iter().find(|s| s.agent == "reviewer").unwrap();
    assert_eq!(reviewer.tasks, 1);
    assert!(reviewer.failures.is_empty() && reviewer.areas.is_empty());

    let none = env
        .store
        .agent_stats(Some(32_503_680_000_000))
        .await
        .unwrap();
    assert!(none.is_empty());
}
