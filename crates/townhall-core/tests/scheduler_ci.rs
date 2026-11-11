use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use townhall_core::model::{Checks, Pr, SettingKey, Task, TaskState};
use townhall_core::scheduler::run_once;
use townhall_core::Pacing;

mod scheduler_support;
use scheduler_support::{git, unique, Sched};

const PENDING: &str = r#"[{"name":"build","state":"IN_PROGRESS","bucket":"pending"},{"name":"lint","state":"SUCCESS","bucket":"pass"}]"#;
const GREEN: &str = r#"[{"name":"build","state":"SUCCESS","bucket":"pass"},{"name":"lint","state":"SUCCESS","bucket":"pass"}]"#;
const RED: &str = r#"[{"name":"build","state":"FAILURE","bucket":"fail"},{"name":"lint","state":"SUCCESS","bucket":"pass"}]"#;

fn gh_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("townhall-ci-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("tasks")).unwrap();
        let gh = dir.join("gh");
        let script = r#"#!/bin/sh
task="$(basename "$PWD")"
here="$(dirname "$0")/tasks"
echo "$@ GH_TOKEN=$GH_TOKEN" >> "$here/$task.log"
if [ "$1 $2" = "auth token" ]; then echo "tok-$4"; exit 0; fi
if [ "$1 $2" = "pr checks" ]; then cat "$here/$task.json"; exit 0; fi
exit 1
"#;
        std::fs::write(&gh, script).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("TOWNHALL_GH_BIN", &gh);
        dir
    })
}

fn reports(task: &str, json: &str) {
    std::fs::write(gh_dir().join("tasks").join(format!("{task}.json")), json).unwrap();
}

fn asked(task: &str) -> Vec<String> {
    let log = gh_dir().join("tasks").join(format!("{task}.log"));
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.lines()
        .filter(|l| l.starts_with("pr checks"))
        .map(str::to_string)
        .collect()
}

async fn lander(s: &Sched, json: &str) -> String {
    gh_dir();
    git(
        &s.repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/shop.git",
        ],
    )
    .await;
    s.ctx().set_pacing(Pacing {
        ci_poll: Duration::ZERO,
        ..s.ctx().pacing()
    });
    let name = unique("lander");
    reports(&name, json);
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.state(&name).await, TaskState::Working);
    let pr = Pr {
        number: 7,
        url: Some("https://github.com/acme/shop/pull/7".into()),
        checks: Checks::Pending,
    };
    let change = move |t: Task| Ok(Task { pr: Some(pr), ..t });
    s.store().update_task(&name, change).await.unwrap();
    s.end(&name).await;
    name
}

async fn until(s: &Sched, name: &str, want: impl Fn(&Task) -> bool) -> Task {
    for _ in 0..100 {
        run_once(s.ctx()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let task = s.store().find_task(name).await.unwrap().unwrap();
        if want(&task) {
            return task;
        }
    }
    let task = s.store().find_task(name).await.unwrap().unwrap();
    panic!("`{name}` never got there: {task:?}");
}

async fn floor(s: &Sched, gb: u32) {
    let put = s
        .store()
        .set_setting(SettingKey::MinFreeDiskGb, serde_json::json!(gb));
    put.await.unwrap();
}

async fn queued(s: &Sched, name: &str) -> Task {
    floor(s, 100_000).await;
    let task = until(s, name, |t| t.state == TaskState::Queued).await;
    floor(s, 0).await;
    task
}

async fn polled(s: &Sched, name: &str, times: usize) {
    for _ in 0..100 {
        if asked(name).len() >= times {
            return;
        }
        run_once(s.ctx()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("`{name}` was not polled {times} times: {:?}", asked(name));
}

async fn heard(s: &Sched) -> Vec<String> {
    for _ in 0..100 {
        let told = s.ctx().notes().waiting(&s.conversation);
        if !told.is_empty() {
            return told;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("nothing told to the conversation");
}

async fn passes(s: &Sched, count: usize) {
    for _ in 0..count {
        run_once(s.ctx()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn session(s: &Sched, name: &str) -> Option<String> {
    s.store()
        .task_launch(name)
        .await
        .unwrap()
        .unwrap()
        .session_id
}

#[tokio::test]
async fn a_task_that_ended_with_pending_checks_waits_then_is_relaunched_once() {
    let s = Sched::new().await;
    let name = lander(&s, PENDING).await;
    let before = session(&s, &name).await;
    until(&s, &name, |t| t.state == TaskState::InCi).await;
    passes(&s, 4).await;
    polled(&s, &name, 2).await;
    assert_eq!(s.state(&name).await, TaskState::InCi);
    assert_eq!(session(&s, &name).await, before);
    assert!(s.store().handoffs(&name).await.unwrap().is_empty());
    assert!(asked(&name).len() >= 2, "{:?}", asked(&name));

    reports(&name, RED);
    queued(&s, &name).await;
    let pass = run_once(s.ctx()).await.unwrap();
    assert_eq!(pass.started, std::slice::from_ref(&name), "{pass:?}");
    assert_eq!(s.state(&name).await, TaskState::Working);
    assert_ne!(session(&s, &name).await, before, "a new agent run");
    let brief = std::fs::read_to_string(s.brief(&name)).unwrap();
    for needle in [
        "CI finished: merge if green, fix if red",
        "build: FAILURE",
        "lint: SUCCESS",
        "#7",
        &format!("the brief for {name}"),
    ] {
        assert!(brief.contains(needle), "{needle}\n{brief}");
    }
    let handoffs = s.store().handoffs(&name).await.unwrap();
    assert_eq!(handoffs.len(), 1, "{handoffs:?}");
    assert_eq!(
        (handoffs[0].from.as_str(), handoffs[0].to.as_str()),
        ("plain", "plain")
    );
    let task = s.store().find_task(&name).await.unwrap().unwrap();
    assert_eq!(task.pr.unwrap().checks, Checks::Failed);

    passes(&s, 3).await;
    assert_eq!(s.state(&name).await, TaskState::Working);
    assert_eq!(s.store().handoffs(&name).await.unwrap().len(), 1);
}

#[tokio::test]
async fn the_wait_is_in_the_thread() {
    let s = Sched::new().await;
    let name = lander(&s, PENDING).await;
    until(&s, &name, |t| t.state == TaskState::InCi).await;
    let entries = s.store().entries(&s.conversation).await.unwrap();
    let said = format!("{:?}", entries.last().unwrap().body);
    assert!(
        said.contains("waiting for CI") && said.contains("build: IN_PROGRESS"),
        "{said}"
    );
}

#[tokio::test]
async fn checks_are_read_as_the_registered_account() {
    let s = Sched::new().await;
    s.store()
        .set_setting(SettingKey::GithubAccount, serde_json::json!("ci-bot"))
        .await
        .unwrap();
    let name = lander(&s, PENDING).await;
    until(&s, &name, |t| t.state == TaskState::InCi).await;
    let asks = asked(&name);
    assert!(
        asks[0].starts_with("pr checks 7 --json name,state,bucket")
            && asks[0].ends_with("GH_TOKEN=tok-ci-bot"),
        "{asks:?}"
    );
}

#[tokio::test]
async fn the_checks_are_read_no_more_often_than_the_poll_interval() {
    let s = Sched::new().await;
    let name = lander(&s, PENDING).await;
    s.ctx().set_pacing(Pacing {
        ci_poll: Duration::from_secs(3600),
        ..s.ctx().pacing()
    });
    until(&s, &name, |t| t.state == TaskState::InCi).await;
    passes(&s, 5).await;
    assert_eq!(asked(&name).len(), 1, "{:?}", asked(&name));
}

#[tokio::test]
async fn a_task_without_a_pr_is_left_alone() {
    let s = Sched::new().await;
    let name = lander(&s, PENDING).await;
    let clear = |t: Task| Ok(Task { pr: None, ..t });
    s.store().update_task(&name, clear).await.unwrap();
    passes(&s, 4).await;
    assert_eq!(s.state(&name).await, TaskState::Ended);
    assert!(asked(&name).is_empty());
}

#[tokio::test]
async fn a_pr_whose_checks_are_all_done_does_not_relaunch_the_task() {
    for (json, checks) in [
        (GREEN, Checks::Passed),
        (RED, Checks::Failed),
        ("[]", Checks::None),
    ] {
        let s = Sched::new().await;
        let name = lander(&s, json).await;
        let before = session(&s, &name).await;
        until(&s, &name, |t| {
            t.pr.as_ref().is_some_and(|p| p.checks != Checks::Pending)
        })
        .await;
        passes(&s, 3).await;
        let task = s.store().find_task(&name).await.unwrap().unwrap();
        assert_eq!(task.state, TaskState::Ended, "{json}");
        assert_eq!(task.pr.unwrap().checks, checks, "{json}");
        assert_eq!(session(&s, &name).await, before);
        assert!(s.store().handoffs(&name).await.unwrap().is_empty());
        assert_eq!(asked(&name).len(), 1, "settled checks are not read again");
    }
}

#[tokio::test]
async fn a_task_that_keeps_ending_with_pending_checks_needs_you_after_the_last_wait() {
    let s = Sched::new().await;
    s.ctx().set_pacing(Pacing {
        ci_waits: 1,
        ..s.ctx().pacing()
    });
    let name = lander(&s, PENDING).await;
    until(&s, &name, |t| t.state == TaskState::InCi).await;
    reports(&name, GREEN);
    queued(&s, &name).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.state(&name).await, TaskState::Working);

    reports(&name, PENDING);
    let change = |t: Task| {
        let pr = t.pr.map(|p| Pr {
            checks: Checks::Pending,
            ..p
        });
        Ok(Task { pr, ..t })
    };
    s.store().update_task(&name, change).await.unwrap();
    s.end(&name).await;
    let task = until(&s, &name, |t| t.state == TaskState::NeedsYou).await;
    assert!(
        task.blocked
            .as_deref()
            .is_some_and(|b| b.contains("build: IN_PROGRESS")),
        "{task:?}"
    );
    let told = heard(&s).await;
    let line = format!("task {name} needs you");
    assert!(
        told.iter()
            .any(|t| t.starts_with(&line) && t.contains("build: IN_PROGRESS")),
        "{told:?}"
    );

    passes(&s, 3).await;
    assert_eq!(s.state(&name).await, TaskState::NeedsYou);
    assert_eq!(s.store().handoffs(&name).await.unwrap().len(), 1);
}

#[tokio::test]
async fn checks_that_stay_pending_past_the_timeout_need_you_with_their_states() {
    let s = Sched::new().await;
    let name = lander(&s, PENDING).await;
    s.ctx().set_pacing(Pacing {
        ci_timeout: Duration::from_secs(3600),
        ..s.ctx().pacing()
    });
    until(&s, &name, |t| t.state == TaskState::InCi).await;
    passes(&s, 2).await;
    assert_eq!(s.state(&name).await, TaskState::InCi);

    s.ctx().set_pacing(Pacing {
        ci_timeout: Duration::ZERO,
        ..s.ctx().pacing()
    });
    let task = until(&s, &name, |t| t.state == TaskState::NeedsYou).await;
    let why = task.blocked.unwrap();
    assert!(
        why.contains("build: IN_PROGRESS") && why.contains("lint: SUCCESS"),
        "{why}"
    );
    assert!(why.contains("#7"), "{why}");
    assert!(s.store().handoffs(&name).await.unwrap().is_empty());
}
