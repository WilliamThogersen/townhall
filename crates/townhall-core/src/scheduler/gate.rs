//! The gate on an agent type's `requires`. A queued task whose agent type requires other
//! types starts only when, for each required type, the newest task of that type in its
//! `after` chain ended with a report that clears it for the commit the task will start from.
//!
//! - The report is the final message of the session recorded for that task, never the newest
//!   transcript of its folder. An artifact is never read: `add_artifact` on
//!   `/api/tasks/{name}/mcp` has no auth, so any local agent could file one for another task.
//! - Its lines `VERDICT: APPROVE` or `VERDICT: PASS` and `COMMIT: <sha>` may be anywhere in
//!   it (`report.rs`). A REJECT, a FAIL, a missing or conflicting line blocks.
//! - The commit is compared with the head the task starts from: its own branch when it has
//!   one (a task handed back), else its base branch. It is stale unless it is exactly that
//!   head. Any later commit, a merge of the base branch included, makes it stale: a merge can
//!   carry changes of its own, so none is excused.
//! - The gate runs for a task with no agent yet and for a handed-back task before its new
//!   agent resumes; an agent that is already running is adopted, never gated.
//!
//! A blocked task stays queued, its reason stored in `blocked`; the planner is told when the
//! reason first appears or changes. The reason never holds a head sha, so a base branch that
//! moves does not tell the planner again. A REJECT or FAIL is final: the task ends in `error`
//! with that reason kept in `blocked` (the mark the watcher reads to leave it in `error`) and
//! the planner is told once, so a new task is made after the fix.
//!
//! Only `tester` and `reviewer` can be required (`report::vocabulary`); a type that names
//! another in `requires` blocks its tasks with a reason saying so.

use serde_json::json;

use crate::agent::{self, state};
use crate::cmd::run;
use crate::context::Context;
use crate::model::{AgentEvent, EntryBody, Task, TaskState};
use crate::plugins::hooks;
use crate::{runner, steps, worktree, Error, Result, Workspace};

use super::launch::{commit_of, MAX_FAILED_TEXT};
use super::report;
use super::requires;
use super::{plan, MOVED};

pub(super) enum Gate {
    Open(String),
    Blocked(String),
    Rejected(String),
}

enum Problem {
    Waiting(String),
    Rejected(String),
}

impl Problem {
    fn waiting(why: String) -> Option<Self> {
        Some(Self::Waiting(why))
    }
}

pub(super) struct At {
    ws: Workspace,
    label: String,
    sha: String,
    handed_back: bool,
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

pub(super) async fn head_of(ctx: &Context, task: &Task, handed_back: bool) -> Result<At> {
    let repo = ctx.repo(&task.repo).await?;
    let ws = ctx.workspace(&repo)?;
    let (label, sha) = match handed_back {
        true => {
            let branch = task.branch();
            let sha = commit_of(&ws, &branch).await.map_err(|e| match e {
                Error::InvalidInput(_) => Error::InvalidInput(format!(
                    "the branch `{branch}` of this handed-back task is not a commit in {}",
                    ws.repo().display()
                )),
                other => other,
            })?;
            (branch, sha)
        }
        false => {
            let base = worktree::fresh_base(&ws, &task.base_branch).await;
            (task.base_branch.clone(), commit_of(&ws, &base).await?)
        }
    };
    Ok(At {
        ws,
        label,
        sha,
        handed_back,
    })
}

impl At {
    /// Why a report covering `commit` is stale, or `None` when it is fresh.
    async fn stale(&self, who: &str, commit: &str) -> Result<Option<String>> {
        let covered = match commit_of(&self.ws, commit).await {
            Ok(sha) => sha,
            Err(Error::InvalidInput(_)) => {
                return Ok(Some(format!(
                    "{who} report names `{}`, which is not a commit of this repo",
                    short(commit)
                )))
            }
            Err(e) => return Err(e),
        };
        if covered == self.sha {
            return Ok(None);
        }
        Ok(Some(format!(
            "{who} report is stale: it covers {}, not the tip of `{}`",
            short(&covered),
            self.label
        )))
    }

    async fn uncommitted(&self, task: &Task) -> Result<Option<String>> {
        let dir = self.ws.worktrees_dir().join(&task.name);
        let status = run("git", &["status", "--porcelain"], Some(&dir)).await?;
        Ok(match status.trim().is_empty() {
            true => None,
            false => Some(format!(
                "`{}` has uncommitted changes in its worktree that no COMMIT covers: commit or discard them",
                task.name
            )),
        })
    }
}

/// The final message of the Claude session recorded for the task: the only place a report is
/// read from. `Err` is why there is none to read.
pub(super) async fn report_of(
    ctx: &Context,
    done: &Task,
    who: &str,
) -> Result<std::result::Result<String, String>> {
    let launch = ctx.store()?.task_launch(&done.name).await?;
    let Some(session) = launch.and_then(|l| l.session_id) else {
        return Ok(Err(format!(
            "{who} has no recorded session to read its report from"
        )));
    };
    let repo = ctx.repo(&done.repo).await?;
    let worktree = ctx.workspace(&repo)?.worktrees_dir().join(&done.name);
    let projects = steps::claude_projects_dir()?;
    Ok(
        match state::last_message_of_session(&projects, &worktree, &session).await? {
            None => Err(format!(
                "{who} session transcript was not found: its report cannot be read"
            )),
            Some(None) => Err(format!(
                "{who} left no report: it must finish with the report as its final message"
            )),
            Some(Some(text)) => Ok(text),
        },
    )
}

/// What keeps `done`'s report from clearing the gated task, or `None` when it clears it.
async fn judge(ctx: &Context, done: &Task, head: &At) -> Result<Option<Problem>> {
    let who = format!("`{}` ({})", done.name, done.agent);
    if !plan::finished(done.state) {
        let why = format!("{who} did not end (state {:?})", done.state);
        return Ok(Problem::waiting(why));
    }
    let text = match report_of(ctx, done, &who).await? {
        Ok(text) => text,
        Err(why) => return Ok(Problem::waiting(why)),
    };
    let parsed = report::parse(&text, &done.agent);
    let verdict = match parsed.verdict {
        Ok(Some(verdict)) => verdict,
        Ok(None) => {
            return Ok(Problem::waiting(format!(
                "{who} report has no `VERDICT:` line"
            )))
        }
        Err(why) => return Ok(Problem::waiting(format!("{who} report: {why}"))),
    };
    if !verdict.clears() {
        let why = format!(
            "{who} reported {}; create a new task after the fix",
            verdict.as_str()
        );
        return Ok(Some(Problem::Rejected(why)));
    }
    let commit = match parsed.commit {
        Ok(Some(commit)) => commit,
        Ok(None) => {
            return Ok(Problem::waiting(format!(
                "{who} report has no `COMMIT:` line"
            )))
        }
        Err(why) => return Ok(Problem::waiting(format!("{who} report: {why}"))),
    };
    Ok(head.stale(&who, &commit).await?.and_then(Problem::waiting))
}

/// Whether `task` may start. `tasks` is every task, so the `after` chain can be walked.
pub(super) async fn check(ctx: &Context, task: &Task, tasks: &[Task], at: &At) -> Result<Gate> {
    let Some(kind) = ctx.store()?.agent_type(&task.agent).await? else {
        return Ok(Gate::Open(at.sha.clone()));
    };
    if kind.def.requires.is_empty() {
        return Ok(Gate::Open(at.sha.clone()));
    }
    let chain = requires::chain(tasks, &task.after);
    let mut waiting = vec![];
    let mut rejected = vec![];
    if at.handed_back {
        waiting.extend(at.uncommitted(task).await?);
    }
    for wanted in &kind.def.requires {
        if let Some(why) = report::unrequirable(wanted) {
            waiting.push(format!("agent type `{}`: {why}", task.agent));
            continue;
        }
        let newest = chain
            .iter()
            .rev()
            .find(|t| t.agent.eq_ignore_ascii_case(wanted));
        let problem = match newest {
            Some(done) => judge(ctx, done, at).await?,
            None => Problem::waiting(format!("no task of type `{wanted}` in its `after` chain")),
        };
        match problem {
            Some(Problem::Waiting(why)) => waiting.push(why),
            Some(Problem::Rejected(why)) => rejected.push(why),
            None => {}
        }
    }
    Ok(match (rejected.is_empty(), waiting.is_empty()) {
        (false, _) => Gate::Rejected(rejected.join("; ")),
        (true, false) => Gate::Blocked(waiting.join("; ")),
        (true, true) => Gate::Open(at.sha.clone()),
    })
}

/// Writes `blocked` while the task is still queued; false when its state moved on.
async fn set_blocked(ctx: &Context, name: &str, blocked: Option<String>) -> Result<bool> {
    let change = move |t: Task| match t.state == TaskState::Queued {
        true => Ok(Task { blocked, ..t }),
        false => Err(Error::Conflict(MOVED.into())),
    };
    match ctx.store()?.update_task(name, change).await {
        Ok(_) => Ok(true),
        Err(Error::Conflict(m)) if m == MOVED => Ok(false),
        Err(e) => Err(e),
    }
}

/// Stores why `task` stays queued and tells the planner, once per reason.
pub(super) async fn hold(ctx: &Context, task: &Task, why: &str) -> Result<()> {
    let text = format!(
        "task `{}` stays queued, its agent type's requirements are not met: {why}",
        task.name
    );
    hold_with(ctx, task, why, text).await
}

pub(super) async fn hold_for_plugin(ctx: &Context, task: &Task, why: &str) -> Result<()> {
    let text = format!("task `{}` stays queued, held by {why}", task.name);
    hold_with(ctx, task, why, text).await
}

async fn hold_with(ctx: &Context, task: &Task, why: &str, text: String) -> Result<()> {
    if task.blocked.as_deref() == Some(why) {
        return Ok(());
    }
    if set_blocked(ctx, &task.name, Some(why.to_string())).await? {
        runner::notify(ctx, &task.conversation_id, text);
    }
    Ok(())
}

/// Ends `task` in `error` with the reason in `blocked`, so the planner makes a new task after
/// the fix; a task that moved on meanwhile is left alone. The session of a handed-back task is
/// killed, and the watcher leaves an `error` task that has a `blocked` reason as it is, so it
/// never reaches ended or needsYou and no task `after` it starts. The planner hears of it once.
pub(super) async fn reject(ctx: &Context, task: &Task, why: &str) -> Result<bool> {
    let reason = why.to_string();
    let change = move |t: Task| match t.state == TaskState::Queued {
        true => Ok(Task {
            state: TaskState::Error,
            blocked: Some(reason),
            ..t
        }),
        false => Err(Error::Conflict(MOVED.into())),
    };
    let store = ctx.store()?;
    match store.update_task(&task.name, change).await {
        Ok(_) => {}
        Err(Error::Conflict(m)) if m == MOVED => return Ok(false),
        Err(e) => return Err(e),
    }
    hooks::fire(
        ctx,
        "task.failed",
        json!({"task": task, "reason": why, "phase": "gate"}),
    );
    let text = format!("task `{}` failed, it will not start: {why}", task.name);
    runner::notify(ctx, &task.conversation_id, text.clone());
    let body = EntryBody::Agent {
        agent: task.name.clone(),
        event: AgentEvent::Failed,
        text: plan::capped(&text, MAX_FAILED_TEXT),
    };
    store.append_entry(&task.conversation_id, body).await?;
    let killed = match ctx.repo(&task.repo).await.and_then(|r| ctx.workspace(&r)) {
        Ok(ws) => agent::kill_session(&ws, &task.name).await,
        Err(e) => Err(e),
    };
    if let Err(e) = killed {
        tracing::warn!(
            target: "townhall::task", event = "task.reject_kill_failed", task = %task.name,
            error = %e, "the session of a rejected task could not be killed"
        );
    }
    Ok(true)
}

/// Drops a stored reason that no longer holds.
pub(super) async fn clear(ctx: &Context, task: &Task) -> Result<()> {
    if task.blocked.is_some() {
        set_blocked(ctx, &task.name, None).await?;
    }
    Ok(())
}
