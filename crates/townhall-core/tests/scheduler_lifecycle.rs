use std::time::Duration;

use townhall_core::model::{AgentEvent, EntryBody, ServerEvent, Task, TaskState};
use townhall_core::scheduler::{self, run_once};

mod scheduler_support;
use scheduler_support::{commit, git, kill_sessions, sleeper, unique, Sched};

async fn merge(repo: &std::path::Path, branch: &str) {
    let args = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "merge",
        "-q",
        "--no-ff",
        "-m",
        "merge it",
        branch,
    ];
    git(repo, &args).await;
}

#[tokio::test]
async fn merged_is_detected_after_a_real_merge_and_never_merges_anything() {
    let s = Sched::new().await;
    let (done, idle) = (unique("done"), unique("idle"));
    s.create(&done, &[]).await;
    s.create(&idle, &[]).await;
    run_once(s.ctx()).await.unwrap();
    commit(&s.worktree(&done), "the work").await;
    s.end(&done).await;
    s.end(&idle).await;
    let main_before = git(&s.repo, &["rev-parse", "main"]).await;
    assert!(run_once(s.ctx()).await.unwrap().merged.is_empty());
    assert_eq!(s.state(&done).await, TaskState::Ended);
    assert_eq!(git(&s.repo, &["rev-parse", "main"]).await, main_before);

    let branch = format!("townhall/{done}");
    merge(&s.repo, &branch).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.merged, [done.as_str()]);
    assert_eq!(s.state(&done).await, TaskState::Merged);
    let idle_state = s.state(&idle).await;
    assert_eq!(
        idle_state,
        TaskState::Ended,
        "no commits of its own: not merged"
    );
    match s.last_entry().await {
        EntryBody::Agent { agent, event, text } => {
            assert_eq!((agent.as_str(), event), (done.as_str(), AgentEvent::Passed));
            assert!(text.contains("main"), "{text}");
        }
        other => panic!("expected an agent entry, got {other:?}"),
    }
    assert!(run_once(s.ctx()).await.unwrap().merged.is_empty());
    let branches = git(&s.repo, &["branch", "--list", &branch]).await;
    assert!(branches.contains(&branch), "the branch is kept");
}

#[tokio::test]
async fn a_restart_never_starts_a_second_agent() {
    let mut s = Sched::new().await;
    let (running, crashed) = (unique("running"), unique("crashed"));
    s.create(&running, &[]).await;
    assert_eq!(run_once(s.ctx()).await.unwrap().started, [running.as_str()]);
    s.create(&crashed, &[]).await;
    sleeper(&s.ws, &crashed).await;

    s.restart().await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.started.is_empty(), "{pass:?}");
    assert_eq!(pass.adopted, [crashed.as_str()]);
    assert_eq!(s.state(&running).await, TaskState::Working);
    assert_eq!(s.state(&crashed).await, TaskState::Working);
    let mut want = vec![running.clone(), crashed.clone()];
    want.sort();
    assert_eq!(s.agent_names().await, want);
    let main = git(&s.repo, &["rev-parse", "main"]).await;
    let launch = s.store().task_launch(&crashed).await.unwrap().unwrap();
    assert_eq!(launch.start_commit, main);

    s.restart().await;
    let pass = run_once(s.ctx()).await.unwrap();
    let idle = pass.started.is_empty() && pass.adopted.is_empty();
    assert!(idle, "{pass:?}");
    assert_eq!(s.agents().await.len(), 2);
}

#[tokio::test]
async fn the_loop_wakes_on_task_updates() {
    let s = Sched::new().await;
    let mut events = s.ctx().bus().subscribe();
    let handle = scheduler::start(s.ctx()).expect("the scheduler runs");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let name = unique("woken");
    s.create(&name, &[]).await;
    let working = tokio::time::timeout(Duration::from_millis(1500), async {
        loop {
            if let Ok(ServerEvent::TaskUpdated { task }) = events.recv().await {
                if task.name == name && task.state == TaskState::Working {
                    return task;
                }
            }
        }
    })
    .await;
    handle.abort();
    assert!(
        working.is_ok(),
        "the task did not start before the slow tick"
    );
}

#[tokio::test]
async fn a_name_another_repo_has_is_an_error() {
    let s = Sched::new().await;
    let (other, other_ws) = s.other_repo().await;
    let name = unique("taken");
    sleeper(&other_ws, &name).await;
    s.create(&name, &[]).await;
    let pass = run_once(s.ctx()).await;
    kill_sessions(&other_ws);
    assert_eq!(pass.unwrap().failed, [name.as_str()]);
    assert_eq!(s.state(&name).await, TaskState::Error);
    let text = s.failed_text(&name).await;
    assert!(text.contains(&other.name), "{text}");
}

#[tokio::test]
async fn a_base_that_cannot_be_read_is_warned_not_merged() {
    let s = Sched::new().await;
    let ended = unique("ended");
    s.create(&ended, &[]).await;
    run_once(s.ctx()).await.unwrap();
    s.end(&ended).await;
    let change = |t: Task| {
        Ok(Task {
            base_branch: "-oops".into(),
            ..t
        })
    };
    s.store().update_task(&ended, change).await.unwrap();
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.merged.is_empty());
    let warned = pass.warnings.iter().any(|w| w.contains("-oops"));
    assert!(warned, "{:?}", pass.warnings);
}

#[tokio::test]
async fn a_base_branch_deleted_after_its_merge_is_skipped_without_a_warning() {
    let s = Sched::new().await;
    let ended = unique("stacked");
    s.create(&ended, &[]).await;
    run_once(s.ctx()).await.unwrap();
    commit(&s.worktree(&ended), "the work").await;
    s.end(&ended).await;
    let gone = format!("townhall/{}", unique("gone"));
    git(&s.repo, &["branch", &gone, "main"]).await;
    let base = gone.clone();
    let change = move |t: Task| {
        Ok(Task {
            base_branch: base.clone(),
            ..t
        })
    };
    s.store().update_task(&ended, change).await.unwrap();
    git(&s.repo, &["branch", "-D", &gone]).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.merged.is_empty());
    assert!(pass.warnings.is_empty(), "{:?}", pass.warnings);
    assert_eq!(s.state(&ended).await, TaskState::Ended);
}
