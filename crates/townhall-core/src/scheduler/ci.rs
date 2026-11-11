use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::context::Context;
use crate::mcp;
use crate::model::{AgentEvent, Checks, EntryBody, Pr, Task, TaskState};
use crate::repos::accounts::Accounts;
use crate::repos::pr_checks::{pr_checks, CheckState, ChecksRequest};
use crate::repos::publish::Programs;
use crate::{runner, Error, Result};

use super::reflect::REFLECTOR;
use super::{plan, transition, MOVED};

const REASON_PREFIX: &str = "the checks of PR #";

#[derive(Default)]
struct Watch {
    busy: bool,
    next: Option<Instant>,
    since: Option<Instant>,
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

fn begin_wait(key: &(PathBuf, String)) {
    watching(key, |w| w.since = Some(Instant::now()));
}

fn end_wait(key: &(PathBuf, String)) {
    watching(key, |w| w.since = None);
}

fn waited(key: &(PathBuf, String)) -> Duration {
    watching(key, |w| w.since.get_or_insert_with(Instant::now).elapsed())
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
            if let Err(e) = judge(&ctx, &task, &key).await {
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

async fn read(ctx: &Context, task: &Task, number: u64) -> Result<Option<Vec<CheckState>>> {
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
        pr_checks(&req, &Programs::from_env())
    })
    .await
    .map_err(|e| {
        Error::io(
            "read pull request checks",
            std::io::Error::other(e.to_string()),
        )
    })?
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

async fn judge(ctx: &Context, task: &Task, key: &(PathBuf, String)) -> Result<()> {
    let Some(pr) = task.pr.clone() else {
        return Ok(());
    };
    let Some(states) = read(ctx, task, pr.number).await? else {
        return Ok(());
    };
    let pending = states.iter().any(CheckState::pending);
    match (task.state, pending) {
        (TaskState::Ended, true) => wait(ctx, task, &pr, &states, key).await,
        (TaskState::Ended, false) => record(ctx, task, &states).await,
        (TaskState::InCi, true) => bound(ctx, task, &pr, &states, key).await,
        (TaskState::InCi, false) => relaunch(ctx, task, &pr, &states, key).await,
        _ => Ok(()),
    }
}

async fn waits_done(ctx: &Context, task: &Task) -> Result<usize> {
    let handoffs = ctx.store()?.handoffs(&task.name).await?;
    Ok(handoffs
        .iter()
        .filter(|h| h.reason.starts_with(REASON_PREFIX))
        .count())
}

async fn wait(
    ctx: &Context,
    task: &Task,
    pr: &Pr,
    states: &[CheckState],
    key: &(PathBuf, String),
) -> Result<()> {
    let seen = describe(states);
    if waits_done(ctx, task).await? >= ctx.pacing().ci_waits {
        let why = format!(
            "its pull request #{} still has pending checks after {} waits for CI ({seen})",
            pr.number,
            ctx.pacing().ci_waits
        );
        return needs_you(ctx, task, TaskState::Ended, why, key).await;
    }
    let store = ctx.store()?;
    if transition(store, &task.name, TaskState::Ended, TaskState::InCi)
        .await?
        .is_some()
    {
        begin_wait(key);
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

async fn bound(
    ctx: &Context,
    task: &Task,
    pr: &Pr,
    states: &[CheckState],
    key: &(PathBuf, String),
) -> Result<()> {
    let limit = ctx.pacing().ci_timeout;
    if waited(key) < limit {
        return Ok(());
    }
    let why = format!(
        "its pull request #{} still has pending checks after waiting {} min for CI ({})",
        pr.number,
        limit.as_secs() / 60,
        describe(states)
    );
    needs_you(ctx, task, TaskState::InCi, why, key).await
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
    key: &(PathBuf, String),
) -> Result<()> {
    record(ctx, task, states).await?;
    let reason = format!(
        "{REASON_PREFIX}{} finished: {}. CI finished: merge if green, fix if red",
        pr.number,
        describe(states)
    );
    let store = ctx.store()?;
    match store.hand_after_ci(&task.name, &task.agent, &reason).await {
        Ok(_) => {
            end_wait(key);
            Ok(())
        }
        Err(Error::Conflict(why)) => {
            let still = store.find_task(&task.name).await?;
            match still.is_some_and(|t| t.state == TaskState::InCi) {
                true => needs_you(ctx, task, TaskState::InCi, why, key).await,
                false => Ok(()),
            }
        }
        Err(e) => Err(e),
    }
}

async fn needs_you(
    ctx: &Context,
    task: &Task,
    from: TaskState,
    why: String,
    key: &(PathBuf, String),
) -> Result<()> {
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
            end_wait(key);
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
