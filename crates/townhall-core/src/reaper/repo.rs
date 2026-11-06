//! One repo's part of a pass: what is there, then what goes.

use std::collections::HashSet;

use crate::agent::{self, Agent};
use crate::cmd::run;
use crate::model::{Task, TaskState};
use crate::{tmux, Result, Workspace};

use super::purge::purge;
use super::verdict::verdict;
use super::Reaped;

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
    ended: &HashSet<String>,
) -> Reaped {
    let mut reaped = Reaped::default();
    for a in found.agents {
        let Some(why) = verdict(&a.name, tasks, ended) else {
            if !a.alive && finished(&a.name, tasks) {
                purge_artifacts(&a, repo, &mut reaped).await;
            }
            continue;
        };
        match agent::retire(ws, &a.name).await {
            Ok(true) => {
                tracing::info!(
                    target: "townhall::task", event = "reaper.agent_removed", agent = %a.name,
                    repo, why = why.as_str(), "agent removed"
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
    reaped
}

fn finished(name: &str, tasks: &[Task]) -> bool {
    tasks.iter().any(|t| {
        t.name.eq_ignore_ascii_case(name)
            && matches!(
                t.state,
                TaskState::Ended | TaskState::InReview | TaskState::InCi
            )
    })
}

async fn purge_artifacts(agent: &Agent, repo: &str, reaped: &mut Reaped) {
    match purge(&agent.worktree).await {
        Ok(0) => {}
        Ok(bytes) => {
            tracing::info!(
                target: "townhall::task", event = "reaper.artifacts_purged", agent = %agent.name,
                repo, bytes, "build artifacts deleted"
            );
            reaped.purged.push(agent.name.clone());
        }
        Err(e) => reaped.warnings.push(format!(
            "artifacts of agent `{}` of repo `{repo}` not deleted: {e}",
            agent.name
        )),
    }
}
