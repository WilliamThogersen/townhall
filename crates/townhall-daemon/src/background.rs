//! The one hook that starts every long-running unit. The daemon's `main.rs` and the desktop
//! app's in-process daemon both call `start_background`; each unit fills in its own `start`.

use tokio::task::JoinHandle;
use townhall_core::{agent, reaper, runner, scheduler};

use crate::logging;
use crate::state::AppState;

/// The tasks the units started. `stop` aborts them (the desktop app restarts the daemon
/// when the repo changes). Agents live in tmux and are never touched.
#[derive(Debug, Default)]
pub struct Background {
    handles: Vec<JoinHandle<()>>,
}

impl Background {
    pub fn running(&self) -> usize {
        self.handles.iter().filter(|h| !h.is_finished()).count()
    }

    pub fn stop(self) {
        self.handles.iter().for_each(JoinHandle::abort);
    }
}

/// Registers the startup repo, then starts the conversation runner (2.2), the reaper, the
/// task scheduler (2.3) and the agent watcher (2.5).
pub async fn start_background(state: &AppState) -> Background {
    let ctx = state.ctx();
    ctx.ensure_startup_repo().await;
    let handles = [
        runner::start(ctx),
        reaper::start(ctx),
        scheduler::start(ctx),
        agent::watch::start(ctx),
        Some(start_log_pruning(ctx.home().clone())),
    ]
    .into_iter()
    .flatten()
    .collect();
    Background { handles }
}

const PRUNE_EVERY: std::time::Duration = std::time::Duration::from_secs(3600);

/// Keeps the logs folder under its size and age limits while the process runs (`init` prunes at startup).
fn start_log_pruning(home: townhall_core::Home) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(PRUNE_EVERY);
        tick.tick().await; // the first tick is immediate and `init` just pruned
        loop {
            tick.tick().await;
            let dir = home.root().join("logs");
            let pruned = tokio::task::spawn_blocking(move || {
                logging::prune(
                    &dir,
                    logging::MAX_LOG_BYTES,
                    logging::MAX_LOG_AGE_DAYS,
                    std::time::SystemTime::now(),
                )
            })
            .await;
            match pruned {
                Ok(Ok(deleted)) if deleted > 0 => {
                    tracing::info!(target: "townhall::logging", event = "logs.pruned", deleted, "pruned old log files");
                }
                Ok(Err(e)) => {
                    tracing::warn!(target: "townhall::logging", event = "logs.prune_failed", error = %townhall_core::redact::redact(&e.to_string()), "cannot prune logs");
                }
                _ => {}
            }
        }
    })
}
