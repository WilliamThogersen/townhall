use std::time::Duration;

use townhall_core::model::{AgentEvent, Conversation, EntryBody, TaskState};
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

fn fill(worktree: &std::path::Path) {
    for file in ["target/debug/app", "node_modules/dep/index.js"] {
        let path = worktree.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "artifact").unwrap();
    }
}

#[tokio::test]
async fn a_finished_agent_loses_its_artifacts_but_keeps_its_worktree() {
    let s = Sched::new().await;
    std::fs::write(s.repo.join(".gitignore"), "target\nnode_modules\n").unwrap();
    git(&s.repo, &["add", ".gitignore"]).await;
    commit(&s.repo, "ignore artifacts").await;
    let (live, done, review) = (unique("live"), unique("done"), unique("review"));
    for name in [&live, &done, &review] {
        s.create(name, &[]).await;
    }
    run_once(s.ctx()).await.unwrap();
    for name in [&live, &done, &review] {
        fill(&s.worktree(name));
    }
    std::fs::write(s.worktree(&done).join("notes.txt"), "uncommitted").unwrap();
    s.end(&done).await;
    s.set_state(&review, TaskState::InReview).await;
    townhall_core::tmux::kill(s.ws.key(), &review)
        .await
        .unwrap();

    let reaped = reaper::run_once(s.ctx()).await.unwrap();

    assert!(reaped.warnings.is_empty(), "{reaped:?}");
    assert!(reaped.removed.is_empty(), "{reaped:?}");
    assert_eq!(reaped.purged, sorted(&[done.as_str(), review.as_str()]));
    for name in [&done, &review] {
        assert!(!s.worktree(name).join("target").exists(), "{name}");
        assert!(!s.worktree(name).join("node_modules").exists(), "{name}");
        assert!(s.worktree(name).join(".git").exists(), "{name} is kept");
    }
    assert!(s.worktree(&done).join("notes.txt").exists());
    assert!(s.worktree(&live).join("target").exists(), "live work");
    assert!(s.worktree(&live).join("node_modules").exists(), "live work");

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

#[tokio::test]
async fn an_ended_task_of_an_open_conversation_is_reaped_after_the_grace() {
    let s = Sched::new().await;
    let (done, live) = (unique("done"), unique("live"));
    s.create(&done, &[]).await;
    s.create(&live, &[]).await;
    run_once(s.ctx()).await.unwrap();
    commit(&s.worktree(&done), "work that exists nowhere else").await;
    std::fs::write(s.worktree(&done).join("scratch.txt"), "uncommitted").unwrap();
    s.set_state(&done, TaskState::Ended).await;

    let early = reaper::run_once(s.ctx()).await.unwrap();
    assert_eq!(early, reaper::Reaped::default(), "inside the grace period");
    assert!(s.worktree(&done).exists());

    let late = reaper::run_once_after(s.ctx(), Duration::ZERO)
        .await
        .unwrap();
    assert!(late.warnings.is_empty(), "{late:?}");
    assert_eq!(late.removed, [done.as_str()]);
    assert!(!s.worktree(&done).exists(), "a dirty worktree goes too");
    assert!(
        has_branch(&s, &done).await,
        "unmerged commits keep their branch"
    );
    assert_eq!(s.agent_names().await, [live.as_str()], "live work is kept");
}

#[tokio::test]
async fn folders_git_does_not_know_are_swept_but_live_work_is_kept() {
    let s = Sched::new().await;
    let (live, lost, leftover) = (unique("live"), unique("lost"), unique("leftover"));
    s.create(&live, &[]).await;
    run_once(s.ctx()).await.unwrap();
    let dir = s.ws.worktrees_dir();
    std::fs::create_dir_all(dir.join(&leftover).join("node_modules")).unwrap();
    s.create(&lost, &[]).await;
    std::fs::create_dir_all(dir.join(&lost)).unwrap();
    std::fs::write(dir.join(&lost).join("keep.txt"), "mine").unwrap();

    let early = reaper::run_once(s.ctx()).await.unwrap();
    assert!(early.removed.is_empty(), "{early:?}");
    assert!(
        dir.join(&leftover).exists(),
        "too young to be sure it is stray"
    );

    let late = reaper::run_once_after(s.ctx(), Duration::ZERO)
        .await
        .unwrap();
    assert!(late.warnings.is_empty(), "{late:?}");
    assert_eq!(late.removed, [leftover.as_str()]);
    assert!(!dir.join(&leftover).exists());
    assert!(
        dir.join(&lost).join("keep.txt").exists(),
        "a live task's folder stays"
    );
    assert!(s.worktree(&live).exists());
}
