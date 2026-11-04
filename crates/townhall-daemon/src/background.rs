//! The one hook that starts every long-running unit. The daemon's `main.rs` and the desktop
//! app's in-process daemon both call `start_background`; each unit fills in its own `start`.

use tokio::task::JoinHandle;
use townhall_core::{agent, runner, scheduler};

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

/// Registers the startup repo, then starts the conversation runner (2.2), the task
/// scheduler (2.3) and the agent watcher (2.5).
pub async fn start_background(state: &AppState) -> Background {
    let ctx = state.ctx();
    ctx.ensure_startup_repo().await;
    let handles = [
        runner::start(ctx),
        scheduler::start(ctx),
        agent::watch::start(ctx),
    ]
    .into_iter()
    .flatten()
    .collect();
    Background { handles }
}
