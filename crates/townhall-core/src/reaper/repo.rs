//! One repo's part of a pass: what is there, then what goes.

use std::time::SystemTime;

use crate::agent::{self, Agent};
use crate::cmd::run;
use crate::model::Task;
use crate::{tmux, worktree, Error, Result, Workspace};

use super::verdict::verdict;
use super::{Ended, Reaped};

/// What one repo holds. Sessions are listed before worktrees: `agent::start` makes the
/// worktree first, so a session listed here whose worktree is missing below was not
/// being started, its worktree is gone.
pub(super) struct Survey {
    pub agents: Vec<Agent>,
    /// Names of townhall sessions of this repo without a worktree.
    pub sessions: Vec<String>,
}

pub(super) async fn survey(ws: &Workspace) -> Result<Survey> {
    let prefix = tmux::session_name(ws.key(), "");
    let panes = tmux::panes(ws.key()).await?;
    let agents = agent::list(ws).await?;
    let sessions = panes
        .iter()
        .filter_map(|p| p.session.strip_prefix(&prefix))
        .filter(|name| !agents.iter().any(|a| a.name == *name))
        .fold(vec![], |acc: Vec<String>, name| {
            match acc.iter().any(|n| n == name) {
                true => acc,
                false => [acc, vec![name.to_string()]].concat(),
            }
        });
    Ok(Survey { agents, sessions })
}

/// Removes the agents that go, kills the sessions left without a worktree and prunes
/// git's records of worktree folders that are gone. A failure is a warning; the rest goes on.
pub(super) async fn reap(
    ws: &Workspace,
    repo: &str,
    found: Survey,
    tasks: &[Task],
    why: &Ended<'_>,
) -> Reaped {
    let mut reaped = Reaped::default();
    for a in found.agents {
        let Some(reason) = verdict(&a.name, tasks, why.ended, why.settled) else {
            continue;
        };
        match agent::retire(ws, &a.name).await {
            Ok(true) => {
                tracing::info!(
                    target: "townhall::task", event = "reaper.agent_removed", agent = %a.name,
                    repo, why = reason.as_str(), "agent removed"
                );
                reaped.removed.push(a.name);
            }
            Ok(false) => {}
            Err(e) => reaped.warnings.push(format!(
                "agent `{}` of repo `{repo}` not removed: {e}",
                a.name
            )),
        }
    }
    for name in found.sessions {
        match agent::kill_session(ws, &name).await {
            Ok(true) => {
                tracing::info!(
                    target: "townhall::task", event = "reaper.session_killed", agent = %name,
                    repo, "session without a worktree killed"
                );
                reaped.sessions.push(name);
            }
            Ok(false) => {}
            Err(e) => reaped
                .warnings
                .push(format!("session `{name}` of repo `{repo}` not killed: {e}")),
        }
    }
    if let Err(e) = run("git", &["worktree", "prune"], Some(ws.repo())).await {
        reaped
            .warnings
            .push(format!("worktrees of repo `{repo}` not pruned: {e}"));
    }
    match sweep(ws, repo, tasks, why).await {
        Ok(swept) => reaped.removed.extend(swept),
        Err(e) => reaped
            .warnings
            .push(format!("stray folders of repo `{repo}` not swept: {e}")),
    }
    reaped
}

async fn sweep(ws: &Workspace, repo: &str, tasks: &[Task], why: &Ended<'_>) -> Result<Vec<String>> {
    let dir = ws.worktrees_dir();
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let known = worktree::list(ws).await?;
    let mut swept = vec![];
    for entry in
        std::fs::read_dir(&dir).map_err(|e| Error::io(format!("read {}", dir.display()), e))?
    {
        let entry = entry.map_err(|e| Error::io(format!("read {}", dir.display()), e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let meta = entry
            .metadata()
            .map_err(|e| Error::io(format!("stat {}", entry.path().display()), e))?;
        let age = meta
            .modified()
            .ok()
            .and_then(|at| SystemTime::now().duration_since(at).ok())
            .unwrap_or_default();
        let stray = meta.is_dir()
            && !known.iter().any(|w| w.name.eq_ignore_ascii_case(&name))
            && age >= why.grace;
        if !stray {
            continue;
        }
        let Some(reason) = verdict(&name, tasks, why.ended, why.settled) else {
            continue;
        };
        std::fs::remove_dir_all(entry.path())
            .map_err(|e| Error::io(format!("remove {}", entry.path().display()), e))?;
        tracing::info!(
            target: "townhall::task", event = "reaper.stray_removed", agent = %name,
            repo, why = reason.as_str(), "stray worktree folder removed"
        );
        swept.push(name);
    }
    Ok(swept)
}
