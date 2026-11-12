use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use townhall_core::model::{Checks, NewTask, Pr, SettingKey, Task, TaskState};
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
case "$*" in
  *mergeCommit*) echo "feedface"; exit 0;;
  "pr view"*) if [ -f "$here/$task.view" ]; then cat "$here/$task.view"; else echo '{"headRefOid":"abc123","mergeable":"MERGEABLE"}'; fi; exit 0;;
  "api "*) if [ -f "$here/$task.behind" ]; then cat "$here/$task.behind"; else echo 0; fi; exit 0;;
  "pr merge"*) exit 0;;
esac
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

fn merges(task: &str) -> Vec<String> {
    let log = gh_dir().join("tasks").join(format!("{task}.log"));
    let text = std::fs::read_to_string(log).unwrap_or_default();
    text.lines()
        .filter(|l| l.starts_with("pr merge"))
        .map(str::to_string)
        .collect()
}

fn says(task: &str, file: &str, text: &str) {
    std::fs::write(gh_dir().join("tasks").join(format!("{task}.{file}")), text).unwrap();
}

async fn lander(s: &Sched, json: &str) -> String {
    landing(s, json, "plain").await
}

async fn landing(s: &Sched, json: &str, agent: &str) -> String {
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
    if agent != "plain" {
        let source = format!("name = \"{agent}\"\nadapter = \"claude\"\n");
        s.store().save_agent_type(&source).await.unwrap();
    }
    let new = NewTask {
        agent: agent.into(),
        ..s.new_task(&name, &[])
    };
    s.store()
        .create_task(&s.conversation, new)
        .await
        .unwrap();
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.state(&name).await, TaskState::Working);
    let pr = Pr {
        number: 7,
        url: Some("https://github.com/acme/shop/pull/7".into()),
        checks: Checks::Pending,
        merge_sha: None,
    };
    let change = move |t: Task| Ok(Task { pr: Some(pr), ..t });
    s.store().update_task(&name, change).await.unwrap();
    s.end(&name).await;
    name
}

async fn until(s: &Sched, name: &str, want: impl Fn(&Task) -> bool) -> Task {
    let give_up = Instant::now() + Duration::from_secs(120);
    while Instant::now() < give_up {
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

async fn relaunches(s: &Sched, name: &str) -> Vec<String> {
    let give_up = Instant::now() + Duration::from_secs(120);
    let mut started = vec![];
    while Instant::now() < give_up {
        let pass = run_once(s.ctx()).await.unwrap();
        started.extend(pass.started.into_iter().filter(|n| n == name));
        let task = s.store().find_task(name).await.unwrap().unwrap();
        if task.state == TaskState::Working {
            return started;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("`{name}` was never relaunched: {started:?}");
}

async fn heard(s: &Sched) -> Vec<String> {
    let give_up = Instant::now() + Duration::from_secs(120);
    while Instant::now() < give_up {
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
    assert_eq!(s.state(&name).await, TaskState::InCi);
    assert_eq!(session(&s, &name).await, before);
    assert!(s.store().handoffs(&name).await.unwrap().is_empty());
    let give_up = Instant::now() + Duration::from_secs(120);
    while asked(&name).len() < 2 && Instant::now() < give_up {
        passes(&s, 1).await;
    }
    assert!(asked(&name).len() >= 2, "{:?}", asked(&name));

    reports(&name, RED);
    let started = relaunches(&s, &name).await;
    assert_eq!(started, std::slice::from_ref(&name), "{started:?}");
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
    relaunches(&s, &name).await;
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

#[tokio::test]
async fn a_lander_whose_checks_are_green_is_merged_by_the_daemon() {
    let s = Sched::new().await;
    let name = landing(&s, GREEN, "lander").await;
    let before = session(&s, &name).await;
    let task = until(&s, &name, |t| t.state == TaskState::Merged).await;
    let pr = task.pr.unwrap();
    assert_eq!(pr.merge_sha.as_deref(), Some("feedface"));
    assert_eq!(pr.checks, Checks::Passed);
    assert_eq!(merges(&name).len(), 1, "{:?}", merges(&name));
    let merge = &merges(&name)[0];
    for needle in ["pr merge 7", "--merge", "--match-head-commit abc123", "GH_TOKEN="] {
        assert!(merge.contains(needle), "{needle}: {merge}");
    }
    assert!(!merge.contains("--auto") && !merge.contains("--squash"), "{merge}");
    assert_eq!(session(&s, &name).await, before, "no new agent run");
    assert!(s.store().handoffs(&name).await.unwrap().is_empty());
    let told = heard(&s).await;
    assert!(
        told.iter()
            .any(|t| t.contains(&name) && t.contains("merged") && t.contains("feedface")),
        "{told:?}"
    );
    passes(&s, 3).await;
    assert_eq!(merges(&name).len(), 1);
}

#[tokio::test]
async fn a_lander_with_pending_checks_is_never_merged() {
    let s = Sched::new().await;
    let name = landing(&s, PENDING, "lander").await;
    until(&s, &name, |t| t.state == TaskState::InCi).await;
    passes(&s, 4).await;
    assert_eq!(s.state(&name).await, TaskState::InCi);
    assert!(merges(&name).is_empty());
}

#[tokio::test]
async fn a_lander_waits_in_ci_and_is_merged_once_the_checks_turn_green() {
    let s = Sched::new().await;
    let name = landing(&s, PENDING, "lander").await;
    until(&s, &name, |t| t.state == TaskState::InCi).await;
    reports(&name, GREEN);
    let task = until(&s, &name, |t| t.state == TaskState::Merged).await;
    assert_eq!(task.pr.unwrap().merge_sha.as_deref(), Some("feedface"));
    assert!(s.store().handoffs(&name).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_lander_with_a_failed_check_is_relaunched_with_its_name_and_not_merged() {
    let s = Sched::new().await;
    let name = landing(&s, RED, "lander").await;
    let before = session(&s, &name).await;
    let started = relaunches(&s, &name).await;
    assert_eq!(started, std::slice::from_ref(&name));
    assert_ne!(session(&s, &name).await, before);
    let brief = std::fs::read_to_string(s.brief(&name)).unwrap();
    assert!(brief.contains("build: FAILURE"), "{brief}");
    assert!(merges(&name).is_empty());
    assert_eq!(s.store().handoffs(&name).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_lander_behind_its_base_is_relaunched_and_not_merged() {
    let s = Sched::new().await;
    let name = landing(&s, GREEN, "lander").await;
    says(&name, "behind", "3\n");
    relaunches(&s, &name).await;
    let brief = std::fs::read_to_string(s.brief(&name)).unwrap();
    assert!(brief.contains("behind") && brief.contains("main"), "{brief}");
    assert!(merges(&name).is_empty());
    assert_ne!(s.state(&name).await, TaskState::Merged);
}

#[tokio::test]
async fn a_lander_whose_pr_conflicts_is_relaunched_and_not_merged() {
    let s = Sched::new().await;
    let name = landing(&s, GREEN, "lander").await;
    says(&name, "view", r#"{"headRefOid":"abc123","mergeable":"CONFLICTING"}"#);
    relaunches(&s, &name).await;
    assert!(merges(&name).is_empty());
}

#[tokio::test]
async fn a_task_of_another_type_with_green_checks_is_not_merged_by_the_daemon() {
    let s = Sched::new().await;
    let name = lander(&s, GREEN).await;
    until(&s, &name, |t| {
        t.pr.as_ref().is_some_and(|p| p.checks == Checks::Passed)
    })
    .await;
    passes(&s, 3).await;
    assert_eq!(s.state(&name).await, TaskState::Ended);
    assert!(merges(&name).is_empty());
}

#[tokio::test]
async fn a_type_can_opt_in_to_merging_on_green_by_its_flag() {
    let s = Sched::new().await;
    let source = "name = \"shipper\"\nadapter = \"claude\"\nmerges_on_green = true\n";
    s.store().save_agent_type(source).await.unwrap();
    let name = landing(&s, GREEN, "shipper").await;
    until(&s, &name, |t| t.state == TaskState::Merged).await;
}
