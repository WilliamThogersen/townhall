use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use townhall_core::model::{Checks, Pr, PrStatus, Task, TaskState, TrackedPr};
use townhall_core::scheduler::run_once;
use townhall_core::Pacing;

mod scheduler_support;
use scheduler_support::{commit, git, unique, Sched};

const GREEN: &str = r#"[{"name":"build","state":"SUCCESS","bucket":"pass"}]"#;
const PENDING: &str = r#"[{"name":"build","state":"IN_PROGRESS","bucket":"pending"}]"#;
const RED: &str = r#"[{"name":"build","state":"FAILURE","bucket":"fail","link":"https://github.com/acme/shop/actions/runs/1/job/55"}]"#;
const FLAKY: &str = r#"[{"name":"secret-detection","state":"FAILURE","bucket":"fail","link":"https://github.com/acme/shop/actions/runs/1/job/42"},{"name":"build","state":"SUCCESS","bucket":"pass"}]"#;

fn gh_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("townhall-prs-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("tasks")).unwrap();
        let gh = dir.join("gh");
        let script = r#"#!/bin/sh
task="$(basename "$PWD")"
here="$(dirname "$0")/tasks"
echo "$@" >> "$here/$task.log"
if [ "$1 $2" = "auth token" ]; then echo "tok-$4"; exit 0; fi
case "$*" in
  "pr checks"*) cat "$here/$task.json"; exit 0;;
  "pr list"*) if [ -f "$here/$task.list" ]; then cat "$here/$task.list"; else echo '[]'; fi; exit 0;;
  *"--json mergeCommit --jq"*) echo "feedface"; exit 0;;
  "pr view"*) if [ -f "$here/$task.view" ]; then cat "$here/$task.view"; else echo '{"headRefOid":"abc123","mergeable":"MERGEABLE"}'; fi; exit 0;;
  "api -X PUT"*)
    if [ -f "$here/$task.conflict" ]; then echo "merge conflict between base and head" >&2; exit 1; fi
    echo '{"headRefOid":"def456","mergeable":"MERGEABLE"}' > "$here/$task.view"; echo 0 > "$here/$task.behind"; exit 0;;
  "api "*) if [ -f "$here/$task.behind" ]; then cat "$here/$task.behind"; else echo 0; fi; exit 0;;
  "run rerun"*) exit 0;;
  "run view"*) echo "boom: the build broke"; exit 0;;
  "pr merge"*) if [ -f "$here/$task.blocked" ]; then echo "base branch policy prohibits the merge" >&2; exit 1; fi; exit 0;;
esac
exit 1
"#;
        std::fs::write(&gh, script).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("TOWNHALL_GH_BIN", &gh);
        dir
    })
}

fn says(task: &str, file: &str, text: &str) {
    std::fs::write(gh_dir().join("tasks").join(format!("{task}.{file}")), text).unwrap();
}

fn log(task: &str, starts: &str) -> Vec<String> {
    let path = gh_dir().join("tasks").join(format!("{task}.log"));
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter(|l| l.starts_with(starts))
        .map(str::to_string)
        .collect()
}

async fn setup() -> Sched {
    gh_dir();
    let s = Sched::new().await;
    let url = "https://github.com/acme/shop.git";
    git(&s.repo, &["remote", "add", "origin", url]).await;
    s.ctx().set_pacing(Pacing {
        ci_poll: Duration::ZERO,
        ..s.ctx().pacing()
    });
    s
}

async fn ended(s: &Sched, prefix: &str, checks: &str) -> String {
    let name = unique(prefix);
    says(&name, "json", checks);
    s.create(&name, &[]).await;
    run_once(s.ctx()).await.unwrap();
    assert_eq!(s.state(&name).await, TaskState::Working);
    s.end(&name).await;
    name
}

async fn track(s: &Sched, owner: &str) {
    let pr = TrackedPr {
        repo: s.repo_name.clone(),
        number: 7,
        url: Some("https://github.com/acme/shop/pull/7".into()),
        branch: format!("townhall/{owner}"),
        base: "main".into(),
        owner: owner.into(),
        pusher: None,
        head: None,
        status: PrStatus::Pending,
        detail: None,
        reruns: vec![],
        woke_for: None,
        seen_pending: false,
        updated_at: String::new(),
    };
    s.store().track_pr(pr).await.unwrap();
}

async fn tracked(s: &Sched, number: u64) -> Option<TrackedPr> {
    let tasks = s.store().all_tasks().await.unwrap();
    tasks
        .into_iter()
        .flat_map(|t| t.prs)
        .find(|p| p.number == number)
}

async fn until_pr(s: &Sched, want: impl Fn(&TrackedPr) -> bool) -> TrackedPr {
    let give_up = Instant::now() + Duration::from_secs(60);
    while Instant::now() < give_up {
        run_once(s.ctx()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(pr) = tracked(s, 7).await.filter(&want) {
            return pr;
        }
    }
    panic!(
        "the pull request never got there: {:?}",
        tracked(s, 7).await
    );
}

async fn until_state(s: &Sched, name: &str, state: TaskState) {
    let give_up = Instant::now() + Duration::from_secs(60);
    while Instant::now() < give_up {
        run_once(s.ctx()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        if s.state(name).await == state {
            return;
        }
    }
    panic!("`{name}` is {:?}, not {state:?}", s.state(name).await);
}

async fn passes(s: &Sched, count: usize) {
    for _ in 0..count {
        run_once(s.ctx()).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn a_green_clean_pr_is_merged_and_the_conversation_hears_it() {
    let s = setup().await;
    let name = ended(&s, "owner", GREEN).await;
    let pr = Pr {
        number: 7,
        url: None,
        checks: Checks::Pending,
        merge_sha: None,
    };
    let change = move |t: Task| Ok(Task { pr: Some(pr), ..t });
    s.store().update_task(&name, change).await.unwrap();
    track(&s, &name).await;

    let done = until_pr(&s, |p| p.status == PrStatus::Merged).await;
    assert_eq!(done.detail.as_deref(), Some("feedface"));
    assert_eq!(
        log(&name, "pr merge"),
        vec!["pr merge 7 --merge --match-head-commit abc123"]
    );
    until_state(&s, &name, TaskState::Merged).await;
    let told = s.ctx().notes().waiting(&s.conversation);
    assert!(told.iter().any(|t| t.contains("merged PR #7")), "{told:?}");
    assert!(log(&name, "pr merge").iter().all(|l| !l.contains("admin")));
}

#[tokio::test]
async fn a_pr_the_forge_blocks_is_held_never_forced() {
    let s = setup().await;
    let name = ended(&s, "owner", GREEN).await;
    says(&name, "blocked", "yes");
    track(&s, &name).await;
    let held = until_pr(&s, |p| p.status == PrStatus::Blocked).await;
    assert!(held.detail.unwrap().contains("prohibits"));
    assert!(log(&name, "pr merge").iter().all(|l| !l.contains("admin")));
    assert_eq!(s.state(&name).await, TaskState::Ended);
}

#[tokio::test]
async fn a_red_pr_wakes_its_owner_with_the_failing_log() {
    let s = setup().await;
    let name = ended(&s, "owner", RED).await;
    track(&s, &name).await;
    until_state(&s, &name, TaskState::Working).await;
    let pr = tracked(&s, 7).await.unwrap();
    assert_eq!(pr.status, PrStatus::Red);
    assert_eq!(pr.woke_for.as_deref(), Some("abc123"));
    let brief = std::fs::read_to_string(s.brief(&name)).unwrap();
    for needle in [
        "pull request #7 needs work",
        "build",
        "boom: the build broke",
    ] {
        assert!(brief.contains(needle), "{needle} missing in {brief}");
    }
    passes(&s, 3).await;
    assert_eq!(s.store().handoffs(&name).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_flaky_check_is_rerun_once_and_the_pr_then_merges() {
    let s = setup().await;
    let name = ended(&s, "owner", FLAKY).await;
    track(&s, &name).await;
    until_pr(&s, |p| p.status == PrStatus::Rerunning).await;
    assert_eq!(log(&name, "run rerun"), vec!["run rerun --job 42"]);
    passes(&s, 3).await;
    assert_eq!(log(&name, "run rerun").len(), 1);
    assert_eq!(s.state(&name).await, TaskState::Ended);
    assert!(s.store().handoffs(&name).await.unwrap().is_empty());

    says(&name, "json", GREEN);
    until_pr(&s, |p| p.status == PrStatus::Merged).await;
}

#[tokio::test]
async fn a_flaky_check_that_fails_again_wakes_the_owner() {
    let s = setup().await;
    let name = ended(&s, "owner", FLAKY).await;
    track(&s, &name).await;
    until_pr(&s, |p| p.status == PrStatus::Rerunning).await;
    says(&name, "json", PENDING);
    until_pr(&s, |p| p.seen_pending).await;
    says(&name, "json", FLAKY);
    until_state(&s, &name, TaskState::Working).await;
    assert_eq!(log(&name, "run rerun").len(), 1);
    assert_eq!(tracked(&s, 7).await.unwrap().status, PrStatus::Red);
}

#[tokio::test]
async fn a_pr_behind_main_gets_main_merged_in_by_the_daemon_then_merges() {
    let s = setup().await;
    let name = ended(&s, "owner", GREEN).await;
    says(&name, "behind", "3");
    track(&s, &name).await;
    until_pr(&s, |p| p.status == PrStatus::Merged).await;
    assert_eq!(log(&name, "api -X PUT").len(), 1);
    assert_eq!(
        log(&name, "pr merge"),
        vec!["pr merge 7 --merge --match-head-commit def456"]
    );
    assert!(s.store().handoffs(&name).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_conflicting_pr_wakes_its_owner_with_the_conflict_files() {
    let s = setup().await;
    std::fs::write(s.repo.join("a.txt"), "base\n").unwrap();
    git(&s.repo, &["add", "a.txt"]).await;
    commit(&s.repo, "base").await;
    let name = ended(&s, "owner", GREEN).await;
    let tree = s.worktree(&name);
    std::fs::write(tree.join("a.txt"), "branch\n").unwrap();
    git(&tree, &["add", "a.txt"]).await;
    commit(&tree, "branch side").await;
    let head = git(&tree, &["rev-parse", "HEAD"]).await;
    std::fs::write(s.repo.join("a.txt"), "main\n").unwrap();
    git(&s.repo, &["add", "a.txt"]).await;
    commit(&s.repo, "main side").await;
    let view = format!(
        r#"{{"headRefOid":"{}","mergeable":"CONFLICTING"}}"#,
        head.trim()
    );
    says(&name, "view", &view);
    track(&s, &name).await;

    until_state(&s, &name, TaskState::Working).await;
    assert_eq!(tracked(&s, 7).await.unwrap().status, PrStatus::Conflict);
    let brief = std::fs::read_to_string(s.brief(&name)).unwrap();
    for needle in ["conflicts with main", "a.txt"] {
        assert!(brief.contains(needle), "{needle} missing in {brief}");
    }
    assert!(log(&name, "pr merge").is_empty());
}

#[tokio::test]
async fn a_conflict_found_by_update_branch_wakes_the_owner() {
    let s = setup().await;
    let name = ended(&s, "owner", GREEN).await;
    says(&name, "behind", "2");
    says(&name, "conflict", "yes");
    track(&s, &name).await;
    until_state(&s, &name, TaskState::Working).await;
    assert_eq!(tracked(&s, 7).await.unwrap().status, PrStatus::Conflict);
    assert!(log(&name, "pr merge").is_empty());
}

#[tokio::test]
async fn a_task_that_pushed_to_someone_elses_pr_is_the_one_woken() {
    let s = setup().await;
    let owner = ended(&s, "owner", RED).await;
    let lander = ended(&s, "lander", RED).await;
    let tree = s.worktree(&lander);
    commit(&tree, "fix for the other pr").await;
    let head = git(&tree, &["rev-parse", "HEAD"]).await;
    let view = format!(
        r#"{{"headRefOid":"{}","mergeable":"MERGEABLE"}}"#,
        head.trim()
    );
    says(&owner, "view", &view);
    track(&s, &owner).await;

    until_state(&s, &lander, TaskState::Working).await;
    let pr = tracked(&s, 7).await.unwrap();
    assert_eq!(pr.pusher.as_deref(), Some(lander.as_str()));
    assert_eq!(pr.owner, owner);
    assert_eq!(s.state(&owner).await, TaskState::Ended);
    assert!(s.store().handoffs(&owner).await.unwrap().is_empty());
    let brief = std::fs::read_to_string(s.brief(&lander)).unwrap();
    assert!(brief.contains("pull request #7 needs work"), "{brief}");
}

#[tokio::test]
async fn an_open_pr_on_a_task_branch_is_found_and_tracked() {
    let s = setup().await;
    let name = ended(&s, "owner", PENDING).await;
    let listed = format!(
        r#"[{{"number":7,"url":"https://github.com/acme/shop/pull/7","headRefName":"townhall/{name}","headRefOid":"abc123","baseRefName":"main"}}]"#
    );
    says(&name, "list", &listed);
    let found = until_pr(&s, |_| true).await;
    assert_eq!(found.owner, name);
    assert_eq!(found.base, "main");
}
