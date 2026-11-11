use std::time::Duration;

use townhall_core::model::{Done, NewTask, Task, TaskState, Verify, VerifyPhase};
use townhall_core::scheduler::run_once;
use townhall_core::Pacing;

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
    let told = s.ctx().notes().waiting(&s.conversation);
    let line = format!("task {name} needs you, its check `echo still-red; exit 1` still fails");
    assert!(told.iter().any(|t| t.starts_with(&line)), "{told:?}");
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.started.is_empty(), "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::NeedsYou);
}

#[tokio::test]
async fn each_retry_of_a_failing_check_waits_twice_as_long_as_the_one_before() {
    let s = Sched::new().await;
    let name = verifying(&s, "exit 1", 3).await;
    for retries in 0..3 {
        if retries > 0 {
            run_once(s.ctx()).await.unwrap();
            mark(&s, &name, retries).await;
        }
        let task = judged(&s, &name).await;
        assert_eq!(task.state, TaskState::Queued, "{task:?}");
    }
    let secs: Vec<u64> = s.slept().iter().map(Duration::as_secs).collect();
    assert_eq!(secs, [60, 120, 240]);
}

#[tokio::test]
async fn the_wait_between_retries_stops_growing_at_the_cap() {
    let s = Sched::new().await;
    let pacing = Pacing {
        check_backoff: Duration::from_secs(400),
        ..s.ctx().pacing()
    };
    s.ctx().set_pacing(pacing);
    let name = verifying(&s, "exit 1", 3).await;
    for retries in 0..2 {
        if retries > 0 {
            run_once(s.ctx()).await.unwrap();
            mark(&s, &name, retries).await;
        }
        judged(&s, &name).await;
    }
    let secs: Vec<u64> = s.slept().iter().map(Duration::as_secs).collect();
    assert_eq!(secs, [400, 600]);
}

#[tokio::test]
async fn the_last_failure_does_not_wait_before_it_needs_the_user() {
    let s = Sched::new().await;
    let name = verifying(&s, "exit 1", 0).await;
    let task = judged(&s, &name).await;
    assert_eq!(task.state, TaskState::NeedsYou);
    assert!(s.slept().is_empty(), "{:?}", s.slept());
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

async fn install_check_hook(s: &Sched, answer: &str) {
    use townhall_core::store::{Marketplace, Plugin};
    let store = s.store();
    let source = serde_json::json!({"source": "github", "repo": "acme/market"});
    let market = Marketplace {
        name: "acme".into(),
        source,
        account: None,
        enabled: true,
        official: false,
        head_sha: None,
        fetched_at: None,
        error: None,
    };
    store.add_marketplace(market).await.unwrap();
    let plugin = Plugin {
        id: String::new(),
        marketplace: "acme".into(),
        name: "lint".into(),
        version: Some("1.0.0".into()),
        sha: "abc123".into(),
        source: serde_json::json!({"source": "git-subdir", "path": "plugins/lint"}),
        enabled: true,
        pinned: false,
        permissions_hash: "sha256:aa".into(),
        approved: serde_json::json!(["hook:task.check"]),
        installed_at: String::new(),
        updated_at: String::new(),
        error: None,
    };
    store.add_plugin(plugin).await.unwrap();
    let plugins = s.ws.home().plugins_dir().unwrap();
    let root = plugins.join("cache/acme/lint/abc123");
    std::fs::create_dir_all(root.join(".townhall-plugin")).unwrap();
    let manifest = serde_json::json!({
        "name": "lint", "version": "1.0.0", "apiVersion": 1,
        "permissions": {"hooks": ["task.check"]},
        "contributes": {"hooks": "townhall-hooks.json"}
    });
    let file = root.join(".townhall-plugin/plugin.json");
    std::fs::write(file, manifest.to_string()).unwrap();
    let hooks = serde_json::json!({"hooks": {"task.check": [{"command": answer}]}});
    std::fs::write(root.join("townhall-hooks.json"), hooks.to_string()).unwrap();
    std::fs::create_dir_all(plugins.join("data/acme/lint")).unwrap();
}

#[tokio::test]
async fn a_check_a_plugin_added_fails_the_verify_loop_like_the_own_check() {
    let s = Sched::new().await;
    let add = r#"echo '{"checks":["echo lint-red; exit 4"]}'"#;
    install_check_hook(&s, add).await;
    let name = verifying(&s, "touch own-ran", 2).await;
    let task = judged(&s, &name).await;
    assert!(s.worktree(&name).join("own-ran").exists());
    assert_eq!(task.state, TaskState::Queued, "{task:?}");
    let verify = task.done.verify.unwrap();
    assert_eq!((verify.phase, verify.retries), (VerifyPhase::Retrying, 1));
    let output = verify.output.unwrap();
    assert!(
        output.contains("lint-red") && output.contains("exit code 4"),
        "{output}"
    );
}

#[tokio::test]
async fn a_check_a_plugin_added_that_passes_ends_the_task() {
    let s = Sched::new().await;
    install_check_hook(&s, r#"echo '{"checks":["touch added-ran"]}'"#).await;
    let name = verifying(&s, "true", 2).await;
    let task = judged(&s, &name).await;
    assert_eq!(task.state, TaskState::Ended);
    assert!(s.worktree(&name).join("added-ran").exists());
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
