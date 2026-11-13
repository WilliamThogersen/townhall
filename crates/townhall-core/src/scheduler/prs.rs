use std::path::PathBuf;

use crate::cmd::run;
use crate::context::Context;
use crate::model::{
    AgentEvent, Checks, EntryBody, Pr, PrStatus, Task, TaskState, TrackedPr, MAX_HANDOFFS,
};
use crate::repos::accounts::Accounts;
use crate::repos::pr_checks::{
    conflict_files, failed_log, job_id, merge_pr, open_prs, pr_checks, pr_facts, rerun_job,
    update_branch, CheckState, ChecksRequest, PrFacts, Updated,
};
use crate::repos::publish::Programs;
use crate::{clock, runner, Error, Result};

use super::{ci, plan, transition};

const KEY: &str = "<pull requests>";
const RERUN_GRACE_MS: i64 = 120_000;
const LOG_JOBS: usize = 3;
const RETRY_DETAIL: &str = "waking the owner failed";
const RETRY_BACKOFF_MS: i64 = 300_000;

#[derive(Debug, PartialEq, Eq)]
enum Cause {
    Checks(Vec<CheckState>),
    Conflict,
}

#[derive(Debug, PartialEq, Eq)]
enum Plan {
    Hold(PrStatus, Option<String>),
    Rerun(Vec<(String, u64)>),
    Wake(PrStatus, Cause),
    Update,
    Merge,
}

fn flaky(name: &str, list: &[String]) -> bool {
    let name = name.to_ascii_lowercase();
    list.iter()
        .any(|f| !f.is_empty() && name.contains(&f.to_ascii_lowercase()))
}

fn names(checks: &[&CheckState]) -> String {
    checks
        .iter()
        .map(|c| c.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn decide(
    pr: &TrackedPr,
    states: &[CheckState],
    facts: &PrFacts,
    flaky_list: &[String],
    now_ms: i64,
) -> Plan {
    if states.iter().any(CheckState::pending) {
        let status = match pr.reruns.is_empty() {
            true => PrStatus::Pending,
            false => PrStatus::Rerunning,
        };
        return Plan::Hold(status, None);
    }
    let failed: Vec<&CheckState> = states.iter().filter(|c| c.failed()).collect();
    if !failed.is_empty() {
        let fresh: Vec<(String, u64)> = failed
            .iter()
            .filter(|c| flaky(&c.name, flaky_list) && !pr.reruns.contains(&c.name))
            .filter_map(|c| job_id(&c.link).map(|j| (c.name.clone(), j)))
            .collect();
        if fresh.len() == failed.len() {
            return Plan::Rerun(fresh);
        }
        let reran = failed.iter().all(|c| pr.reruns.contains(&c.name));
        let since = clock::parse_rfc3339(&pr.updated_at).unwrap_or(0);
        if reran && !pr.seen_pending && now_ms - since < RERUN_GRACE_MS {
            return Plan::Hold(PrStatus::Rerunning, None);
        }
        return Plan::Wake(PrStatus::Red, Cause::Checks(states.to_vec()));
    }
    if states.is_empty() {
        return Plan::Hold(PrStatus::Pending, Some("no checks reported yet".into()));
    }
    if facts.conflicting {
        return Plan::Wake(PrStatus::Conflict, Cause::Conflict);
    }
    if facts.unknown {
        return Plan::Hold(PrStatus::Pending, Some("mergeability not known yet".into()));
    }
    if facts.behind > 0 {
        return Plan::Update;
    }
    match facts.merge_state.as_str() {
        "BLOCKED" | "DRAFT" => Plan::Hold(
            PrStatus::Blocked,
            Some(format!(
                "checks are green but the forge holds the merge ({})",
                facts.merge_state.to_ascii_lowercase()
            )),
        ),
        _ => Plan::Merge,
    }
}

pub(super) fn start(ctx: &Context) {
    let key = (ctx.home().root().to_path_buf(), KEY.to_string());
    if !ci::claim(&key) {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(e) = poll(&ctx).await {
            tracing::warn!(
                target: "townhall::task", event = "task.prs_unread",
                error_kind = e.kind(), error = %crate::redact::redact(&e.to_string()),
                "the tracked pull requests could not be polled"
            );
        }
        ci::release(&key, ctx.pacing().ci_poll);
    });
}

struct Anchor {
    worktree: PathBuf,
    repo: String,
}

async fn anchor(ctx: &Context, tasks: &[Task], pr: &TrackedPr) -> Option<Anchor> {
    let pusher = pr.pusher.as_deref().unwrap_or_default();
    for name in [pr.owner.as_str(), pusher] {
        let Some(task) = tasks.iter().find(|t| t.name.eq_ignore_ascii_case(name)) else {
            continue;
        };
        if let Some(worktree) = crate::mcp::worktree_of(ctx, task).await {
            return Some(Anchor {
                worktree,
                repo: task.repo.clone(),
            });
        }
    }
    None
}

async fn forge<T: Send + 'static>(
    ctx: &Context,
    anchor: &Anchor,
    number: u64,
    branch: &str,
    call: impl FnOnce(&ChecksRequest, &Programs) -> Result<T> + Send + 'static,
) -> Result<Option<T>> {
    let settings = ctx.settings().await?;
    let repo = ctx.repo(&anchor.repo).await?;
    let accounts = Accounts::for_repo(&settings, Some(&repo));
    let (worktree, branch) = (anchor.worktree.clone(), branch.to_string());
    tokio::task::spawn_blocking(move || {
        let req = ChecksRequest {
            worktree: &worktree,
            number,
            branch: &branch,
            accounts: &accounts,
        };
        call(&req, &Programs::from_env()).map(Some)
    })
    .await
    .map_err(|e| {
        Error::io(
            "read pull request state",
            std::io::Error::other(e.to_string()),
        )
    })?
}

async fn poll(ctx: &Context) -> Result<()> {
    let store = ctx.store()?;
    let tasks = store.all_tasks().await?;
    discover(ctx, &tasks).await?;
    let list = ctx.settings().await?.flaky_checks;
    let tasks = store.all_tasks().await?;
    for pr in store.open_prs().await? {
        if let Err(e) = step(ctx, &tasks, &list, pr.clone()).await {
            tracing::warn!(
                target: "townhall::task", event = "task.pr_unread", repo = %pr.repo, number = pr.number,
                error_kind = e.kind(), error = %crate::redact::redact(&e.to_string()),
                "a tracked pull request could not be handled"
            );
        }
    }
    Ok(())
}

async fn discover(ctx: &Context, tasks: &[Task]) -> Result<()> {
    let store = ctx.store()?;
    let tracked = store.open_prs().await?;
    let mut repos: Vec<&str> = vec![];
    for task in tasks {
        if !repos.iter().any(|r| r.eq_ignore_ascii_case(&task.repo)) {
            repos.push(&task.repo);
        }
    }
    for repo in repos {
        let Some(anchor) = first_anchor(ctx, tasks, repo).await else {
            continue;
        };
        let listed = forge(ctx, &anchor, 0, "", open_prs).await?.flatten();
        for open in listed.unwrap_or_default() {
            let known = tracked
                .iter()
                .any(|p| p.repo.eq_ignore_ascii_case(repo) && p.number == open.number);
            let owner = tasks
                .iter()
                .find(|t| t.repo.eq_ignore_ascii_case(repo) && t.branch() == open.branch);
            if let (false, Some(owner)) = (known, owner) {
                track(ctx, owner, open.number, Some(open.url), &open.base).await?;
            }
        }
    }
    Ok(())
}

async fn first_anchor(ctx: &Context, tasks: &[Task], repo: &str) -> Option<Anchor> {
    for task in tasks.iter().filter(|t| t.repo.eq_ignore_ascii_case(repo)) {
        if let Some(worktree) = crate::mcp::worktree_of(ctx, task).await {
            return Some(Anchor {
                worktree,
                repo: task.repo.clone(),
            });
        }
    }
    None
}

pub(crate) async fn track(
    ctx: &Context,
    owner: &Task,
    number: u64,
    url: Option<String>,
    base: &str,
) -> Result<TrackedPr> {
    let pr = TrackedPr {
        repo: owner.repo.clone(),
        number,
        url,
        branch: owner.branch(),
        base: base.to_string(),
        owner: owner.name.clone(),
        pusher: None,
        head: None,
        status: PrStatus::Pending,
        detail: None,
        reruns: vec![],
        woke_for: None,
        seen_pending: false,
        updated_at: String::new(),
    };
    ctx.store()?.track_pr(pr).await
}

async fn pusher_of(ctx: &Context, tasks: &[Task], pr: &TrackedPr, head: &str) -> Option<String> {
    for task in tasks
        .iter()
        .filter(|t| t.repo.eq_ignore_ascii_case(&pr.repo))
    {
        let Some(tree) = crate::mcp::worktree_of(ctx, task).await else {
            continue;
        };
        let at = run("git", &["rev-parse", "HEAD"], Some(&tree)).await.ok();
        if at.is_some_and(|sha| sha.trim() == head) {
            return Some(task.name.clone());
        }
    }
    None
}

async fn observe(
    ctx: &Context,
    tasks: &[Task],
    pr: TrackedPr,
    facts: &PrFacts,
) -> Result<TrackedPr> {
    if pr.head.as_deref() == Some(facts.head.as_str()) {
        return Ok(pr);
    }
    let pusher = pusher_of(ctx, tasks, &pr, &facts.head).await;
    let head = facts.head.clone();
    let change = move |p: TrackedPr| {
        Ok(TrackedPr {
            head: Some(head),
            status: PrStatus::Pending,
            detail: None,
            reruns: vec![],
            seen_pending: false,
            pusher: pusher.or(p.pusher.clone()),
            ..p
        })
    };
    ctx.store()?.update_pr(&pr.repo, pr.number, change).await
}

async fn settle(
    ctx: &Context,
    pr: &TrackedPr,
    status: PrStatus,
    detail: Option<String>,
    seen_pending: bool,
) -> Result<()> {
    let seen_pending = seen_pending || pr.seen_pending;
    if (pr.status, &pr.detail, pr.seen_pending) == (status, &detail, seen_pending) {
        return Ok(());
    }
    let change = move |p: TrackedPr| {
        Ok(TrackedPr {
            status,
            detail,
            seen_pending,
            ..p
        })
    };
    ctx.store()?
        .update_pr(&pr.repo, pr.number, change)
        .await
        .map(|_| ())
}

async fn step(ctx: &Context, tasks: &[Task], list: &[String], pr: TrackedPr) -> Result<()> {
    let Some(anchor) = anchor(ctx, tasks, &pr).await else {
        return Ok(());
    };
    let base = pr.base.clone();
    let seen = forge(ctx, &anchor, pr.number, &pr.branch, move |req, p| {
        pr_facts(req, &base, p)
    })
    .await?
    .flatten();
    let Some(facts) = seen else {
        return Ok(());
    };
    match facts.state.as_str() {
        "MERGED" => return finish(ctx, &pr, PrStatus::Merged, facts.merge_commit).await,
        "CLOSED" => return finish(ctx, &pr, PrStatus::Closed, None).await,
        _ => {}
    }
    let pr = observe(ctx, tasks, pr, &facts).await?;
    let read = forge(ctx, &anchor, pr.number, &pr.branch, pr_checks)
        .await?
        .flatten();
    let Some(states) = read else {
        return Ok(());
    };
    let pending = states.iter().any(CheckState::pending);
    let now = clock::unix_millis(std::time::SystemTime::now());
    match decide(&pr, &states, &facts, list, now) {
        Plan::Hold(status, detail) => settle(ctx, &pr, status, detail, pending).await,
        Plan::Rerun(jobs) => rerun(ctx, &anchor, &pr, jobs).await,
        Plan::Wake(status, cause) => wake(ctx, tasks, &anchor, &pr, &facts, status, cause).await,
        Plan::Update => refresh(ctx, tasks, &anchor, &pr, &facts).await,
        Plan::Merge => merge(ctx, tasks, &anchor, &pr, &facts.head).await,
    }
}

async fn finish(
    ctx: &Context,
    pr: &TrackedPr,
    status: PrStatus,
    sha: Option<String>,
) -> Result<()> {
    let change = move |p: TrackedPr| {
        Ok(TrackedPr {
            status,
            detail: sha,
            ..p
        })
    };
    ctx.store()?
        .update_pr(&pr.repo, pr.number, change)
        .await
        .map(|_| ())
}

async fn rerun(
    ctx: &Context,
    anchor: &Anchor,
    pr: &TrackedPr,
    jobs: Vec<(String, u64)>,
) -> Result<()> {
    let mut done = vec![];
    for (name, job) in jobs {
        forge(ctx, anchor, pr.number, &pr.branch, move |req, p| {
            rerun_job(req, job, p)
        })
        .await?;
        done.push(name);
    }
    let detail = format!("rerunning {} once", done.join(", "));
    let change = move |p: TrackedPr| {
        let mut reruns = p.reruns.clone();
        reruns.extend(done);
        Ok(TrackedPr {
            status: PrStatus::Rerunning,
            detail: Some(detail),
            reruns,
            seen_pending: false,
            ..p
        })
    };
    ctx.store()?
        .update_pr(&pr.repo, pr.number, change)
        .await
        .map(|_| ())
}

async fn describe(
    ctx: &Context,
    anchor: &Anchor,
    pr: &TrackedPr,
    facts: &PrFacts,
    cause: &Cause,
) -> String {
    match cause {
        Cause::Conflict => {
            let (tree, base, head) = (anchor.worktree.clone(), pr.base.clone(), facts.head.clone());
            let files = tokio::task::spawn_blocking(move || conflict_files(&tree, &base, &head))
                .await
                .map_err(|e| Error::io("list conflicts", std::io::Error::other(e.to_string())))
                .and_then(|r| r);
            let listed = match files {
                Ok(f) if f.is_empty() => "files could not be determined locally".to_string(),
                Ok(f) => f.join(", "),
                Err(e) => format!("files could not be listed: {e}"),
            };
            format!(
                "it conflicts with {}: merge {} into the branch, resolve the conflicts in {listed}, and push",
                pr.base, pr.base
            )
        }
        Cause::Checks(states) => {
            let failed: Vec<&CheckState> = states.iter().filter(|c| c.failed()).collect();
            let mut text = format!("checks failed: {}. Fix them and push", names(&failed));
            for check in failed.iter().take(LOG_JOBS) {
                let Some(job) = job_id(&check.link) else {
                    continue;
                };
                let log = forge(ctx, anchor, pr.number, &pr.branch, move |req, p| {
                    failed_log(req, job, p)
                })
                .await;
                match log {
                    Ok(Some(Some(excerpt))) => {
                        text.push_str(&format!("\n\n{} log tail:\n{excerpt}", check.name));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        text.push_str(&format!("\n\n{} log could not be read: {e}", check.name))
                    }
                }
            }
            text
        }
    }
}

async fn wake(
    ctx: &Context,
    tasks: &[Task],
    anchor: &Anchor,
    pr: &TrackedPr,
    facts: &PrFacts,
    status: PrStatus,
    cause: Cause,
) -> Result<()> {
    if pr.woke_for.as_deref() == Some(facts.head.as_str()) {
        return settle(ctx, pr, status, None, false).await;
    }
    let target = pr.pusher.as_deref().unwrap_or(&pr.owner);
    let found = tasks
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(target))
        .or_else(|| {
            tasks
                .iter()
                .find(|t| t.name.eq_ignore_ascii_case(&pr.owner))
        });
    let Some(task) = found else {
        return settle(ctx, pr, status, None, false).await;
    };
    if !matches!(task.state, TaskState::Ended | TaskState::InCi) {
        return settle(ctx, pr, status, None, false).await;
    }
    let store = ctx.store()?;
    if backing_off(pr) {
        return Ok(());
    }
    if store.handoffs(&task.name).await?.len() >= MAX_HANDOFFS {
        let why = format!(
            "pull request #{} needs work ({}) but task `{}` was already handed back {MAX_HANDOFFS} times",
            pr.number,
            summary(&cause),
            task.name
        );
        return give_up(ctx, pr, task, task.state, facts, status, why).await;
    }
    let advice = describe(ctx, anchor, pr, facts, &cause).await;
    let reason = format!("pull request #{} needs work: {advice}", pr.number);
    let was = task.state;
    if was == TaskState::Ended
        && transition(store, &task.name, TaskState::Ended, TaskState::InCi)
            .await?
            .is_none()
    {
        return Ok(());
    }
    match store.hand_after_ci(&task.name, &task.agent, &reason).await {
        Ok(_) => woke(ctx, pr, facts, status, plan::capped(&advice, 200)).await,
        Err(Error::Conflict(why)) => {
            let why = format!("pull request #{} needs work: {why}. {advice}", pr.number);
            give_up(ctx, pr, task, TaskState::InCi, facts, status, why).await
        }
        Err(e) => {
            if was == TaskState::Ended {
                transition(store, &task.name, TaskState::InCi, TaskState::Ended).await?;
            }
            let detail = format!("{RETRY_DETAIL}: {e}");
            settle(ctx, pr, status, Some(plan::capped(&detail, 200)), false).await?;
            Err(e)
        }
    }
}

fn backing_off(pr: &TrackedPr) -> bool {
    let failed = pr
        .detail
        .as_deref()
        .is_some_and(|d| d.starts_with(RETRY_DETAIL));
    let at = clock::parse_rfc3339(&pr.updated_at);
    let now = clock::unix_millis(std::time::SystemTime::now());
    failed && at.is_some_and(|at| now - at < RETRY_BACKOFF_MS)
}

fn summary(cause: &Cause) -> String {
    match cause {
        Cause::Conflict => "it conflicts with its base".to_string(),
        Cause::Checks(states) => {
            let failed: Vec<&CheckState> = states.iter().filter(|c| c.failed()).collect();
            format!("red checks: {}", names(&failed))
        }
    }
}

async fn give_up(
    ctx: &Context,
    pr: &TrackedPr,
    task: &Task,
    from: TaskState,
    facts: &PrFacts,
    status: PrStatus,
    why: String,
) -> Result<()> {
    ci::needs_you(ctx, task, from, why.clone()).await?;
    woke(ctx, pr, facts, status, plan::capped(&why, 200)).await
}

async fn woke(
    ctx: &Context,
    pr: &TrackedPr,
    facts: &PrFacts,
    status: PrStatus,
    detail: String,
) -> Result<()> {
    let head = facts.head.clone();
    let change = move |p: TrackedPr| {
        Ok(TrackedPr {
            status,
            detail: Some(detail),
            woke_for: Some(head),
            ..p
        })
    };
    ctx.store()?
        .update_pr(&pr.repo, pr.number, change)
        .await
        .map(|_| ())
}

async fn refresh(
    ctx: &Context,
    tasks: &[Task],
    anchor: &Anchor,
    pr: &TrackedPr,
    facts: &PrFacts,
) -> Result<()> {
    let head = facts.head.clone();
    let updated = forge(ctx, anchor, pr.number, &pr.branch, move |req, p| {
        update_branch(req, &head, p)
    })
    .await?
    .flatten();
    match updated {
        Some(Updated::Started) => {
            let detail = format!("merging {} into the branch", pr.base);
            settle(ctx, pr, PrStatus::Updating, Some(detail), false).await
        }
        Some(Updated::Conflict(_)) => {
            wake(
                ctx,
                tasks,
                anchor,
                pr,
                facts,
                PrStatus::Conflict,
                Cause::Conflict,
            )
            .await
        }
        None => Ok(()),
    }
}

async fn merge(
    ctx: &Context,
    tasks: &[Task],
    anchor: &Anchor,
    pr: &TrackedPr,
    head: &str,
) -> Result<()> {
    let kept = head.to_string();
    let merged = forge(ctx, anchor, pr.number, &pr.branch, move |req, p| {
        merge_pr(req, &kept, p)
    })
    .await;
    let sha = match merged {
        Ok(Some(Some(sha))) => sha,
        Ok(_) => return Ok(()),
        Err(Error::InvalidInput(why)) => {
            return settle(
                ctx,
                pr,
                PrStatus::Blocked,
                Some(plan::capped(&why, 300)),
                false,
            )
            .await;
        }
        Err(e) => return Err(e),
    };
    finish(ctx, pr, PrStatus::Merged, Some(sha.clone())).await?;
    let Some(owner) = tasks
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(&pr.owner))
    else {
        return Ok(());
    };
    let (number, kept) = (pr.number, sha.clone());
    let change = move |t: Task| match (t.state, t.pr.as_ref().map(|p| p.number)) {
        (TaskState::Ended | TaskState::InCi, Some(n)) if n == number => Ok(Task {
            state: TaskState::Merged,
            pr: t.pr.map(|p| Pr {
                checks: Checks::Passed,
                merge_sha: Some(kept),
                ..p
            }),
            ..t
        }),
        _ => Ok(t),
    };
    let store = ctx.store()?;
    store.update_task(&owner.name, change).await?;
    let said = format!(
        "task {} merged PR #{} into {}: merged, sha {sha}",
        owner.name, pr.number, pr.base
    );
    let body = EntryBody::Agent {
        agent: owner.name.clone(),
        event: AgentEvent::Passed,
        text: plan::capped(&said, 400),
    };
    store.append_entry(&owner.conversation_id, body).await?;
    let all = store.tasks(&owner.conversation_id).await?;
    let event = match plan::closing_line(&all) {
        Some(closing) => format!("{said}. {closing}"),
        None => said,
    };
    runner::notify(ctx, &owner.conversation_id, event);
    tracing::info!(
        target: "townhall::task", event = "task.merged", task = %owner.name,
        sha = %sha, "the daemon merged a green pull request"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pr() -> TrackedPr {
        TrackedPr {
            repo: "shop".into(),
            number: 7,
            url: None,
            branch: "townhall/a".into(),
            base: "main".into(),
            owner: "a".into(),
            pusher: None,
            head: Some("abc".into()),
            status: PrStatus::Pending,
            detail: None,
            reruns: vec![],
            woke_for: None,
            seen_pending: false,
            updated_at: String::new(),
        }
    }

    fn check(name: &str, bucket: &str) -> CheckState {
        CheckState {
            name: name.into(),
            state: bucket.to_uppercase(),
            bucket: bucket.into(),
            link: format!("https://github.com/o/r/actions/runs/1/job/{}", name.len()),
        }
    }

    fn facts() -> PrFacts {
        PrFacts {
            head: "abc".into(),
            conflicting: false,
            unknown: false,
            behind: 0,
            state: "OPEN".into(),
            merge_state: "CLEAN".into(),
            merge_commit: None,
        }
    }

    fn list() -> Vec<String> {
        vec!["secret-detection".into()]
    }

    #[test]
    fn green_and_clean_merges() {
        let states = [check("build", "pass")];
        assert_eq!(decide(&pr(), &states, &facts(), &list(), 0), Plan::Merge);
    }

    #[test]
    fn a_red_flaky_check_is_rerun_once_then_wakes() {
        let states = [check("secret-detection", "fail"), check("build", "pass")];
        let first = decide(&pr(), &states, &facts(), &list(), 0);
        assert_eq!(first, Plan::Rerun(vec![("secret-detection".into(), 16)]));
        let reran = TrackedPr {
            reruns: vec!["secret-detection".into()],
            seen_pending: true,
            ..pr()
        };
        let again = decide(&reran, &states, &facts(), &list(), 0);
        assert!(matches!(again, Plan::Wake(PrStatus::Red, _)), "{again:?}");
    }

    #[test]
    fn a_red_check_that_is_not_flaky_wakes() {
        let states = [check("build", "fail"), check("secret-detection", "fail")];
        let got = decide(&pr(), &states, &facts(), &list(), 0);
        assert!(matches!(got, Plan::Wake(PrStatus::Red, _)), "{got:?}");
    }

    #[test]
    fn a_stale_red_right_after_a_rerun_holds() {
        let states = [check("secret-detection", "fail")];
        let reran = TrackedPr {
            reruns: vec!["secret-detection".into()],
            updated_at: clock::from_unix_millis(1_000_000),
            ..pr()
        };
        let got = decide(&reran, &states, &facts(), &list(), 1_010_000);
        assert_eq!(got, Plan::Hold(PrStatus::Rerunning, None));
        let late = decide(&reran, &states, &facts(), &list(), 2_000_000);
        assert!(matches!(late, Plan::Wake(PrStatus::Red, _)), "{late:?}");
    }

    #[test]
    fn behind_updates_and_conflicts_wake() {
        let states = [check("build", "pass")];
        let behind = PrFacts {
            behind: 2,
            ..facts()
        };
        assert_eq!(decide(&pr(), &states, &behind, &list(), 0), Plan::Update);
        let conflicting = PrFacts {
            conflicting: true,
            ..facts()
        };
        let got = decide(&pr(), &states, &conflicting, &list(), 0);
        assert_eq!(got, Plan::Wake(PrStatus::Conflict, Cause::Conflict));
    }

    #[test]
    fn a_protected_branch_is_never_overridden() {
        let states = [check("build", "pass")];
        let blocked = PrFacts {
            merge_state: "BLOCKED".into(),
            ..facts()
        };
        let got = decide(&pr(), &states, &blocked, &list(), 0);
        assert!(
            matches!(got, Plan::Hold(PrStatus::Blocked, Some(_))),
            "{got:?}"
        );
    }

    #[test]
    fn no_checks_and_pending_checks_hold() {
        let got = decide(&pr(), &[], &facts(), &list(), 0);
        assert!(matches!(got, Plan::Hold(PrStatus::Pending, Some(_))));
        let states = [check("build", "pending")];
        let got = decide(&pr(), &states, &facts(), &list(), 0);
        assert_eq!(got, Plan::Hold(PrStatus::Pending, None));
    }
}
