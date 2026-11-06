//! The gate on an agent type's `requires`. A queued task whose agent type requires other
//! types starts only when, for each required type, the newest task of that type in its
//! `after` chain ended with a report that clears it for the commit the task will start from.
//!
//! - The report is that task's final message, else its latest `report` artifact.
//! - Its last lines must say `VERDICT: APPROVE` or `VERDICT: PASS` and `COMMIT: <sha>`
//!   (`report.rs`). A REJECT, a FAIL or a missing line blocks.
//! - The commit is compared with the head the task starts from: its own branch when it has
//!   one (a task handed back), else its base branch. It is stale unless it is that head, or
//!   an ancestor of it and every commit since is a merge or already on the repo's base
//!   branch, so merging the base branch in keeps a report fresh and any other change does not.
//!   When the head is the repo's base branch itself that allowance would excuse any older
//!   commit, so there the report must name the tip exactly.
//! - The gate runs for a task with no agent yet and for a handed-back task before its new
//!   agent resumes; an agent that is already running is adopted, never gated.
//!
//! A blocked task stays queued, its reason stored in `blocked`; the planner is told when the
//! reason first appears or changes.

use crate::agent::state;
use crate::cmd::run;
use crate::context::Context;
use crate::model::{ArtifactKind, Task, TaskState};
use crate::{runner, steps, Error, Result, Workspace};

use super::launch::commit_of;
use super::report;
use super::requires;
use super::{plan, MOVED};

pub(super) enum Gate {
    Open,
    Blocked(String),
}

/// What the gated task will start from, resolved when a report names a commit.
struct Head<'a> {
    ws: Workspace,
    task: &'a Task,
    base: String,
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

impl Head<'_> {
    async fn at(&self) -> Result<(String, String)> {
        match commit_of(&self.ws, &self.task.branch()).await {
            Ok(sha) => Ok((self.task.branch(), sha)),
            Err(Error::InvalidInput(_)) => {
                let sha = commit_of(&self.ws, &self.task.base_branch).await?;
                Ok((self.task.base_branch.clone(), sha))
            }
            Err(e) => Err(e),
        }
    }

    async fn is_ancestor(&self, older: &str, newer: &str) -> Result<bool> {
        let args = ["merge-base", "--is-ancestor", older, newer];
        match run("git", &args, Some(self.ws.repo())).await {
            Ok(_) => Ok(true),
            Err(Error::Command { code: Some(1), .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

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
        let (at, head) = self.at().await?;
        if covered == head {
            return Ok(None);
        }
        let behind = format!(
            "{who} report is stale: it covers {} but `{at}` is at {}",
            short(&covered),
            short(&head)
        );
        if at == self.base {
            return Ok(Some(format!(
                "{behind}: a task starting from the repo's base branch needs a report for its tip"
            )));
        }
        if !self.is_ancestor(&covered, &head).await? {
            return Ok(Some(format!("{behind}, which does not contain it")));
        }
        let range = format!("{covered}..{head}");
        let not_base = format!("^{}", self.base);
        let args = ["rev-list", "--no-merges", range.as_str(), not_base.as_str()];
        let own = run("git", &args, Some(self.ws.repo())).await?;
        Ok(match own.is_empty() {
            true => None,
            false => Some(format!(
                "{behind}, with changes beyond a merge of `{}`",
                self.base
            )),
        })
    }
}

/// The task's final message, else its latest `report` artifact.
async fn report_of(ctx: &Context, done: &Task) -> Result<Option<String>> {
    let repo = ctx.repo(&done.repo).await?;
    let worktree = ctx.workspace(&repo)?.worktrees_dir().join(&done.name);
    let projects = steps::claude_projects_dir()?;
    if let Some(said) = state::last_message(&projects, &worktree).await? {
        return Ok(Some(said));
    }
    let store = ctx.store()?;
    let latest = store
        .artifacts(&done.name)
        .await?
        .into_iter()
        .rev()
        .find(|a| a.kind == ArtifactKind::Report);
    let Some(found) = latest else {
        return Ok(None);
    };
    let (_, path) = store.artifact(&done.name, &found.id).await?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| Error::io(format!("read report {}", path.display()), e))?;
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

/// What keeps `done`'s report from clearing the gated task, or `None` when it clears it.
async fn judge(ctx: &Context, done: &Task, head: &Head<'_>) -> Result<Option<String>> {
    let who = format!("`{}` ({})", done.name, done.agent);
    if !plan::finished(done.state) {
        return Ok(Some(format!("{who} did not end (state {:?})", done.state)));
    }
    let Some(text) = report_of(ctx, done).await? else {
        return Ok(Some(format!(
            "{who} left no report: it must finish with the report as its last message or add a report artifact"
        )));
    };
    let parsed = report::parse(&text);
    let Some(verdict) = parsed.verdict else {
        return Ok(Some(format!(
            "{who} report has no `VERDICT:` line in its last lines"
        )));
    };
    if !verdict.clears() {
        return Ok(Some(format!("{who} reported {}", verdict.as_str())));
    }
    let Some(commit) = parsed.commit else {
        return Ok(Some(format!(
            "{who} report has no `COMMIT:` line in its last lines"
        )));
    };
    head.stale(&who, &commit).await
}

/// Whether `task` may start. `tasks` is every task, so the `after` chain can be walked.
pub(super) async fn check(ctx: &Context, task: &Task, tasks: &[Task]) -> Result<Gate> {
    let Some(kind) = ctx.store()?.agent_type(&task.agent).await? else {
        return Ok(Gate::Open);
    };
    if kind.def.requires.is_empty() {
        return Ok(Gate::Open);
    }
    let repo = ctx.repo(&task.repo).await?;
    let head = Head {
        ws: ctx.workspace(&repo)?,
        task,
        base: repo.base_branch.clone(),
    };
    let chain = requires::chain(tasks, &task.after);
    let mut problems = vec![];
    for wanted in &kind.def.requires {
        let newest = chain
            .iter()
            .rev()
            .find(|t| t.agent.eq_ignore_ascii_case(wanted));
        let problem = match newest {
            Some(done) => judge(ctx, done, &head).await?,
            None => Some(format!("no task of type `{wanted}` in its `after` chain")),
        };
        problems.extend(problem);
    }
    Ok(match problems.is_empty() {
        true => Gate::Open,
        false => Gate::Blocked(problems.join("; ")),
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
    if task.blocked.as_deref() == Some(why) {
        return Ok(());
    }
    if set_blocked(ctx, &task.name, Some(why.to_string())).await? {
        let text = format!(
            "task `{}` stays queued, its agent type's requirements are not met: {why}",
            task.name
        );
        runner::notify(ctx, &task.conversation_id, text);
    }
    Ok(())
}

/// Drops a stored reason that no longer holds.
pub(super) async fn clear(ctx: &Context, task: &Task) -> Result<()> {
    if task.blocked.is_some() {
        set_blocked(ctx, &task.name, None).await?;
    }
    Ok(())
}
