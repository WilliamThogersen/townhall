use std::collections::BTreeMap;

use townhall_core::model::{
    Checks, Done, EntryBody, NewTask, Pr, Task, TaskState, Verify, VerifyPhase,
};
use townhall_core::store::{GraphQuery, Landed, NodeKind};

mod common;
use common::{new_task, store_env, StoreEnv};

fn query(kind: Option<NodeKind>, node: &str, deep: bool) -> GraphQuery {
    GraphQuery {
        kind,
        node: node.into(),
        since_ms: None,
        limit: 20,
        deep,
    }
}

async fn add(env: &StoreEnv, conv: &str, name: &str, agent: &str, after: &[&str]) {
    let mut new: NewTask = new_task(name);
    new.agent = agent.into();
    new.after = after.iter().map(|a| a.to_string()).collect();
    env.store.create_task(conv, new).await.unwrap();
}

async fn finish(env: &StoreEnv, name: &str, state: TaskState) {
    env.store
        .update_task(name, move |t| Ok(Task { state, ..t }))
        .await
        .unwrap();
}

async fn say(env: &StoreEnv, conv: &str, text: &str) {
    env.store
        .append_entry(conv, EntryBody::Event { text: text.into() })
        .await
        .unwrap();
}

async fn outcome(env: &StoreEnv, name: &str) -> Option<String> {
    let found = env
        .store
        .history_graph(query(Some(NodeKind::Task), name, true))
        .await
        .unwrap()
        .unwrap();
    found.tasks[0].outcome.clone()
}

async fn files(env: &StoreEnv, name: &str) -> Vec<String> {
    let found = env
        .store
        .history_graph(query(Some(NodeKind::Task), name, true))
        .await
        .unwrap()
        .unwrap();
    found.tasks[0].files.clone()
}

fn landed(tasks: &[&str], prs: &[u64], files: &[(&str, &[&str])]) -> Landed {
    Landed {
        head: "abc".into(),
        tasks: tasks.iter().map(|t| t.to_string()).collect(),
        prs: prs.to_vec(),
        files: files
            .iter()
            .map(|(t, f)| (t.to_string(), f.iter().map(|f| f.to_string()).collect()))
            .collect::<BTreeMap<_, _>>(),
    }
}

async fn with_pr(env: &StoreEnv, name: &str, number: u64) {
    env.store
        .update_task(name, move |t| {
            Ok(Task {
                pr: Some(Pr {
                    number,
                    url: None,
                    checks: Checks::Passed,
                    merge_sha: None,
                }),
                ..t
            })
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn landed_tasks_count_as_merged_by_name_by_pr_and_by_what_the_lander_said() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap().id;
    for name in ["by-name", "by-pr", "lander", "plain"] {
        add(&env, &c, name, "builder", &[]).await;
        finish(&env, name, TaskState::Ended).await;
    }
    with_pr(&env, "by-pr", 7).await;
    say(
        &env,
        &c,
        "task lander ended (done). its agent's last message: merged PR #130 at ec5fef5",
    )
    .await;
    say(
        &env,
        &c,
        "task plain ended (done). its agent's last message: opened PR #131, not touching #130",
    )
    .await;
    for name in ["by-name", "by-pr", "lander", "plain"] {
        assert_eq!(outcome(&env, name).await.as_deref(), Some("ended"));
    }

    env.store
        .apply_landed("shop", landed(&["by-name"], &[7, 130], &[]))
        .await
        .unwrap();

    for name in ["by-name", "by-pr", "lander"] {
        assert_eq!(
            outcome(&env, name).await.as_deref(),
            Some("merged"),
            "{name}"
        );
    }
    assert_eq!(outcome(&env, "plain").await.as_deref(), Some("ended"));
}

#[tokio::test]
async fn the_builder_a_merged_lander_waited_for_is_merged_too() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap().id;
    add(&env, &c, "feature", "builder", &[]).await;
    add(&env, &c, "land-feature", "lander", &["feature"]).await;
    add(&env, &c, "idle", "reviewer", &[]).await;
    for name in ["feature", "land-feature", "idle"] {
        finish(&env, name, TaskState::Ended).await;
    }
    env.store
        .record_task_graph("feature", vec!["src/a.rs".into()], vec![])
        .await
        .unwrap();
    env.store
        .apply_landed("shop", landed(&["land-feature"], &[], &[]))
        .await
        .unwrap();
    assert_eq!(outcome(&env, "feature").await.as_deref(), Some("merged"));
    assert_eq!(outcome(&env, "idle").await.as_deref(), Some("ended"));
}

#[tokio::test]
async fn merged_survives_a_later_update_and_applying_twice_changes_nothing() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap().id;
    add(&env, &c, "alpha", "builder", &[]).await;
    finish(&env, "alpha", TaskState::Ended).await;
    let facts = landed(&["alpha"], &[], &[("alpha", &["src/x/a.rs", "src/x/b.rs"])]);
    env.store.apply_landed("shop", facts.clone()).await.unwrap();
    env.store.apply_landed("shop", facts).await.unwrap();
    finish(&env, "alpha", TaskState::Ended).await;
    assert_eq!(outcome(&env, "alpha").await.as_deref(), Some("merged"));
    assert_eq!(files(&env, "alpha").await, ["src/x/a.rs", "src/x/b.rs"]);
}

#[tokio::test]
async fn merge_commit_files_fill_only_tasks_with_no_files_and_are_found_by_file() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap().id;
    add(&env, &c, "bare", "builder", &[]).await;
    add(&env, &c, "known", "builder", &[]).await;
    finish(&env, "bare", TaskState::Ended).await;
    finish(&env, "known", TaskState::Ended).await;
    env.store
        .record_task_graph("known", vec!["own.rs".into()], vec![])
        .await
        .unwrap();
    let missing = env
        .store
        .tasks_missing_files(vec!["bare".into(), "known".into(), "ghost".into()])
        .await
        .unwrap();
    assert_eq!(missing, ["bare"]);
    env.store
        .apply_landed(
            "shop",
            landed(
                &["bare", "known"],
                &[],
                &[("bare", &["insights/mod.rs"]), ("known", &["other.rs"])],
            ),
        )
        .await
        .unwrap();
    let found = env
        .store
        .history_graph(query(None, "insights/mod.rs", false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.tasks[0].name, "bare");
    assert_eq!(files(&env, "known").await, ["own.rs"]);
}

#[tokio::test]
async fn a_repo_is_scanned_again_only_when_its_head_moved_or_a_task_is_new() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap().id;
    add(&env, &c, "alpha", "builder", &[]).await;
    finish(&env, "alpha", TaskState::Ended).await;
    assert!(env.store.landing_scan_due("shop", "abc").await.unwrap());
    env.store
        .record_task_graph("alpha", vec![], vec![])
        .await
        .unwrap();
    assert!(env.store.landing_scan_due("shop", "abc").await.unwrap());
    env.store
        .apply_landed("shop", landed(&[], &[], &[]))
        .await
        .unwrap();
    assert!(!env.store.landing_scan_due("shop", "abc").await.unwrap());
    assert!(env.store.landing_scan_due("shop", "def").await.unwrap());
}

async fn failed_with(env: &StoreEnv, conv: &str, name: &str, text: &str) {
    add(env, conv, name, name, &[]).await;
    finish(env, name, TaskState::Error).await;
    say(env, conv, &format!("task {name} failed: {text}")).await;
}

#[tokio::test]
async fn failures_are_classified_from_the_stored_text_and_check_output() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap().id;
    failed_with(
        &env,
        &c,
        "f-conn",
        "claude failed: connection lost, the machine went to sleep",
    )
    .await;
    failed_with(
        &env,
        &c,
        "f-tool",
        "claude failed: cargo: command not found",
    )
    .await;
    failed_with(&env, &c, "f-ci", "ci failed on PR #4").await;
    failed_with(&env, &c, "f-retries", "gave up after max retries").await;
    failed_with(
        &env,
        &c,
        "f-deny",
        "tool use was denied: permission denied for Bash",
    )
    .await;
    failed_with(&env, &c, "f-none", "something odd happened").await;
    add(&env, &c, "f-check", "f-check", &[]).await;
    env.store
        .update_task("f-check", |t| {
            Ok(Task {
                state: TaskState::Error,
                done: Done {
                    verify: Some(Verify {
                        phase: VerifyPhase::Failed,
                        retries: 3,
                        output: Some("error[E0432]: unresolved import\nmore".into()),
                    }),
                    ..t.done
                },
                ..t
            })
        })
        .await
        .unwrap();
    add(&env, &c, "asks", "asks", &[]).await;
    finish(&env, "asks", TaskState::NeedsYou).await;
    say(&env, &c, "task asks needs you, its agent asks: which db?").await;
    add(&env, &c, "blocked", "blocked", &[]).await;
    finish(&env, "blocked", TaskState::NeedsYou).await;
    say(
        &env,
        &c,
        "task blocked needs you, its agent asks: permission denied to run `rm`, allow it?",
    )
    .await;
    add(&env, &c, "no-event", "no-event", &[]).await;
    finish(&env, "no-event", TaskState::Error).await;

    let stats = env.store.agent_stats(None).await.unwrap();
    for (agent, expected) in [
        ("f-conn", "connection lost"),
        ("f-tool", "tool missing"),
        ("f-ci", "ci failed"),
        ("f-retries", "max retries"),
        ("f-deny", "permission denied"),
        ("f-none", "something odd happened"),
        ("f-check", "check failed: error[E<n>]: unresolved import"),
        ("asks", "needs user: which db?"),
        ("blocked", "permission denied"),
        ("no-event", "unknown"),
    ] {
        let stat = stats.iter().find(|s| s.agent == agent).unwrap();
        assert_eq!(stat.failures.len(), 1, "{agent}");
        assert!(
            stat.failures[0].0.starts_with(expected),
            "{agent}: {:?}",
            stat.failures
        );
    }
}

#[tokio::test]
async fn fails_on_lists_the_folders_failed_and_needs_user_tasks_changed() {
    let env = store_env();
    let c = env.store.create_conversation().await.unwrap().id;
    add(&env, &c, "bad", "builder", &[]).await;
    finish(&env, "bad", TaskState::Error).await;
    add(&env, &c, "ask", "builder", &[]).await;
    finish(&env, "ask", TaskState::NeedsYou).await;
    env.store
        .record_task_graph(
            "bad",
            vec!["src/store/a.rs".into(), "web/x.ts".into()],
            vec![],
        )
        .await
        .unwrap();
    env.store
        .apply_landed("shop", landed(&[], &[], &[("ask", &["src/store/q.rs"])]))
        .await
        .unwrap();
    let stats = env.store.agent_stats(None).await.unwrap();
    assert_eq!(stats[0].areas[0], ("src/store".to_string(), 2));
    assert_eq!(stats[0].areas[1], ("web".to_string(), 1));
}
