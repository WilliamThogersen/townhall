//! The conversation runner (unit 2.2): townhall's own Claude session per conversation.
//! Each message is one turn: repo inference first (`repo.rs`), then `claude -p` with
//! stream-json in and out, `--session-id` on the first turn and `--resume` after, and
//! townhall's MCP tools from `mcp::config_json`. The session plans and creates tasks; it
//! never writes to a repo (`args.rs`, and every other tool request is denied).
//!
//! At most one turn per conversation runs (`Turns`, held by the `Context`). `start` fixes
//! conversations a restart left `thinking`; its handle aborts every running turn when
//! `Background::stop` aborts it.

mod actions;
mod answer;
mod args;
mod content;
mod process;
mod prompt;
mod repo;
mod stream;
mod turn;
mod turns;

use tokio::task::JoinHandle;

pub use actions::{create, end, patch, send, stop, MAX_ATTACHMENTS, MAX_MESSAGE_CHARS};
pub use args::{allowed_tools, DENIED_TOOLS, READ_ONLY_TOOLS};
pub use content::user_content;
pub use process::{EXIT_GRACE, KILL_WAIT, STOP_GRACE};
pub use turn::DEFAULT_CONTEXT_LIMIT;
pub use turns::{TurnGuard, Turns, BUSY};

use crate::model::{ConversationStatus, EntryBody};
use crate::{Context, Result};

pub const RESTARTED: &str = "townhall restarted before this reply finished. send it again";

/// Fixes every conversation a restart left `thinking`, then holds on until aborted;
/// aborting it aborts every running turn (their claude is killed).
pub fn start(ctx: &Context) -> Option<JoinHandle<()>> {
    let ctx = ctx.clone();
    Some(tokio::spawn(async move {
        let _turns = turns::AbortOnDrop(ctx.turns().clone());
        if let Err(e) = recover(&ctx).await {
            eprintln!("warn: conversations left thinking were not fixed: {e}");
        }
        std::future::pending::<()>().await
    }))
}

/// Sets `yourTurn` on each conversation left `thinking` with no turn running, with an
/// `error` entry that says so. Returns how many it fixed.
pub async fn recover(ctx: &Context) -> Result<usize> {
    let store = ctx.store()?;
    let mut fixed = 0;
    for c in store.conversations().await? {
        if c.status != ConversationStatus::Thinking || !actions::unstick(ctx, &c.id).await? {
            continue;
        }
        let note = EntryBody::Error {
            message: RESTARTED.into(),
        };
        store.append_entry(&c.id, note).await?;
        fixed += 1;
    }
    Ok(fixed)
}
