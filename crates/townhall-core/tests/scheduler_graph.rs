use townhall_core::model::TaskState;
use townhall_core::scheduler::run_once;
use townhall_core::store::{GraphQuery, NodeKind};

mod scheduler_support;
use scheduler_support::{commit, git, unique, Sched};

fn query(kind: Option<NodeKind>, node: &str, deep: bool) -> GraphQuery {
    GraphQuery {
        kind,
        node: node.into(),
        since_ms: None,
        limit: 20,
        deep,
    }
}

async fn change(s: &Sched, task: &str, file: &str) {
    let dir = s.worktree(task);
    std::fs::create_dir_all(dir.join(file).parent().unwrap()).unwrap();
    std::fs::write(dir.join(file), "work\n").unwrap();
    git(&dir, &["add", "-A"]).await;
    commit(&dir, "the work").await;
}

#[tokio::test]
async fn an_ended_task_gets_the_files_its_branch_changed() {
    let s = Sched::new().await;
    let name = unique("edit");
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    change(&s, &name, "src/insights/mod.rs").await;
    change(&s, &name, "docs/notes.md").await;
    assert!(s.store().unrecorded_graph_tasks().await.unwrap().is_empty());

    s.end(&name).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.warnings.is_empty(), "{pass:?}");

    let found = s
        .store()
        .history_graph(query(None, "src/insights/mod.rs", false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.kind, NodeKind::File);
    assert_eq!(found.tasks[0].name, name);
    assert_eq!(found.tasks[0].outcome.as_deref(), Some("ended"));
    let own = s
        .store()
        .history_graph(query(Some(NodeKind::Task), &name, true))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(own.tasks[0].files, ["docs/notes.md", "src/insights/mod.rs"]);
    assert!(s.store().unrecorded_graph_tasks().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_merged_task_keeps_its_files_after_its_branch_is_in_the_base() {
    let s = Sched::new().await;
    let name = unique("shipped");
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    change(&s, &name, "lib/a.rs").await;
    let branch = format!("townhall/{name}");
    git(
        &s.repo,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "merge it",
            &branch,
        ],
    )
    .await;
    s.end(&name).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.state(&name).await, TaskState::Merged);
    let found = s
        .store()
        .history_graph(query(None, "lib/a.rs", false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.tasks[0].name, name);
    assert_eq!(found.tasks[0].outcome.as_deref(), Some("merged"));
}

#[tokio::test]
async fn backfill_records_old_tasks_best_effort_even_when_the_branch_is_gone() {
    let s = Sched::new().await;
    let (old, gone) = (unique("old"), unique("gone"));
    s.create(&old, &[]).await;
    run_once(s.ctx()).await.unwrap();
    change(&s, &old, "old/file.rs").await;
    s.create(&gone, &[]).await;
    s.set_state(&gone, TaskState::Ended).await;
    s.set_state(&old, TaskState::Ended).await;
    assert_eq!(s.store().unrecorded_graph_tasks().await.unwrap().len(), 2);

    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.warnings.is_empty(), "{pass:?}");
    assert!(s.store().unrecorded_graph_tasks().await.unwrap().is_empty());
    let found = s
        .store()
        .history_graph(query(None, "old/file.rs", false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.tasks[0].name, old);
    let none = s
        .store()
        .history_graph(query(Some(NodeKind::Task), &gone, true))
        .await
        .unwrap()
        .unwrap();
    assert!(none.tasks[0].files.is_empty());
    assert_eq!(none.tasks[0].outcome.as_deref(), Some("ended"));
}
