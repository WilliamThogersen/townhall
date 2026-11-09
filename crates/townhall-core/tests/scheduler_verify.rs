use std::time::Duration;

use townhall_core::model::{Done, NewTask, Task, TaskState, Verify, VerifyPhase};
use townhall_core::scheduler::run_once;

mod scheduler_support;
use scheduler_support::{unique, Sched};

/// A running task with `check`, as the watcher leaves it when its agent ended.
async fn verifying(s: &Sched, check: &str, max_retries: u32) -> String {
    let name = unique("verify");
    let new = NewTask {
        done: Done {
            check: Some(check.into()),
            max_retries,
            ..Done::default()
        },
        ..s.new_task(&name, &[])
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.state(&name).await, TaskState::Working);
    mark(s, &name, 0).await;
    name
}

async fn mark(s: &Sched, name: &str, retries: u32) {
    let change = move |t: Task| {
        Ok(Task {
            done: Done {
                verify: Some(Verify {
                    phase: VerifyPhase::Verifying,
                    retries,
                    output: None,
                }),
                ..t.done
            },
            ..t
        })
    };
    s.store().update_task(name, change).await.unwrap();
}

/// One pass starts the check; then waits until it has been judged: the task left `verifying`.
/// No further pass runs, so a task queued again is still queued when this returns.
async fn judged(s: &Sched, name: &str) -> Task {
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.verifying, std::slice::from_ref(&name.to_string()));
    for _ in 0..100 {
        let task = s.store().find_task(name).await.unwrap().unwrap();
        let phase = task.done.verify.as_ref().map(|v| v.phase);
        if phase != Some(VerifyPhase::Verifying) {
            return task;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the check of `{name}` was never judged");
}

#[tokio::test]
async fn a_passing_check_ends_the_task() {
    let s = Sched::new().await;
    let name = verifying(&s, "test -d .git || test -f .git", 2).await;
    let task = judged(&s, &name).await;
    assert_eq!(task.state, TaskState::Ended);
    let verify = task.done.verify.unwrap();
    assert_eq!(verify.phase, VerifyPhase::Passed);
    assert_eq!(verify.output, None);
}

#[tokio::test]
async fn the_check_runs_in_the_worktree_of_the_task() {
    let s = Sched::new().await;
    let name = verifying(&s, "touch ran-here", 2).await;
    judged(&s, &name).await;
    assert!(s.worktree(&name).join("ran-here").exists());
}

#[tokio::test]
async fn a_failing_check_relaunches_the_task_with_its_output_in_the_brief() {
    let s = Sched::new().await;
    let name = verifying(&s, "echo boom-out; echo boom-err >&2; exit 3", 2).await;
    let task = judged(&s, &name).await;
    assert_eq!(task.state, TaskState::Queued, "{task:?}");
    let verify = task.done.verify.clone().unwrap();
    assert_eq!((verify.phase, verify.retries), (VerifyPhase::Retrying, 1));
    let output = verify.output.unwrap();
    assert!(
        output.contains("boom-out") && output.contains("boom-err"),
        "{output}"
    );
    assert!(output.contains("exit code 3"), "{output}");
    let sessions = s
        .store()
        .task_launch(&name)
        .await
        .unwrap()
        .unwrap()
        .session_id;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, std::slice::from_ref(&name), "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::Working);
    let again = s
        .store()
        .task_launch(&name)
        .await
        .unwrap()
        .unwrap()
        .session_id;
    assert_ne!(sessions, again, "a new agent run");
    let brief = std::fs::read_to_string(s.brief(&name)).unwrap();
    assert!(
        brief.contains("boom-out") && brief.contains("retry 1/2"),
        "{brief}"
    );
    assert!(brief.contains(&format!("the brief for {name}")), "{brief}");
}

#[tokio::test]
async fn a_check_that_keeps_failing_needs_the_user_after_the_last_retry() {
    let s = Sched::new().await;
    let name = verifying(&s, "echo still-red; exit 1", 1).await;
    let first = judged(&s, &name).await;
    assert_eq!(first.state, TaskState::Queued);
    run_once(s.ctx()).await.unwrap();
    mark(&s, &name, 1).await;
    let last = judged(&s, &name).await;
    assert_eq!(last.state, TaskState::NeedsYou);
    let verify = last.done.verify.unwrap();
    assert_eq!((verify.phase, verify.retries), (VerifyPhase::Failed, 1));
    assert!(verify.output.unwrap().contains("still-red"));
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.started.is_empty(), "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::NeedsYou);
}

#[tokio::test]
async fn a_check_that_cannot_run_counts_as_a_failure() {
    let s = Sched::new().await;
    let name = verifying(&s, "definitely-not-a-command-xyz", 0).await;
    let task = judged(&s, &name).await;
    assert_eq!(task.state, TaskState::NeedsYou);
    let output = task.done.verify.unwrap().output.unwrap();
    assert!(output.contains("definitely-not-a-command-xyz"), "{output}");
}

#[tokio::test]
async fn a_task_without_a_check_is_left_alone() {
    let s = Sched::new().await;
    let name = unique("plain");
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    for _ in 0..3 {
        run_once(s.ctx()).await.unwrap();
    }
    assert_eq!(s.state(&name).await, TaskState::Working);
    assert_eq!(
        s.store()
            .find_task(&name)
            .await
            .unwrap()
            .unwrap()
            .done
            .verify,
        None
    );
}
