use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::context::Context;
use crate::mcp;
use crate::model::{AgentEvent, Checks, EntryBody, Pr, Task, TaskState};
use crate::repos::accounts::Accounts;
use crate::repos::pr_checks::{merge_pr, pr_checks, pr_facts, CheckState, ChecksRequest, PrFacts};
use crate::repos::publish::Programs;
use crate::{runner, Error, Result};

use super::land::{all_green, decide, Verdict};
use super::reflect::REFLECTOR;
use super::{plan, transition, MOVED};

const REASON_PREFIX: &str = "the checks of PR #";

#[derive(Default)]
struct Watch {
    busy: bool,
    next: Option<Instant>,
}

static WATCHES: Mutex<Option<HashMap<(PathBuf, String), Watch>>> = Mutex::new(None);

fn watching<T>(key: &(PathBuf, String), act: impl FnOnce(&mut Watch) -> T) -> T {
    let mut guard = WATCHES.lock().unwrap_or_else(|p| p.into_inner());
    let all = guard.get_or_insert_with(HashMap::new);
    act(all.entry(key.clone()).or_default())
}

fn claim(key: &(PathBuf, String)) -> bool {
    watching(key, |w| {
        let due = w.next.is_none_or(|at| Instant::now() >= at);
        let free = due && !w.busy;
        w.busy |= free;
        free
    })
}

fn release(key: &(PathBuf, String), poll: Duration) {
    watching(key, |w| {
        w.busy = false;
        w.next = Some(Instant::now() + poll);
    });
}

fn due(tasks: &[Task]) -> Vec<&Task> {
    tasks
        .iter()
        .filter(|t| !t.agent.eq_ignore_ascii_case(REFLECTOR))
        .filter(|t| match (&t.pr, t.state) {
            (Some(pr), TaskState::Ended) => pr.checks == Checks::Pending,
            (Some(_), TaskState::InCi) => true,
            _ => false,
        })
        .collect()
}

pub(super) fn start(ctx: &Context, tasks: &[Task]) -> Vec<String> {
    let mut started = vec![];
    for task in due(tasks) {
        let key = (ctx.home().root().to_path_buf(), task.name.clone());
        if !claim(&key) {
            continue;
        }
        started.push(task.name.clone());
        let (ctx, task) = (ctx.clone(), task.clone());
        tokio::spawn(async move {
            if let Err(e) = judge(&ctx, &task).await {
                tracing::warn!(
                    target: "townhall::task", event = "task.ci_unread", task = %task.name,
                    error_kind = e.kind(), error = %crate::redact::redact(&e.to_string()),
                    "a task's pull request checks could not be read"
                );
            }
            release(&key, ctx.pacing().ci_poll);
        });
    }
    started
}

async fn on_forge<T: Send + 'static>(
    ctx: &Context,
    task: &Task,
    number: u64,
    call: impl FnOnce(&ChecksRequest, &Programs) -> Result<T> + Send + 'static,
) -> Result<Option<T>> {
    let Some(worktree) = mcp::worktree_of(ctx, task).await else {
        return Ok(None);
    };
    let settings = ctx.settings().await?;
    let repo = ctx.repo(&task.repo).await?;
    let accounts = Accounts::for_repo(&settings, Some(&repo));
    let branch = task.branch();
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
            "read pull request checks",
            std::io::Error::other(e.to_string()),
        )
    })?
}

async fn read(ctx: &Context, task: &Task, number: u64) -> Result<Option<Vec<CheckState>>> {
    let read = on_forge(ctx, task, number, pr_checks).await?;
    Ok(read.flatten())
}

async fn facts(ctx: &Context, task: &Task, number: u64) -> Result<Option<PrFacts>> {
    let base = task.pr_target().to_string();
    let read = on_forge(ctx, task, number, move |req, programs| {
        pr_facts(req, &base, programs)
    })
    .await?;
    Ok(read.flatten())
}

async fn merges_on_green(ctx: &Context, task: &Task) -> Result<bool> {
    let def = ctx.store()?.agent_type(&task.agent).await?;
    Ok(match def {
        Some(t) => t.def.merges_on_green(),
        None => task.agent.eq_ignore_ascii_case("lander"),
    })
}

fn describe(states: &[CheckState]) -> String {
    match states.is_empty() {
        true => "no checks".to_string(),
        false => states
            .iter()
            .map(|c| format!("{}: {}", c.name, c.state))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

async fn judge(ctx: &Context, task: &Task) -> Result<()> {
    let Some(pr) = task.pr.clone() else {
        return Ok(());
    };
    let Some(states) = read(ctx, task, pr.number).await? else {
        return Ok(());
    };
    if matches!(task.state, TaskState::Ended | TaskState::InCi)
        && merges_on_green(ctx, task).await?
    {
        return judge_for_merge(ctx, task, &pr, &states).await;
    }
    let pending = states.iter().any(CheckState::pending);
    match (task.state, pending) {
        (TaskState::Ended, true) => wait(ctx, task, &pr, &states).await,
        (TaskState::Ended, false) => record(ctx, task, &states).await,
        (TaskState::InCi, true) => Ok(()),
        (TaskState::InCi, false) => relaunch(ctx, task, &pr, &states, None).await,
        _ => Ok(()),
    }
}

async fn judge_for_merge(ctx: &Context, task: &Task, pr: &Pr, states: &[CheckState]) -> Result<()> {
    let seen = match all_green(states) {
        true => facts(ctx, task, pr.number).await?,
        false => None,
    };
    match decide(states, seen.as_ref()) {
        Verdict::Merge => match seen {
            Some(f) => merge(ctx, task, pr, states, &f.head).await,
            None => Ok(()),
        },
        Verdict::Wait => match task.state {
            TaskState::Ended => wait(ctx, task, pr, states).await,
            _ => Ok(()),
        },
        Verdict::Relaunch => {
            if task.state == TaskState::Ended {
                let store = ctx.store()?;
                if transition(store, &task.name, TaskState::Ended, TaskState::InCi)
                    .await?
                    .is_none()
                {
                    return Ok(());
                }
            }
            let advice = seen.map(|f| fix_advice(task.pr_target(), &f));
            relaunch(ctx, task, pr, states, advice).await
        }
    }
}

fn fix_advice(base: &str, facts: &PrFacts) -> String {
    match facts.conflicting {
        true => format!("it conflicts with {base}: resolve the conflicts and push"),
        false => format!(
            "it is {} commits behind {base}: merge {base} into the branch and push",
            facts.behind
        ),
    }
}

async fn merge(
    ctx: &Context,
    task: &Task,
    pr: &Pr,
    states: &[CheckState],
    head: &str,
) -> Result<()> {
    let head = head.to_string();
    let merged = on_forge(ctx, task, pr.number, move |req, programs| {
        merge_pr(req, &head, programs)
    })
    .await?
    .flatten();
    let Some(sha) = merged else {
        return Ok(());
    };
    let checks = judged(states);
    let kept = sha.clone();
    let change = move |t: Task| match t.state {
        TaskState::Ended | TaskState::InCi => Ok(Task {
            state: TaskState::Merged,
            pr: t.pr.map(|p| Pr {
                checks,
                merge_sha: Some(kept),
                ..p
            }),
            ..t
        }),
        _ => Err(Error::Conflict(MOVED.into())),
    };
    let store = ctx.store()?;
    match store.update_task(&task.name, change).await {
        Ok(_) => {}
        Err(Error::Conflict(m)) if m == MOVED => return Ok(()),
        Err(e) => return Err(e),
    }
    let said = format!(
        "task {} merged PR #{} into {}: merged, sha {sha}",
        task.name,
        pr.number,
        task.pr_target()
    );
    let body = EntryBody::Agent {
        agent: task.name.clone(),
        event: AgentEvent::Passed,
        text: plan::capped(&said, 400),
    };
    store.append_entry(&task.conversation_id, body).await?;
    runner::notify(ctx, &task.conversation_id, said);
    tracing::info!(
        target: "townhall::task", event = "task.merged", task = %task.name,
        sha = %sha, "the daemon merged a green pull request"
    );
    Ok(())
}

async fn waits_done(ctx: &Context, task: &Task) -> Result<usize> {
    let handoffs = ctx.store()?.handoffs(&task.name).await?;
    Ok(handoffs
        .iter()
        .filter(|h| h.reason.starts_with(REASON_PREFIX))
        .count())
}

async fn wait(ctx: &Context, task: &Task, pr: &Pr, states: &[CheckState]) -> Result<()> {
    let seen = describe(states);
    if waits_done(ctx, task).await? >= ctx.pacing().ci_waits {
        let why = format!(
            "CI is still pending on PR #{} after {} waits ({seen}): check the run, then tell it to continue or stop",
            pr.number,
            ctx.pacing().ci_waits
        );
        return needs_you(ctx, task, TaskState::Ended, why).await;
    }
    let store = ctx.store()?;
    if transition(store, &task.name, TaskState::Ended, TaskState::InCi)
        .await?
        .is_some()
    {
        let text = format!(
            "ended with pending checks: waiting for CI on PR #{} ({seen})",
            pr.number
        );
        let body = EntryBody::Agent {
            agent: task.name.clone(),
            event: AgentEvent::Ended,
            text: plan::capped(&text, 400),
        };
        store.append_entry(&task.conversation_id, body).await?;
    }
    Ok(())
}

fn judged(states: &[CheckState]) -> Checks {
    match (states.is_empty(), states.iter().any(CheckState::failed)) {
        (true, _) => Checks::None,
        (false, true) => Checks::Failed,
        (false, false) => Checks::Passed,
    }
}

async fn record(ctx: &Context, task: &Task, states: &[CheckState]) -> Result<()> {
    let checks = judged(states);
    let change = move |t: Task| match t.state {
        TaskState::Ended | TaskState::InCi => Ok(Task {
            pr: t.pr.map(|p| Pr { checks, ..p }),
            ..t
        }),
        _ => Err(Error::Conflict(MOVED.into())),
    };
    match ctx.store()?.update_task(&task.name, change).await {
        Ok(_) => Ok(()),
        Err(Error::Conflict(m)) if m == MOVED => Ok(()),
        Err(e) => Err(e),
    }
}

async fn relaunch(
    ctx: &Context,
    task: &Task,
    pr: &Pr,
    states: &[CheckState],
    advice: Option<String>,
) -> Result<()> {
    record(ctx, task, states).await?;
    let advice = advice.unwrap_or_else(|| "CI finished: merge if green, fix if red".to_string());
    let reason = format!(
        "{REASON_PREFIX}{} finished: {}. {advice}",
        pr.number,
        describe(states)
    );
    let store = ctx.store()?;
    match store.hand_after_ci(&task.name, &task.agent, &reason).await {
        Ok(_) => Ok(()),
        Err(Error::Conflict(why)) => {
            let still = store.find_task(&task.name).await?;
            match still.is_some_and(|t| t.state == TaskState::InCi) {
                true => needs_you(ctx, task, TaskState::InCi, why).await,
                false => Ok(()),
            }
        }
        Err(e) => Err(e),
    }
}

async fn needs_you(ctx: &Context, task: &Task, from: TaskState, why: String) -> Result<()> {
    let store = ctx.store()?;
    let blocked = plan::capped(&why, 500);
    let held = blocked.clone();
    let change = move |t: Task| match t.state == from {
        true => Ok(Task {
            state: TaskState::NeedsYou,
            blocked: Some(held),
            ..t
        }),
        false => Err(Error::Conflict(MOVED.into())),
    };
    match store.update_task(&task.name, change).await {
        Ok(_) => {
            let said = format!("task {} needs you: {blocked}", task.name);
            let body = EntryBody::NeedsYou {
                agent: task.name.clone(),
                question: said.clone(),
                choices: vec![],
            };
            store.append_entry(&task.conversation_id, body).await?;
            runner::notify(ctx, &task.conversation_id, said);
            Ok(())
        }
        Err(Error::Conflict(m)) if m == MOVED => Ok(()),
        Err(e) => Err(e),
    }
}
