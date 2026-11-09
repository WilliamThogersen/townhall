//! Agents across every registered repo: listing, finding one by name, and unregistering a
//! repo safely. The HTTP routes, the scheduler and the watcher share these.

use crate::agent::{self, state};
use crate::cmd::validate_name;
use crate::context::Context;
use crate::home::repo_key;
use crate::model::{AgentInfo, PatchRepo, Repo, TaskState};
use crate::{tmux, worktree, Error, Result, Workspace};

/// Where an agent lives: its repo's name, its name as stored, and the workspace.
#[derive(Debug, Clone)]
pub struct Located {
    pub repo: String,
    pub name: String,
    pub workspace: Workspace,
}

/// Every agent of every repo, with its repo name and state. A repo that cannot be read
/// (its folder is gone) is skipped with a warning instead of failing the whole list.
pub async fn list(ctx: &Context) -> Result<Vec<AgentInfo>> {
    let mut all = vec![];
    for repo in ctx.repos().await? {
        match agents_of(ctx, &repo).await {
            Ok(found) => all.extend(found),
            Err(e) => skipped(&repo.name, &e),
        }
    }
    Ok(all)
}

fn skipped(repo: &str, e: &crate::Error) {
    tracing::warn!(
        target: "townhall::store", event = "fleet.repo_skipped", repo = %repo,
        error_kind = e.kind(), error = %crate::redact::redact(&e.to_string()), "repo skipped"
    );
}

async fn agents_of(ctx: &Context, repo: &Repo) -> Result<Vec<AgentInfo>> {
    let ws = ctx.workspace(repo)?;
    let mut infos = vec![];
    for found in agent::list(&ws).await? {
        let state = state::detect(&ws, &found).await;
        infos.push(AgentInfo::new(found, &repo.name, state));
    }
    Ok(infos)
}

/// The stored name of the agent (worktree or session) called `name` ignoring case: git and
/// tmux tell names apart by case, the macOS file system does not.
async fn held_name(ws: &Workspace, name: &str) -> Result<Option<String>> {
    let sessions = tmux::list(ws.key()).await?;
    if let Some(found) = sessions.into_iter().find(|n| n.eq_ignore_ascii_case(name)) {
        return Ok(Some(found));
    }
    let trees = worktree::list(ws).await?;
    Ok(trees
        .into_iter()
        .map(|w| w.name)
        .find(|n| n.eq_ignore_ascii_case(name)))
}

/// Repos that have an agent called `name`, ignoring case, each with the stored name.
pub async fn holders(ctx: &Context, name: &str) -> Result<Vec<Located>> {
    let mut found = vec![];
    for repo in ctx.repos().await? {
        let ws = match ctx.workspace(&repo) {
            Ok(ws) => ws,
            Err(e) => {
                skipped(&repo.name, &e);
                continue;
            }
        };
        match held_name(&ws, name).await {
            Ok(Some(stored)) => found.push(Located {
                repo: repo.name.clone(),
                name: stored,
                workspace: ws,
            }),
            Ok(None) => {}
            Err(e) => skipped(&repo.name, &e),
        }
    }
    Ok(found)
}

/// An agent whose name differs only by case is not this agent: never act on it.
fn only_by_case(name: &str, stored: &str) -> Result<()> {
    if stored == name {
        Ok(())
    } else {
        Err(Error::NotFound(name.to_string()))
    }
}

async fn in_repo(ctx: &Context, name: &str, wanted: &str) -> Result<Located> {
    let found = ctx.repo(wanted).await?;
    let workspace = ctx.workspace(&found)?;
    if let Some(stored) = held_name(&workspace, name).await? {
        only_by_case(name, &stored)?;
    }
    Ok(Located {
        repo: found.name,
        name: name.to_string(),
        workspace,
    })
}

/// The agent's repo: `repo` when given, else the one repo that has it, else the startup
/// repo (so a missing agent is still `404` from the operation). Two repos is a conflict;
/// an agent whose name differs only by case is `NotFound`.
pub async fn locate(ctx: &Context, name: &str, repo: Option<&str>) -> Result<Located> {
    validate_name(name)?;
    if let Some(wanted) = repo.map(str::trim).filter(|r| !r.is_empty()) {
        return in_repo(ctx, name, wanted).await;
    }
    let mut found = holders(ctx, name).await?;
    match found.len() {
        0 => Ok(Located {
            repo: ctx.startup_repo_name().await,
            name: name.to_string(),
            workspace: ctx.startup().clone(),
        }),
        1 => {
            only_by_case(name, &found[0].name)?;
            Ok(found.remove(0))
        }
        _ => {
            let repos: Vec<String> = found.into_iter().map(|l| l.repo).collect();
            Err(Error::Conflict(format!(
                "agent `{name}` exists in several repos ({}): pass ?repo=<name>",
                repos.join(", ")
            )))
        }
    }
}

/// Edits a repo (see `RepoRegistry::edit`). A new path, by pointing or by moving the folder,
/// is refused for the daemon's startup repo and while agents run in it: their sessions are
/// keyed by the repo's path. Renaming and the base branch are always allowed.
pub async fn edit(ctx: &Context, name: &str, patch: &PatchRepo) -> Result<Repo> {
    let repo = ctx.repo(name).await?;
    if patch.path.is_some() {
        if ctx.ensure_startup_repo().await.as_deref() == Some(repo.name.as_str()) {
            return Err(Error::Conflict(format!(
                "`{}` is the repo the daemon started with: start it on another repo to move it",
                repo.name
            )));
        }
        let running = tmux::list(&repo_key(&repo.path)).await?.len();
        if running > 0 {
            return Err(Error::Conflict(format!(
                "repo `{}` has {running} running agent{}: stop {} before moving it",
                repo.name,
                if running == 1 { "" } else { "s" },
                if running == 1 { "it" } else { "them" }
            )));
        }
    }
    ctx.registry()?.edit(&repo.name, patch).await
}

/// Unregisters a repo. Refused for the daemon's startup repo and while agents run in it.
pub async fn unregister(ctx: &Context, name: &str) -> Result<Repo> {
    remove(ctx, name, false).await
}

/// Like `unregister`, and with `delete_clone` also deletes the clone townhall made in
/// `<home>/repos`. That is refused (nothing changes) for a folder of yours, a clone with
/// uncommitted or unpushed work, or a repo with a task that is not merged or ended.
pub async fn remove(ctx: &Context, name: &str, delete_clone: bool) -> Result<Repo> {
    let repo = ctx.repo(name).await?;
    if ctx.ensure_startup_repo().await.as_deref() == Some(repo.name.as_str()) {
        return Err(Error::Conflict(format!(
            "`{}` is the repo the daemon started with: start it on another repo to remove this one",
            repo.name
        )));
    }
    let running = tmux::list(&repo_key(&repo.path)).await?.len();
    if running > 0 {
        return Err(Error::Conflict(format!(
            "repo `{}` has {running} running agent{}: stop {} first",
            repo.name,
            if running == 1 { "" } else { "s" },
            if running == 1 { "it" } else { "them" }
        )));
    }
    let registry = ctx.registry()?;
    if !delete_clone {
        return registry.remove(&repo.name).await;
    }
    let open = open_tasks(ctx, &repo.name).await?;
    if open > 0 {
        return Err(Error::Conflict(format!(
            "repo `{}` has {open} task{} that {} not merged or ended: finish {} first",
            repo.name,
            if open == 1 { "" } else { "s" },
            if open == 1 { "is" } else { "are" },
            if open == 1 { "it" } else { "them" }
        )));
    }
    registry.remove_with_clone(&repo.name).await
}

async fn open_tasks(ctx: &Context, repo: &str) -> Result<usize> {
    let tasks = ctx.store()?.all_tasks().await?;
    Ok(tasks
        .iter()
        .filter(|t| t.repo.eq_ignore_ascii_case(repo))
        .filter(|t| !matches!(t.state, TaskState::Merged | TaskState::Ended))
        .count())
}
