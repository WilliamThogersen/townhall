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

async fn merge_branch(s: &Sched, name: &str, message: &str, fast_forward: bool) {
    let branch = format!("townhall/{name}");
    let mode = if fast_forward { "--ff-only" } else { "--no-ff" };
    let mut args = vec![
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "merge",
        "-q",
        mode,
    ];
    if !fast_forward {
        args.extend(["-m", message]);
    }
    args.push(&branch);
    git(&s.repo, &args).await;
}

async fn drop_branch(s: &Sched, name: &str) {
    let tree = s.worktree(name).to_string_lossy().into_owned();
    git(&s.repo, &["worktree", "remove", "--force", &tree]).await;
    git(&s.repo, &["branch", "-D", &format!("townhall/{name}")]).await;
}

async fn started_with_work(s: &Sched, name: &str, file: &str) {
    s.create(name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    change(s, name, file).await;
}

#[tokio::test]
async fn a_merge_commit_gives_a_task_with_a_deleted_branch_its_files_and_a_merged_outcome() {
    let s = Sched::new().await;
    let (a, b) = (unique("by-branch"), unique("by-pr"));
    started_with_work(&s, &a, "src/insights/mod.rs").await;
    started_with_work(&s, &b, "src/other/x.rs").await;
    s.store()
        .update_task(&b, |t| {
            Ok(townhall_core::model::Task {
                pr: Some(townhall_core::model::Pr {
                    number: 9,
                    url: None,
                    checks: townhall_core::model::Checks::Passed,
                    merge_sha: None,
                }),
                ..t
            })
        })
        .await
        .unwrap();
    let first = format!("Merge pull request #5 from someone/townhall/{a}");
    merge_branch(&s, &a, &first, false).await;
    merge_branch(&s, &b, "Merge pull request #9 from someone/renamed", false).await;
    s.end(&a).await;
    s.end(&b).await;
    drop_branch(&s, &a).await;
    drop_branch(&s, &b).await;

    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.warnings.is_empty(), "{pass:?}");

    let found = s
        .store()
        .history_graph(query(None, "insights/mod.rs", false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.tasks[0].name, a);
    assert_eq!(found.tasks[0].outcome.as_deref(), Some("merged"));
    let by_pr = s
        .store()
        .history_graph(query(Some(NodeKind::Task), &b, true))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_pr.tasks[0].files, ["src/other/x.rs"]);
    assert_eq!(by_pr.tasks[0].outcome.as_deref(), Some("merged"));

    let again = run_once(s.ctx()).await.unwrap();
    assert!(again.warnings.is_empty(), "{again:?}");
    let twice = s
        .store()
        .history_graph(query(Some(NodeKind::Task), &a, true))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(twice.tasks[0].files, ["src/insights/mod.rs"]);
}

#[tokio::test]
async fn a_branch_reachable_from_the_base_counts_as_merged() {
    let s = Sched::new().await;
    let name = unique("fast");
    started_with_work(&s, &name, "lib/f.rs").await;
    merge_branch(&s, &name, "", true).await;
    s.end(&name).await;
    run_once(s.ctx()).await.unwrap();
    let found = s
        .store()
        .history_graph(query(Some(NodeKind::Task), &name, false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.tasks[0].outcome.as_deref(), Some("merged"));
}

#[tokio::test]
async fn a_branch_with_no_work_of_its_own_is_not_merged() {
    let s = Sched::new().await;
    let name = unique("idle");
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    s.end(&name).await;
    run_once(s.ctx()).await.unwrap();
    let found = s
        .store()
        .history_graph(query(Some(NodeKind::Task), &name, false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.tasks[0].outcome.as_deref(), Some("ended"));
}
