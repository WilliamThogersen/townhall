//! Agent lifecycle: one worktree plus one tmux session per agent.
//! Nothing is stored; state is rediscovered from git and tmux.

mod adapter;

use std::path::{Path, PathBuf};

pub use adapter::{shell_quote, AgentAdapter, ClaudeCodeAdapter, LaunchOptions, ShellAdapter};

use crate::cmd::{run, validate_name};
use crate::{tmux, worktree, Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    /// Known only for agents started in this call; not rediscoverable.
    pub kind: Option<String>,
    pub branch: String,
    pub worktree: PathBuf,
    pub alive: bool,
}

const TMUX_ABSENT: [&str; 4] = [
    "can't find session",
    "no server running",
    "no current target",
    "error connecting to",
];

fn tmux_absent(e: &Error) -> bool {
    matches!(e, Error::Command { stderr, .. } if TMUX_ABSENT.iter().any(|m| stderr.contains(m)))
}

fn git_absent(e: &Error) -> bool {
    matches!(e, Error::Command { stderr, .. }
        if stderr.contains("is not a working tree") || stderr.contains("not found")
            || stderr.contains("No such file"))
}

fn from_worktree(wt: worktree::Worktree, kind: Option<String>, alive: bool) -> Agent {
    Agent {
        name: wt.name,
        kind,
        branch: wt.branch,
        worktree: wt.path,
        alive,
    }
}

/// Create the worktree, then the tmux session. Rolls the worktree back if tmux fails.
pub async fn start(
    repo: &Path,
    name: &str,
    base: &str,
    adapter: &dyn AgentAdapter,
    options: &LaunchOptions,
) -> Result<Agent> {
    validate_name(name)?;
    let command = adapter.launch_command(options)?;
    let wt = worktree::create(repo, name, base).await?;
    if let Err(cause) = tmux::spawn(name, &wt.path, &command).await {
        return Err(match worktree::remove(repo, name).await {
            Ok(()) => cause,
            Err(cleanup) => Error::RollbackFailed {
                cause: cause.to_string(),
                cleanup: cleanup.to_string(),
            },
        });
    }
    Ok(from_worktree(wt, Some(adapter.kind().to_string()), true))
}

/// Kill the session and remove worktree and branch. Either may already be gone,
/// but if both are, the agent does not exist and `NotFound` is returned.
pub async fn stop(repo: &Path, name: &str) -> Result<()> {
    validate_name(name)?;
    let killed = match tmux::kill(name).await {
        Ok(()) => true,
        Err(e) if tmux_absent(&e) => false,
        Err(e) => return Err(e),
    };
    let removed = match worktree::remove(repo, name).await {
        Ok(()) => true,
        Err(e) if git_absent(&e) => delete_orphan_branch(repo, name).await?,
        Err(e) => return Err(e),
    };
    if killed || removed {
        Ok(())
    } else {
        Err(Error::NotFound(name.to_string()))
    }
}

/// Worktree already gone: the branch may still linger. True if it was deleted.
async fn delete_orphan_branch(repo: &Path, name: &str) -> Result<bool> {
    let branch = format!("townhall/{name}");
    match run("git", &["branch", "-D", &branch], Some(repo)).await {
        Ok(_) => Ok(true),
        Err(e) if git_absent(&e) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Worktrees joined with live sessions. A worktree without a session is `alive: false`.
pub async fn list(repo: &Path) -> Result<Vec<Agent>> {
    let (worktrees, sessions) = (worktree::list(repo).await?, tmux::list().await?);
    Ok(worktrees
        .into_iter()
        .map(|wt| {
            let alive = sessions.contains(&wt.name);
            from_worktree(wt, None, alive)
        })
        .collect())
}

fn missing_session(name: &str, e: Error) -> Error {
    if tmux_absent(&e) {
        Error::NotFound(name.to_string())
    } else {
        e
    }
}

/// Type text into the agent's session and press Enter.
pub async fn send(name: &str, text: &str) -> Result<()> {
    tmux::send(name, text)
        .await
        .map_err(|e| missing_session(name, e))
}

/// Current pane contents of the agent's session.
pub async fn output(name: &str) -> Result<String> {
    tmux::capture(name)
        .await
        .map_err(|e| missing_session(name, e))
}
