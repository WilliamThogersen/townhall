//! The task scheduler (unit 2.3): turns queued tasks into agents in a worktree of their
//! repo's base branch, once their `after` tasks finished and a slot is free
//! (`Settings::agents_at_once`). Not built yet, so it starts nothing.

use tokio::task::JoinHandle;

use crate::context::Context;

pub fn start(_ctx: &Context) -> Option<JoinHandle<()>> {
    None
}
