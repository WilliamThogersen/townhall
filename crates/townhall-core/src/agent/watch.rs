//! The agent watcher (unit 2.5): polls every agent, publishes `agentUpdated` when its state
//! changes and appends `needsYou` entries. Not built yet, so it starts nothing.

use tokio::task::JoinHandle;

use crate::context::Context;

pub fn start(_ctx: &Context) -> Option<JoinHandle<()>> {
    None
}
