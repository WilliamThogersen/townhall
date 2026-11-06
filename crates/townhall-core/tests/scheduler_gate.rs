//! The scheduler's gate on an agent type's `requires`: a gated task starts only once each
//! required task ended with a report that clears it for the commit it will start from.

use std::path::Path;
use std::sync::OnceLock;

use serde_json::json;
use tempfile::TempDir;
use townhall_core::model::{ArtifactKind, AuthorRole, NewArtifact, NewTask, Task, TaskState};
use townhall_core::scheduler::run_once;
use townhall_core::steps::{project_dir_for, CLAUDE_PROJECTS_ENV};

mod scheduler_support;
use scheduler_support::{git, unique, Sched};

const TESTER: &str = "name = \"tester\"\nadapter = \"claude\"\n";
const REVIEWER: &str = "name = \"reviewer\"\nadapter = \"claude\"\n";
const SHIPPER: &str =
    "name = \"shipper\"\nadapter = \"claude\"\nrequires = [\"tester\", \"reviewer\"]\n";

static PROJECTS: OnceLock<TempDir> = OnceLock::new();

fn projects() -> &'static Path {
    PROJECTS
        .get_or_init(|| {
            let dir = TempDir::new().unwrap();
            std::env::set_var(CLAUDE_PROJECTS_ENV, dir.path());
            dir
        })
        .path()
}

struct Gated {
    s: Sched,
    tester: String,
    reviewer: String,
    ship: String,
}

async fn create_as(s: &Sched, name: &str, agent: &str, after: &[&str], base: &str) {
    let new = NewTask {
        agent: agent.into(),
        base_branch: base.into(),
        ..s.new_task(name, after)
    };
    s.store().create_task(&s.conversation, new).await.unwrap();
}

async fn gated_on(base: &str) -> Gated {
    projects();
    let s = Sched::new().await;
    for source in [TESTER, REVIEWER, SHIPPER] {
        s.store().save_agent_type(source).await.unwrap();
    }
    let (tester, reviewer, ship) = (unique("tester"), unique("reviewer"), unique("ship"));
    create_as(&s, &tester, "tester", &[], "main").await;
    create_as(&s, &reviewer, "reviewer", &[], "main").await;
    s.set_state(&tester, TaskState::Ended).await;
    s.set_state(&reviewer, TaskState::Ended).await;
    create_as(&s, &ship, "shipper", &[&tester, &reviewer], base).await;
    Gated {
        s,
        tester,
        reviewer,
        ship,
    }
}

async fn gated() -> Gated {
    gated_on("main").await
}

async fn rev(s: &Sched, rev: &str) -> String {
    git(&s.repo, &["rev-parse", rev]).await
}

fn clearing(verdict: &str, commit: &str) -> String {
    format!("Ran everything.\n\nVERDICT: {verdict}\nCOMMIT: {commit}\n")
}

fn transcript(last: &str) -> String {
    let line = json!({"type": "assistant", "timestamp": "2026-11-04T10:01:00.000Z",
        "message": {"id": "end", "role": "assistant", "stop_reason": "end_turn",
                    "content": [{"type": "text", "text": last}]}});
    format!("{line}\n")
}

async fn file_report(s: &Sched, task: &str, text: &str) {
    let dir = project_dir_for(projects(), &s.worktree(task));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("s.jsonl"), transcript(text)).unwrap();
}

async fn both_clear(g: &Gated, commit: &str) {
    file_report(&g.s, &g.tester, &clearing("PASS", commit)).await;
    file_report(&g.s, &g.reviewer, &clearing("APPROVE", commit)).await;
}

async fn blocked(g: &Gated) -> Option<String> {
    let task = g.s.store().find_task(&g.ship).await.unwrap().unwrap();
    assert_eq!(task.state, TaskState::Queued);
    task.blocked
}

async fn assert_starts(g: &Gated) {
    let pass = run_once(g.s.ctx()).await.unwrap();
    assert_eq!(pass.started, [g.ship.as_str()], "{pass:?}");
    let task = g.s.store().find_task(&g.ship).await.unwrap().unwrap();
    assert_eq!((task.state, task.blocked), (TaskState::Working, None));
}

async fn assert_blocked(g: &Gated, words: &[&str]) -> String {
    let pass = run_once(g.s.ctx()).await.unwrap();
    assert!(pass.started.is_empty(), "{pass:?}");
    let why = blocked(g).await.expect("a block reason");
    for word in words {
        assert!(why.contains(word), "`{word}` not in: {why}");
    }
    why
}

#[tokio::test]
async fn both_reports_clearing_the_head_let_the_task_start() {
    let g = gated().await;
    let head = rev(&g.s, "main").await;
    both_clear(&g, &head).await;
    assert_starts(&g).await;
}

#[tokio::test]
async fn a_task_without_requires_is_never_gated() {
    let g = gated().await;
    create_as(&g.s, &unique("plain"), "plain", &[], "main").await;
    let pass = run_once(g.s.ctx()).await.unwrap();
    assert_eq!(pass.started.len(), 1, "{pass:?}");
    assert_ne!(pass.started[0], g.ship);
}

#[tokio::test]
async fn no_report_keeps_the_task_queued_with_a_reason_and_wakes_the_planner() {
    let g = gated().await;
    let why = assert_blocked(&g, &[&g.tester, &g.reviewer, "no report"]).await;
    assert!(!why.contains('\n'), "{why}");
    assert!(g.s.ctx().notes().is_waiting(&g.s.conversation));
    assert!(g.s.agents().await.is_empty());
}

async fn assert_rejected(g: &Gated, words: &[&str]) -> String {
    let pass = run_once(g.s.ctx()).await.unwrap();
    assert_eq!(pass.failed, [g.ship.as_str()], "{pass:?}");
    assert!(pass.started.is_empty(), "{pass:?}");
    let task = g.s.store().find_task(&g.ship).await.unwrap().unwrap();
    assert_eq!((task.state, task.blocked), (TaskState::Error, None));
    let entries = g.s.store().entries(&g.s.conversation).await.unwrap();
    let text = format!("{entries:?}");
    let why = text
        .split("failed, it will not start")
        .nth(1)
        .expect("a failed entry");
    for word in words {
        assert!(why.contains(word), "`{word}` not in: {why}");
    }
    assert!(g.s.ctx().notes().is_waiting(&g.s.conversation));
    let again = run_once(g.s.ctx()).await.unwrap();
    assert!(
        again.failed.is_empty() && again.started.is_empty(),
        "{again:?}"
    );
    why.to_string()
}

#[tokio::test]
async fn a_fail_ends_the_task_with_the_reason_instead_of_blocking_forever() {
    let g = gated().await;
    let head = rev(&g.s, "main").await;
    file_report(&g.s, &g.tester, &clearing("FAIL", &head)).await;
    file_report(&g.s, &g.reviewer, &clearing("APPROVE", &head)).await;
    assert_rejected(&g, &[&g.tester, "FAIL", "new task"]).await;
}

#[tokio::test]
async fn a_reject_ends_the_task_and_names_only_the_rejecting_type() {
    let g = gated().await;
    let head = rev(&g.s, "main").await;
    file_report(&g.s, &g.tester, &clearing("PASS", &head)).await;
    file_report(&g.s, &g.reviewer, &clearing("REJECT", &head)).await;
    let why = assert_rejected(&g, &[&g.reviewer, "REJECT"]).await;
    assert!(!why.contains(&g.tester), "{why}");
}

#[tokio::test]
async fn a_reject_wins_over_a_stale_or_missing_report_of_the_other_type() {
    let g = gated().await;
    let head = rev(&g.s, "main").await;
    file_report(&g.s, &g.reviewer, &clearing("REJECT", &head)).await;
    assert_rejected(&g, &[&g.reviewer, "REJECT"]).await;
}

#[tokio::test]
async fn a_blocked_reason_does_not_change_when_the_base_branch_moves() {
    let g = gated().await;
    let first = rev(&g.s, "main").await;
    both_clear(&g, &first).await;
    scheduler_support::commit(&g.s.repo, "main moved on").await;
    let before = assert_blocked(&g, &["stale"]).await;
    scheduler_support::commit(&g.s.repo, "main moved again").await;
    let after = assert_blocked(&g, &["stale"]).await;
    assert_eq!(before, after);
    assert!(!after.contains(&rev(&g.s, "main").await[..7]), "{after}");
}

#[tokio::test]
async fn a_report_without_a_verdict_or_a_commit_blocks() {
    let g = gated().await;
    let head = rev(&g.s, "main").await;
    file_report(&g.s, &g.tester, "All good, ship it.\n").await;
    file_report(&g.s, &g.reviewer, &format!("COMMIT: {head}\n")).await;
    assert_blocked(&g, &[&g.tester, "VERDICT", &g.reviewer]).await;

    file_report(&g.s, &g.tester, &format!("VERDICT: PASS\nCOMMIT: {head}\n")).await;
    file_report(&g.s, &g.reviewer, "VERDICT: APPROVE\n").await;
    assert_blocked(&g, &[&g.reviewer, "COMMIT"]).await;
}

#[tokio::test]
async fn a_verdict_first_report_clears_and_a_quoted_or_conflicting_one_does_not() {
    let g = gated().await;
    let head = rev(&g.s, "main").await;
    let long = "detail\n".repeat(300);
    let first = |word: &str| format!("VERDICT: {word}\nCOMMIT: {head}\n{long}");
    file_report(&g.s, &g.tester, &first("PASS")).await;
    file_report(&g.s, &g.reviewer, &first("APPROVE")).await;
    assert_starts(&g).await;

    let g = gated().await;
    let head = rev(&g.s, "main").await;
    let quoted = format!("> VERDICT: PASS\n```\nVERDICT: PASS\nCOMMIT: {head}\n```\n");
    file_report(&g.s, &g.tester, &quoted).await;
    file_report(&g.s, &g.reviewer, &clearing("APPROVE", &head)).await;
    assert_blocked(&g, &[&g.tester, "no `VERDICT:`"]).await;

    let g = gated().await;
    let head = rev(&g.s, "main").await;
    let both = format!("VERDICT: FAIL\nVERDICT: PASS\nCOMMIT: {head}\n");
    file_report(&g.s, &g.tester, &both).await;
    file_report(&g.s, &g.reviewer, &clearing("APPROVE", &head)).await;
    assert_blocked(&g, &[&g.tester, "conflicting VERDICT lines: FAIL, PASS"]).await;

    let g = gated().await;
    let head = rev(&g.s, "main").await;
    file_report(&g.s, &g.tester, &clearing("APPROVE", &head)).await;
    file_report(&g.s, &g.reviewer, &clearing("APPROVE", &head)).await;
    assert_blocked(&g, &[&g.tester, "PASS, FAIL"]).await;
}

#[tokio::test]
async fn an_artifact_is_never_a_report() {
    let g = gated().await;
    let head = rev(&g.s, "main").await;
    for (task, word) in [(&g.tester, "PASS"), (&g.reviewer, "APPROVE")] {
        let new = NewArtifact {
            name: "report.md".into(),
            kind: ArtifactKind::Report,
            author_role: AuthorRole::Other,
            caption: None,
            bytes: clearing(word, &head).into_bytes(),
        };
        g.s.store().add_artifact(task, new).await.unwrap();
    }
    assert_blocked(&g, &[&g.tester, &g.reviewer, "no report"]).await;
}

#[tokio::test]
async fn a_required_task_deep_in_the_chain_must_have_ended_too() {
    let g = gated().await;
    let (deep, middle, ship) = (unique("deep"), unique("middle"), unique("deepship"));
    create_as(&g.s, &deep, "tester", &[], "main").await;
    g.s.set_state(&deep, TaskState::Error).await;
    create_as(&g.s, &middle, "plain", &[&deep], "main").await;
    g.s.set_state(&middle, TaskState::Ended).await;
    create_as(&g.s, &ship, "shipper", &[&middle, &g.reviewer], "main").await;
    let head = rev(&g.s, "main").await;
    both_clear(&g, &head).await;
    run_once(g.s.ctx()).await.unwrap();
    let task: Task = g.s.store().find_task(&ship).await.unwrap().unwrap();
    assert_eq!(task.state, TaskState::Queued);
    let why = task.blocked.expect("a block reason");
    assert!(why.contains(&deep) && why.contains("did not end"), "{why}");
}

async fn commit_on(s: &Sched, branch: &str, message: &str) {
    git(&s.repo, &["checkout", "-q", branch]).await;
    scheduler_support::commit(&s.repo, message).await;
    git(&s.repo, &["checkout", "-q", "main"]).await;
}

async fn change_branch(s: &Sched) {
    git(&s.repo, &["branch", "change"]).await;
}

#[tokio::test]
async fn work_after_the_reported_commit_makes_the_reports_stale() {
    let g = gated_on("change").await;
    change_branch(&g.s).await;
    let reported = rev(&g.s, "change").await;
    both_clear(&g, &reported).await;
    commit_on(&g.s, "change", "more work").await;
    let why = assert_blocked(&g, &["stale", &g.tester, &g.reviewer]).await;
    assert!(why.contains(&reported[..7]), "{why}");

    let moved = rev(&g.s, "change").await;
    both_clear(&g, &moved).await;
    assert_starts(&g).await;
}

#[tokio::test]
async fn merging_the_base_branch_in_does_not_make_reports_stale() {
    let g = gated_on("change").await;
    change_branch(&g.s).await;
    let reported = rev(&g.s, "change").await;
    both_clear(&g, &reported).await;
    scheduler_support::commit(&g.s.repo, "main moved on").await;
    git(&g.s.repo, &["checkout", "-q", "change"]).await;
    let merge = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "merge",
        "-q",
        "--no-ff",
        "-m",
        "merge main",
        "main",
    ];
    git(&g.s.repo, &merge).await;
    git(&g.s.repo, &["checkout", "-q", "main"]).await;
    assert_ne!(rev(&g.s, "change").await, reported);
    assert_starts(&g).await;
}

#[tokio::test]
async fn a_commit_the_repo_does_not_have_is_named_in_the_block() {
    let g = gated().await;
    let unknown = "0123456789abcdef0123456789abcdef01234567";
    both_clear(&g, unknown).await;
    assert_blocked(&g, &["0123456", "not a commit"]).await;
}

async fn start_work(g: &Gated) -> String {
    let work = unique("work");
    create_as(&g.s, &work, "builder", &[&g.tester, &g.reviewer], "main").await;
    let pass = run_once(g.s.ctx()).await.unwrap();
    assert_eq!(pass.started, [work.as_str()], "{pass:?}");
    g.s.args(&work).await;
    work
}

#[tokio::test]
async fn a_hand_back_to_a_gated_type_waits_for_the_reports() {
    let g = gated().await;
    let work = start_work(&g).await;
    let before = g.s.agents().await.len();
    g.s.store()
        .hand_back(&work, "shipper", "ship it", "")
        .await
        .unwrap();
    let pass = run_once(g.s.ctx()).await.unwrap();
    assert!(
        !pass.started.contains(&work) && pass.blocked.contains(&work),
        "{pass:?}"
    );
    let task = g.s.store().find_task(&work).await.unwrap().unwrap();
    assert_eq!(task.state, TaskState::Queued);
    let why = task.blocked.expect("a block reason");
    assert!(
        why.contains(&g.tester) && why.contains("no report"),
        "{why}"
    );
    assert_eq!(g.s.agents().await.len(), before);
}

#[tokio::test]
async fn a_hand_back_to_a_gated_type_resumes_once_the_reports_clear_its_branch() {
    let g = gated().await;
    let work = start_work(&g).await;
    g.s.store()
        .hand_back(&work, "shipper", "ship it", "")
        .await
        .unwrap();
    let head = rev(&g.s, &format!("townhall/{work}")).await;
    both_clear(&g, &head).await;
    let pass = run_once(g.s.ctx()).await.unwrap();
    assert!(pass.started.contains(&work), "{pass:?}");
    let task = g.s.store().find_task(&work).await.unwrap().unwrap();
    assert_eq!((task.state, task.blocked), (TaskState::Working, None));
}

#[tokio::test]
async fn a_running_agent_is_adopted_not_gated() {
    let g = gated().await;
    let work = start_work(&g).await;
    let change = |t: Task| {
        Ok(Task {
            agent: "shipper".into(),
            state: TaskState::Queued,
            ..t
        })
    };
    g.s.store().update_task(&work, change).await.unwrap();
    let pass = run_once(g.s.ctx()).await.unwrap();
    assert_eq!(pass.adopted, [work.as_str()], "{pass:?}");
    assert!(!pass.blocked.contains(&work), "{pass:?}");
    assert!(g.s.agents().await.iter().any(|a| a.name == work && a.alive));
}

#[tokio::test]
async fn from_the_base_branch_only_its_tip_clears() {
    let g = gated().await;
    let old = rev(&g.s, "main").await;
    scheduler_support::commit(&g.s.repo, "main moved on").await;
    both_clear(&g, &old).await;
    let why = assert_blocked(&g, &["stale", &g.tester, &old[..7]]).await;
    assert!(why.contains("tip"), "{why}");

    let tip = rev(&g.s, "main").await;
    both_clear(&g, &tip).await;
    assert_starts(&g).await;
}

#[tokio::test]
async fn a_report_commit_may_be_short_or_full_but_not_upper_case() {
    let short = gated().await;
    let tip = rev(&short.s, "main").await;
    both_clear(&short, &tip[..7]).await;
    assert_starts(&short).await;

    let full = gated().await;
    let tip = rev(&full.s, "main").await;
    both_clear(&full, &tip).await;
    assert_starts(&full).await;

    let upper = gated().await;
    let tip = rev(&upper.s, "main").await;
    both_clear(&upper, &tip.to_uppercase()).await;
    assert_blocked(&upper, &[&upper.tester, "COMMIT"]).await;
}

async fn base_gated(base: &str) -> (Gated, String, String) {
    let g = gated_on(base).await;
    let old = rev(&g.s, "main").await;
    scheduler_support::commit(&g.s.repo, "main moved on").await;
    let tip = rev(&g.s, "main").await;
    git(&g.s.repo, &["update-ref", "refs/remotes/origin/main", &tip]).await;
    (g, old, tip)
}

async fn assert_only_the_tip_clears(base: &str, form: fn(&str) -> String) {
    let (g, old, tip) = base_gated(base).await;
    for covered in [form(&old), form(&old[..7])] {
        both_clear(&g, &covered).await;
        assert_blocked(&g, &["stale", &g.tester, "tip"]).await;
    }
    both_clear(&g, &old.to_uppercase()).await;
    assert_blocked(&g, &[&g.tester, "COMMIT"]).await;
    both_clear(&g, &form(&tip[..7])).await;
    assert_starts(&g).await;
}

fn same(sha: &str) -> String {
    sha.to_string()
}

#[tokio::test]
async fn a_base_given_as_a_remote_branch_clears_only_for_its_tip() {
    assert_only_the_tip_clears("origin/main", same).await;
}

#[tokio::test]
async fn a_base_given_as_a_full_ref_clears_only_for_its_tip() {
    assert_only_the_tip_clears("refs/heads/main", same).await;
}

#[tokio::test]
async fn a_base_given_as_a_sha_clears_only_for_that_sha() {
    let g = gated().await;
    let old = rev(&g.s, "main").await;
    scheduler_support::commit(&g.s.repo, "main moved on").await;
    let tip = rev(&g.s, "main").await;
    let base = tip.clone();
    let task = move |t: Task| {
        Ok(Task {
            base_branch: base,
            ..t
        })
    };
    g.s.store().update_task(&g.ship, task).await.unwrap();
    both_clear(&g, &old).await;
    assert_blocked(&g, &["stale", &g.tester, "tip"]).await;
    both_clear(&g, &tip).await;
    assert_starts(&g).await;
}

#[tokio::test]
async fn a_branch_with_no_commits_of_its_own_clears_only_for_its_tip() {
    let g = gated_on("empty").await;
    let old = rev(&g.s, "main").await;
    scheduler_support::commit(&g.s.repo, "main moved on").await;
    git(&g.s.repo, &["branch", "empty"]).await;
    both_clear(&g, &old).await;
    assert_blocked(&g, &["stale", &g.tester, "tip"]).await;
    let tip = rev(&g.s, "empty").await;
    both_clear(&g, &tip[..7]).await;
    assert_starts(&g).await;
}
