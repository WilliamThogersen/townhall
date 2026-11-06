//! The reaper: townhall cleans up after itself. One pass, over every registered repo:
//!
//! - cancels the queued tasks of ended conversations, so they never start;
//! - removes every agent (tmux session and worktree) whose conversation ended, whose task
//!   was merged, or that is no task's at all (`verdict.rs`);
//! - kills townhall sessions whose worktree is gone and prunes git's worktree records.
//!
//! - trims each repo's shared cargo target folder to `sharedTargetMaxGb`, never while an
//!   agent of the repo runs (`target.rs`).
//!
//! A branch is deleted only when git calls it merged, so commits that exist nowhere else
//! keep theirs. Live work, a task of an open conversation that is not merged, is never
//! touched. It passes at startup, every `EVERY`, and right after a conversation ends
//! (`soon`). Passes never overlap.

mod cancel;
mod repo;
mod target;
mod verdict;

use std::collections::HashSet;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::redact::redact;
use crate::{Context, Result, Workspace};

use target::Trim;

pub use cancel::{cancel_queued, CANCELLED};
pub use verdict::{verdict, Why};

pub const EVERY: Duration = Duration::from_secs(300);

/// One pass at a time: two passes would remove the same worktree at once.
static PASS: Mutex<()> = Mutex::const_new(());

/// What one pass did, by agent or task name, plus what it could not do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reaped {
    pub cancelled: Vec<String>,
    pub removed: Vec<String>,
    pub sessions: Vec<String>,
    /// Repos whose shared cargo target folder was trimmed.
    pub trimmed: Vec<String>,
    pub warnings: Vec<String>,
}

impl Reaped {
    fn and(self, other: Reaped) -> Reaped {
        Reaped {
            cancelled: [self.cancelled, other.cancelled].concat(),
            removed: [self.removed, other.removed].concat(),
            sessions: [self.sessions, other.sessions].concat(),
            trimmed: [self.trimmed, other.trimmed].concat(),
            warnings: [self.warnings, other.warnings].concat(),
        }
    }
}

/// One pass. Every repo is surveyed before the tasks are read: an agent is only ever
/// started for a task that exists, so a fresh agent is never taken for an orphan.
pub async fn run_once(ctx: &Context) -> Result<Reaped> {
    let _pass = PASS.lock().await;
    let mut surveyed = vec![];
    let mut reaped = Reaped::default();
    for repo in ctx.repos().await? {
        // A repo whose folder is gone has no agents to remove.
        let Ok(ws) = ctx.workspace(&repo) else {
            continue;
        };
        match repo::survey(&ws).await {
            Ok(found) => surveyed.push((repo.name, ws, found)),
            Err(e) => reaped
                .warnings
                .push(format!("repo `{}` not surveyed: {e}", repo.name)),
        }
    }
    let store = ctx.store()?;
    let ended: HashSet<String> = store
        .conversations()
        .await?
        .into_iter()
        .filter(|c| c.ended)
        .map(|c| c.id)
        .collect();
    let tasks = store.all_tasks().await?;
    let of_ended: Vec<_> = tasks
        .iter()
        .filter(|t| ended.contains(&t.conversation_id))
        .cloned()
        .collect();
    reaped.cancelled = cancel_queued(store, &of_ended).await?;
    let max_gb = ctx.settings().await?.shared_target_max_gb;
    for (name, ws, found) in surveyed {
        let running: Vec<String> = found
            .agents
            .iter()
            .filter(|a| a.alive)
            .map(|a| a.name.clone())
            .collect();
        let done = repo::reap(&ws, &name, found, &tasks, &ended).await;
        let busy = running.iter().any(|a| !done.removed.contains(a));
        let trimmed = trim_target(&ws, &name, max_gb, busy).await;
        reaped = reaped.and(done).and(trimmed);
    }
    Ok(reaped)
}

/// One repo's shared target folder, trimmed to the cap. A failure is a warning.
async fn trim_target(ws: &Workspace, repo: &str, max_gb: u32, busy: bool) -> Reaped {
    let mut reaped = Reaped::default();
    match target::trim(ws, max_gb, busy).await {
        Ok(Trim::Within) => {}
        Ok(Trim::Trimmed) => {
            tracing::info!(
                target: "townhall::task", event = "reaper.target_trimmed", repo,
                max_gb, "shared cargo target trimmed"
            );
            reaped.trimmed.push(repo.to_string());
        }
        Ok(Trim::Busy(size)) => reaped.warnings.push(format!(
            "shared target of repo `{repo}` is {} over its cap of {max_gb} GB, kept while its agents run",
            target::gb(size)
        )),
        Err(e) => reaped
            .warnings
            .push(format!("shared target of repo `{repo}` not trimmed: {e}")),
    }
    reaped
}

fn report(when: &'static str, result: Result<Reaped>) {
    match result {
        Ok(done) => {
            if !done.warnings.is_empty() {
                tracing::warn!(
                    target: "townhall::task", event = "reaper.warning", when,
                    error = %redact(&done.warnings.join("; ")), "the reaper left something behind"
                );
            }
            if done != Reaped::default() {
                tracing::info!(
                    target: "townhall::task", event = "reaper.pass", when,
                    removed = done.removed.len(), sessions = done.sessions.len(),
                    cancelled = done.cancelled.len(), "the reaper cleaned up"
                );
            }
        }
        Err(e) => tracing::warn!(
            target: "townhall::task", event = "reaper.failed", when, error_kind = e.kind(),
            error = %redact(&e.to_string()), "the reaper pass failed"
        ),
    }
}

/// A pass in the background, off the request that asked for it.
pub fn soon(ctx: &Context) {
    let ctx = ctx.clone();
    tokio::spawn(async move { report("soon", run_once(&ctx).await) });
}

/// Passes at startup, then every `EVERY`. `None` when there is no store.
pub fn start(ctx: &Context) -> Option<JoinHandle<()>> {
    if let Err(e) = ctx.store() {
        tracing::warn!(
            target: "townhall::task", event = "reaper.off", error_kind = e.kind(),
            error = %redact(&e.to_string()), "the reaper is off"
        );
        return None;
    }
    let ctx = ctx.clone();
    Some(tokio::spawn(async move {
        let mut tick = tokio::time::interval(EVERY);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            report("tick", run_once(&ctx).await);
        }
    }))
}
