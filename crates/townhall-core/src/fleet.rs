//! Agents across every registered repo: listing, finding one by name, and unregistering a
//! repo safely. The HTTP routes, the scheduler and the watcher share these.

use crate::agent::{self, state};
use crate::cmd::validate_name;
use crate::context::Context;
use crate::home::repo_key;
use crate::model::{AgentInfo, Repo};
use crate::{tmux, worktree, Error, Result, Workspace};

/// Where an agent lives: its repo's name and workspace.
#[derive(Debug, Clone)]
pub struct Located {
    pub repo: String,
    pub workspace: Workspace,
}

/// Every agent of every repo, with its repo name and state. A repo that cannot be read
/// (its folder is gone) is skipped with a warning instead of failing the whole list.
pub async fn list(ctx: &Context) -> Result<Vec<AgentInfo>> {
    let mut all = vec![];
    for repo in ctx.repos().await? {
        match agents_of(ctx, &repo).await {
            Ok(found) => all.extend(found),
            Err(e) => eprintln!("warn: repo `{}` skipped: {e}", repo.name),
        }
    }
    Ok(all)
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

async fn has_agent(ws: &Workspace, name: &str) -> Result<bool> {
    if tmux::list(ws.key()).await?.iter().any(|n| n == name) {
        return Ok(true);
    }
    Ok(worktree::list(ws).await?.iter().any(|w| w.name == name))
}

/// Repos that have an agent (worktree or session) called `name`.
pub async fn holders(ctx: &Context, name: &str) -> Result<Vec<Located>> {
    let mut found = vec![];
    for repo in ctx.repos().await? {
        let ws = match ctx.workspace(&repo) {
            Ok(ws) => ws,
            Err(e) => {
                eprintln!("warn: repo `{}` skipped: {e}", repo.name);
                continue;
            }
        };
        match has_agent(&ws, name).await {
            Ok(true) => found.push(Located {
                repo: repo.name.clone(),
                workspace: ws,
            }),
            Ok(false) => {}
            Err(e) => eprintln!("warn: repo `{}` skipped: {e}", repo.name),
        }
    }
    Ok(found)
}

/// The agent's repo: `repo` when given, else the one repo that has it, else the startup
/// repo (so a missing agent is still `404` from the operation). Two repos is a conflict.
pub async fn locate(ctx: &Context, name: &str, repo: Option<&str>) -> Result<Located> {
    validate_name(name)?;
    if let Some(wanted) = repo.map(str::trim).filter(|r| !r.is_empty()) {
        let found = ctx.repo(wanted).await?;
        return Ok(Located {
            workspace: ctx.workspace(&found)?,
            repo: found.name,
        });
    }
    let mut found = holders(ctx, name).await?;
    match found.len() {
        0 => Ok(Located {
            repo: ctx.startup_repo_name().await,
            workspace: ctx.startup().clone(),
        }),
        1 => Ok(found.remove(0)),
        _ => {
            let repos: Vec<String> = found.into_iter().map(|l| l.repo).collect();
            Err(Error::Conflict(format!(
                "agent `{name}` exists in several repos ({}): pass ?repo=<name>",
                repos.join(", ")
            )))
        }
    }
}

/// Unregisters a repo. Refused for the daemon's startup repo and while agents run in it.
pub async fn unregister(ctx: &Context, name: &str) -> Result<Repo> {
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
    ctx.registry()?.remove(&repo.name).await
}
