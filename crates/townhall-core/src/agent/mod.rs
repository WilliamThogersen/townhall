//! Agent lifecycle: one worktree plus one tmux session per agent.
//! Nothing is stored; state is rediscovered from git and tmux. A session whose process
//! exited stays as a dead pane (it keeps the exit code) until `stop` kills it.

mod adapter;
mod claude;
pub mod state;
pub mod watch;

use std::path::PathBuf;

pub use adapter::{shell_quote, AgentAdapter, LaunchOptions, ShellAdapter};
pub(crate) use claude::{add_dir, plain};
pub use claude::{ClaudeCodeAdapter, DEFAULT_CLAUDE_BIN, OUTER_SESSION_ENV};

use crate::cmd::{run, validate_name};
use crate::model::AgentKey;
use crate::{tmux, worktree, Error, Result, Workspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    /// The adapter that started it, kept by its tmux session; unknown once the session is gone.
    pub kind: Option<String>,
    pub branch: String,
    pub worktree: PathBuf,
    pub alive: bool,
}

const TMUX_ABSENT: [&str; 6] = [
    "can't find session",
    "can't find pane",
    "no server running",
    "no current target",
    "error connecting to",
    "server exited unexpectedly",
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
    ws: &Workspace,
    name: &str,
    base: &str,
    adapter: &dyn AgentAdapter,
    options: &LaunchOptions,
) -> Result<Agent> {
    validate_name(name)?;
    let command = adapter.launch_command(options)?;
    let wt = worktree::create(ws, name, base).await?;
    if let Err(cause) = tmux::spawn(ws.key(), name, &wt.path, &command, adapter.kind()).await {
        return Err(match worktree::remove(ws, name).await {
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
pub async fn stop(ws: &Workspace, name: &str) -> Result<()> {
    validate_name(name)?;
    let killed = match tmux::kill(ws.key(), name).await {
        Ok(()) => true,
        Err(e) if tmux_absent(&e) => false,
        Err(e) => return Err(e),
    };
    let removed = match worktree::remove(ws, name).await {
        Ok(()) => true,
        Err(e) if git_absent(&e) => delete_orphan_branch(ws, name).await?,
        Err(e) => return Err(e),
    };
    if killed || removed {
        Ok(())
    } else {
        Err(Error::NotFound(name.to_string()))
    }
}

/// Worktree already gone: the branch may still linger. True if it was deleted.
async fn delete_orphan_branch(ws: &Workspace, name: &str) -> Result<bool> {
    let branch = format!("townhall/{name}");
    match run("git", &["branch", "-D", &branch], Some(ws.repo())).await {
        Ok(_) => Ok(true),
        Err(e) if git_absent(&e) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Worktrees joined with their sessions. A worktree whose session is missing or dead is
/// `alive: false`; the kind comes from the session, dead or alive.
pub async fn list(ws: &Workspace) -> Result<Vec<Agent>> {
    let (worktrees, panes) = (worktree::list(ws).await?, tmux::panes(ws.key()).await?);
    Ok(worktrees
        .into_iter()
        .map(|wt| {
            let session = tmux::session_name(ws.key(), &wt.name);
            let pane = panes.iter().find(|p| p.session == session);
            let alive = pane.is_some_and(|p| !p.dead());
            let kind = pane.and_then(|p| p.kind.clone());
            from_worktree(wt, kind, alive)
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

/// Type text into the agent's session and press Enter. A dead session is `NotFound`.
pub async fn send(ws: &Workspace, name: &str, text: &str) -> Result<()> {
    tmux::send(ws.key(), name, text)
        .await
        .map_err(|e| missing_session(name, e))
}

/// Press one key in the agent's session. A dead session is `NotFound`.
pub async fn send_key(ws: &Workspace, name: &str, key: AgentKey) -> Result<()> {
    tmux::send_key(ws.key(), name, key)
        .await
        .map_err(|e| missing_session(name, e))
}

/// Current pane contents of the agent's session. A dead session is `NotFound`, as phase 1
/// answered once the session was gone, so the stream still ends when the agent does.
pub async fn output(ws: &Workspace, name: &str) -> Result<String> {
    tmux::capture_live(ws.key(), name)
        .await
        .map_err(|e| missing_session(name, e))
}
