//! The conversation runner (unit 2.2): townhall's own Claude session per conversation,
//! `claude -p` with stream-json, `--session-id` on the first turn and `--resume` after,
//! and `--mcp-config` from `mcp::config_json`. Not built yet, so it starts nothing.

use tokio::task::JoinHandle;

use crate::context::Context;

pub fn start(_ctx: &Context) -> Option<JoinHandle<()>> {
    None
}
