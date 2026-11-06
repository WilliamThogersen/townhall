use std::time::Duration;

use serde_json::json;
use townhall_core::model::{AgentEvent, Conversation, EntryBody, SettingKey, TaskState};
use townhall_core::reaper::{self, CANCELLED};
use townhall_core::runner;
use townhall_core::scheduler::run_once;

mod scheduler_support;
use scheduler_support::{commit, git, sleeper, unique, Sched};

/// Ending hands the reaping to the background: waits until no agent of `names` is left.
async fn wait_gone(s: &Sched, names: &[&str]) {
    for _ in 0..200 {
        let left = s.agent_names().await;
        if names.iter().all(|n| !left.iter().any(|l| l == n)) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("agents {names:?} were never removed");
}

async fn has_branch(s: &Sched, name: &str) -> bool {
    let branch = format!("townhall/{name}");
    !git(&s.repo, &["branch", "--list", &branch])
        .await
        .is_empty()
}

async fn merge(s: &Sched, name: &str) {
    let branch = format!("townhall/{name}");
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
        branch.as_str(),
    ];
    git(&s.repo, &args).await;
}

fn sorted(names: &[&str]) -> Vec<String> {
    let mut all: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    all.sort();
    all
}

#[tokio::test]
async fn ending_removes_its_agents_and_cancels_its_queued_tasks() {
    let s = Sched::new().await;
    let (running, waiting) = (unique("running"), unique("waiting"));
    s.create(&running, &[]).await;
    s.create(&waiting, &[running.as_str()]).await;
    assert_eq!(run_once(s.ctx()).await.unwrap().started, [running.as_str()]);
    assert!(s.worktree(&running).exists());

    let ended = runner::end(s.ctx(), &s.conversation).await.unwrap();
    assert!(ended.ended);
    assert_eq!(
        s.state(&waiting).await,
        TaskState::Ended,
        "a queued task is cancelled before end answers"
    );
    let entries = s.store().entries(&s.conversation).await.unwrap();
    let cancelled = entries.iter().any(|e| {
        matches!(&e.body, EntryBody::Agent { agent, event: AgentEvent::Ended, text }
            if agent == &waiting && text == CANCELLED)
    });
    assert!(cancelled, "{entries:?}");

    wait_gone(&s, &[running.as_str()]).await;
    assert!(
        !s.worktree(&running).exists(),
        "the worktree folder is gone"
    );
    let pass = run_once(s.ctx()).await.unwrap();
    assert!(pass.started.is_empty(), "nothing starts again: {pass:?}");
    assert!(s.agent_names().await.is_empty());
}

#[tokio::test]
async fn a_branch_is_kept_only_while_it_holds_unmerged_commits() {
    let s = Sched::new().await;
    let (merged, unmerged) = (unique("merged"), unique("unmerged"));
    s.create(&merged, &[]).await;
    s.create(&unmerged, &[]).await;
    run_once(s.ctx()).await.unwrap();
    commit(&s.worktree(&merged), "merged work").await;
    commit(&s.worktree(&unmerged), "work that exists nowhere else").await;
    merge(&s, &merged).await;

    runner::end(s.ctx(), &s.conversation).await.unwrap();
    wait_gone(&s, &[merged.as_str(), unmerged.as_str()]).await;
    assert!(!has_branch(&s, &merged).await, "a merged branch is deleted");
    assert!(
        has_branch(&s, &unmerged).await,
        "an unmerged branch keeps its commits"
    );
}

#[tokio::test]
async fn a_pass_reaps_what_is_done_or_orphaned_and_keeps_live_work() {
    let s = Sched::new().await;
    let (old, waiting) = (unique("old"), unique("waiting"));
    s.create(&old, &[]).await;
    s.create(&waiting, &[old.as_str()]).await;
    let other = s.store().create_conversation().await.unwrap().id;
    let (live, shipped) = (unique("live"), unique("shipped"));
    for name in [&live, &shipped] {
        let new = s.new_task(name, &[]);
        s.store().create_task(&other, new).await.unwrap();
    }
    run_once(s.ctx()).await.unwrap();
    s.set_state(&shipped, TaskState::Merged).await;
    let stray = unique("stray");
    sleeper(&s.ws, &stray).await;
    // Ended before the reaper existed: only the flag is set, its agents still run.
    let flag = |c: Conversation| Ok(Conversation { ended: true, ..c });
    s.store()
        .update_conversation(&s.conversation, flag)
        .await
        .unwrap();

    let reaped = reaper::run_once(s.ctx()).await.unwrap();
    assert!(reaped.warnings.is_empty(), "{reaped:?}");
    assert_eq!(reaped.cancelled, [waiting.as_str()]);
    let mut removed = reaped.removed.clone();
    removed.sort();
    assert_eq!(
        removed,
        sorted(&[old.as_str(), shipped.as_str(), stray.as_str()])
    );
    assert_eq!(s.agent_names().await, [live.as_str()], "live work is kept");
    assert_eq!(s.state(&live).await, TaskState::Working);

    let again = reaper::run_once(s.ctx()).await.unwrap();
    assert_eq!(again, reaper::Reaped::default(), "nothing is left");
}

#[tokio::test]
async fn a_session_whose_worktree_is_gone_is_killed() {
    let s = Sched::new().await;
    let ghost = unique("ghost");
    sleeper(&s.ws, &ghost).await;
    let path = s.worktree(&ghost).to_string_lossy().into_owned();
    git(&s.repo, &["worktree", "remove", "--force", &path]).await;
    let session = townhall_core::tmux::session_name(s.ws.key(), &ghost);
    let has_session = || async {
        townhall_core::tmux::panes(s.ws.key())
            .await
            .unwrap()
            .iter()
            .any(|p| p.session == session)
    };
    assert!(has_session().await, "the session outlived its worktree");

    let reaped = reaper::run_once(s.ctx()).await.unwrap();
    assert_eq!(reaped.sessions, [ghost.as_str()]);
    assert!(!has_session().await, "the session is killed");
}

const GB: u64 = 1_000_000_000;

/// A file that counts `bytes` towards a folder's size without using the disk.
fn sparse(path: &std::path::Path, bytes: u64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::File::create(path).unwrap().set_len(bytes).unwrap();
}

async fn cap(s: &Sched, gb: u32) {
    let put = s
        .store()
        .set_setting(SettingKey::SharedTargetMaxGb, json!(gb));
    put.await.unwrap();
}

#[tokio::test]
async fn a_shared_target_over_its_cap_loses_its_incremental_caches_first() {
    let s = Sched::new().await;
    cap(&s, 2).await;
    let target = s.ws.cargo_target_dir();
    sparse(
        &target.join("debug/incremental/crate-a/dep-graph.bin"),
        2 * GB,
    );
    sparse(&target.join("debug/deps/libserde.rlib"), GB);

    let reaped = reaper::run_once(s.ctx()).await.unwrap();
    assert_eq!(reaped.trimmed, [s.repo_name.as_str()], "{reaped:?}");
    assert!(!target.join("debug/incremental").exists());
    assert!(
        target.join("debug/deps/libserde.rlib").exists(),
        "the compiled dependencies are what worktrees share"
    );
    let again = reaper::run_once(s.ctx()).await.unwrap();
    assert_eq!(
        again,
        reaper::Reaped::default(),
        "under the cap, nothing goes"
    );
}

#[tokio::test]
async fn a_shared_target_still_over_its_cap_is_emptied_when_nobody_builds() {
    let s = Sched::new().await;
    cap(&s, 2).await;
    let target = s.ws.cargo_target_dir();
    sparse(&target.join("debug/deps/libbig.rlib"), 3 * GB);

    let reaped = reaper::run_once(s.ctx()).await.unwrap();
    assert_eq!(reaped.trimmed, [s.repo_name.as_str()], "{reaped:?}");
    assert!(target.exists(), "the folder stays for the next build");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
}

#[tokio::test]
async fn a_shared_target_is_left_alone_while_an_agent_of_the_repo_runs() {
    let s = Sched::new().await;
    cap(&s, 2).await;
    s.create(&unique("building"), &[]).await;
    run_once(s.ctx()).await.unwrap();
    let target = s.ws.cargo_target_dir();
    let big = target.join("debug/deps/libbig.rlib");
    sparse(&big, 3 * GB);

    let reaped = reaper::run_once(s.ctx()).await.unwrap();
    assert!(reaped.trimmed.is_empty(), "{reaped:?}");
    assert!(big.exists());
    let warned = reaped.warnings.join("; ");
    assert!(
        warned.contains("shared target") && warned.contains(&s.repo_name),
        "{warned}"
    );
}

#[tokio::test]
async fn a_cap_of_zero_never_trims_the_shared_target() {
    let s = Sched::new().await;
    cap(&s, 0).await;
    let big = s.ws.cargo_target_dir().join("debug/deps/libbig.rlib");
    sparse(&big, 60 * GB);
    let reaped = reaper::run_once(s.ctx()).await.unwrap();
    assert_eq!(reaped, reaper::Reaped::default());
    assert!(big.exists());
}

#[tokio::test]
async fn a_worktrees_own_target_goes_with_the_worktree() {
    let s = Sched::new().await;
    let name = unique("leftovers");
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    let own = s.worktree(&name).join("target/debug/deps/libold.rlib");
    sparse(&own, 1_000_000);
    runner::end(s.ctx(), &s.conversation).await.unwrap();
    wait_gone(&s, &[name.as_str()]).await;
    assert!(!s.worktree(&name).join("target").exists());
    assert!(!s.worktree(&name).exists());
}
