//! The task scheduler (unit 2.3): turns queued tasks into agents, each in its own worktree
//! of its repo, and tells when a finished task's branch was merged.
//!
//! - A queued task starts once every `after` task finished (`ended` or `merged`) and fewer
//!   than `agentsAtOnce` tasks are `working` or `needsYou` with a live session. Queued tasks
//!   start oldest first. The agent reads its whole prompt from its brief (`brief.rs`) and
//!   runs as the task's agent type (`agent_type.rs`), whose mode only narrows the setting.
//! - A start that fails sets `error` with an `agent` `failed` entry: the only `error` it
//!   sets. The agent watcher (2.5) owns `needsYou`, `error` and `ended`.
//! - An `ended` task whose branch has commits of its own and is merged into its base
//!   branch becomes `merged`, with an `agent` `passed` entry. It never merges or deletes.
//! - Safe across restarts: it never starts an agent for a name that has a worktree or a
//!   session. A queued task whose agent exists (a start cut short) is adopted as `working`.
//!
//! It runs a pass on every `taskUpdated` and on a slow tick (`TICK`).

mod agent_type;
mod brief;
mod gate;
mod launch;
mod merged;
mod plan;
pub mod reflect;
pub mod report;
pub mod requires;

use std::time::Duration;

use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::context::Context;
use crate::model::{ServerEvent, Task, TaskState};
use crate::store::Store;
use crate::{Error, Result};

pub use brief::brief_file;
pub use launch::start_failure;

/// The slow tick: merges and freed slots are noticed at least this often.
pub const TICK: Duration = Duration::from_secs(2);

/// What a task's agent was started from (`task_launches`): the commit its branch forked
/// from and the Claude session id (`None` for an agent adopted after a restart).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskLaunch {
    pub name: String,
    pub start_commit: String,
    pub session_id: Option<String>,
    pub launched_at: String,
}

/// What one pass did, by task name, plus what it could not do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pass {
    pub started: Vec<String>,
    pub adopted: Vec<String>,
    pub failed: Vec<String>,
    /// Queued tasks held back by their agent type's `requires` (`gate.rs`).
    pub blocked: Vec<String>,
    pub merged: Vec<String>,
    /// Ended tasks handed on to the `reflector` type (`reflect.rs`).
    pub reflected: Vec<String>,
    pub warnings: Vec<String>,
}

/// Starts the scheduler loop. `None` when there is no store to read tasks from.
pub fn start(ctx: &Context) -> Option<JoinHandle<()>> {
    if let Err(e) = ctx.store() {
        tracing::warn!(
            target: "townhall::task", event = "scheduler.off", error_kind = e.kind(),
            error = %crate::redact::redact(&e.to_string()), "the task scheduler is off"
        );
        return None;
    }
    Some(tokio::spawn(run(ctx.clone())))
}

async fn run(ctx: Context) {
    let mut events = ctx.bus().subscribe();
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut reported = String::new();
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            woke = events.recv() => match woke {
                Ok(ServerEvent::TaskUpdated { .. }) | Err(RecvError::Lagged(_)) => drain(&mut events),
                Ok(_) => continue,
                Err(RecvError::Closed) => return,
            },
        }
        reported = report(run_once(&ctx).await, reported);
    }
}

/// Events that came in meanwhile need no pass of their own: the next pass reads them all.
fn drain(events: &mut Receiver<ServerEvent>) {
    while matches!(events.try_recv(), Ok(_) | Err(TryRecvError::Lagged(_))) {}
}

/// Prints what a pass could not do, once until it changes, so the slow tick does not repeat it.
fn report(result: Result<Pass>, last: String) -> String {
    let text = match result {
        Ok(pass) => pass.warnings.join("; "),
        Err(e) => e.to_string(),
    };
    if !text.is_empty() && text != last {
        tracing::warn!(
            target: "townhall::task", event = "scheduler.pass_warning",
            error = %crate::redact::redact(&text), "task scheduler could not do something"
        );
    }
    text
}

/// One pass: start what may start, then tell merges. Tests call it directly.
pub async fn run_once(ctx: &Context) -> Result<Pass> {
    let mut tasks = ctx.store()?.all_tasks().await?;
    let (reflected, reflect_warnings) = reflect::queue(ctx, &tasks).await?;
    if !reflected.is_empty() {
        tasks = ctx.store()?.all_tasks().await?;
    }
    let pass = launch::start_ready(ctx, &tasks).await?;
    let (merged, warnings) = merged::detect(ctx, &tasks).await?;
    Ok(Pass {
        merged,
        reflected,
        warnings: [pass.warnings.clone(), reflect_warnings, warnings].concat(),
        ..pass
    })
}

const MOVED: &str = "the task's state moved on";

/// `from` to `to` in one write; `None` when the task is no longer in `from`.
async fn transition(
    store: &Store,
    name: &str,
    from: TaskState,
    to: TaskState,
) -> Result<Option<Task>> {
    let change = move |t: Task| match t.state == from {
        true => Ok(Task { state: to, ..t }),
        false => Err(Error::Conflict(MOVED.into())),
    };
    match store.update_task(name, change).await {
        Ok(task) => Ok(Some(task)),
        Err(Error::Conflict(m)) if m == MOVED => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_is_the_text_to_compare_the_next_one_with() {
        let warned = Pass {
            warnings: vec!["a".into(), "b".into()],
            ..Pass::default()
        };
        assert_eq!(report(Ok(warned.clone()), String::new()), "a; b");
        assert_eq!(report(Ok(warned), "a; b".into()), "a; b");
        assert_eq!(report(Ok(Pass::default()), "a; b".into()), "");
        assert_eq!(report(Err(Error::Conflict("x".into())), String::new()), "x");
    }
}
