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
pub use claude::{
    mcp_tool_timeout_ms, ClaudeCodeAdapter, AGENT_ISOLATION, DEFAULT_CLAUDE_BIN, FOREGROUND_RULE,
    ISOLATED_SETTINGS, MCP_TOOL_TIMEOUT_ENV, OUTER_SESSION_ENV,
};

use crate::cmd::{probe, validate_name};
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

const TMUX_ABSENT: [&str; 7] = [
    "can't find session",
    "no such session",
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
        let cause = adapter::hide_env(cause, options);
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

/// Run `adapter` again in the existing worktree of `name`: the old session, if any, is
/// killed first. The worktree and its branch are kept, with everything committed in them.
pub async fn restart(
    ws: &Workspace,
    name: &str,
    adapter: &dyn AgentAdapter,
    options: &LaunchOptions,
) -> Result<Agent> {
    validate_name(name)?;
    let command = adapter.launch_command(options)?;
    let wt = worktree::list(ws)
        .await?
        .into_iter()
        .find(|w| w.name == name)
        .ok_or_else(|| Error::NotFound(name.to_string()))?;
    match tmux::kill(ws.key(), name).await {
        Ok(()) => {}
        Err(e) if tmux_absent(&e) => {}
        Err(e) => return Err(e),
    }
    tmux::spawn(ws.key(), name, &wt.path, &command, adapter.kind())
        .await
        .map_err(|e| adapter::hide_env(e, options))?;
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

/// Kill only the tmux session of `name`; a worktree is left as it is. False when it was gone.
pub async fn kill_session(ws: &Workspace, name: &str) -> Result<bool> {
    validate_name(name)?;
    match tmux::kill(ws.key(), name).await {
        Ok(()) => Ok(true),
        Err(e) if tmux_absent(&e) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Kill the session and remove the worktree for good; the branch stays only while it holds
/// unmerged commits. False when neither existed.
pub async fn retire(ws: &Workspace, name: &str) -> Result<bool> {
    let killed = kill_session(ws, name).await?;
    let removed = match worktree::retire(ws, name).await {
        Ok(_) => true,
        Err(e) if git_absent(&e) => false,
        Err(e) => return Err(e),
    };
    Ok(killed || removed)
}

/// Worktree already gone: the branch may still linger. True if it was deleted.
async fn delete_orphan_branch(ws: &Workspace, name: &str) -> Result<bool> {
    let branch = format!("townhall/{name}");
    match probe("git", &["branch", "-D", &branch], Some(ws.repo())).await {
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

pub async fn send_text(ws: &Workspace, name: &str, text: &str) -> Result<()> {
    tmux::send_text(ws.key(), name, text)
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

/// The `tmux` arguments that attach a client to the agent's live session, which is first
/// set up for a viewer. A dead session is `NotFound`.
pub async fn attach_args(ws: &Workspace, name: &str) -> Result<Vec<String>> {
    tmux::prepare_attach(ws.key(), name)
        .await
        .map_err(|e| missing_session(name, e))
}

#[cfg(test)]
mod absent_tests {
    use super::*;

    fn failed(stderr: &str) -> Error {
        Error::Command {
            command: "tmux set-option".into(),
            code: Some(1),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn a_session_that_died_before_set_option_is_absent() {
        assert!(tmux_absent(&failed("no such session: =townhall-k-a:")));
    }

    #[test]
    fn a_missing_session_reads_as_not_found() {
        let e = missing_session("a", failed("no such session: =townhall-k-a:"));
        assert!(matches!(e, Error::NotFound(_)), "{e:?}");
    }
}
