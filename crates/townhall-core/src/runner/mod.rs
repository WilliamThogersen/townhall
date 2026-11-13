//! The conversation runner (unit 2.2): townhall's own Claude session per conversation.
//! Each message is one turn: repo inference first (`repo.rs`), then `claude -p` with
//! stream-json in and out, `--session-id` on the first turn and `--resume` after, and
//! townhall's MCP tools from `mcp::config_json`. The session plans and creates tasks; it
//! never writes to a repo (`args.rs`, and every other tool request is denied).
//!
//! Task events (`notify.rs`) wake the session on their own: they gather for a moment, wait
//! for a running turn and then start one more turn, recorded as an `event` entry.
//!
//! At most one turn per conversation runs (`Turns`, held by the `Context`). `start` fixes
//! conversations a restart left `thinking`; its handle aborts every running turn when
//! `Background::stop` aborts it.

mod actions;
mod advice;
mod answer;
mod args;
mod content;
mod notify;
mod persona;
mod process;
mod prompt;
mod quote;
mod repo;
mod resume;
mod session;
mod stream;
mod turn;
mod turns;

use tokio::task::JoinHandle;

pub use actions::{
    create, create_with_files, end, patch, send, send_as_plugin, stop, StartFile, MAX_ATTACHMENTS,
    MAX_MESSAGE_CHARS,
};
pub use advice::{consult, Advice};
pub use args::{allowed_tools, DENIED_TOOLS, ISOLATION, READ_ONLY_TOOLS};
pub use content::user_content;
pub use notify::{notify, redeliver, Notes};
pub use process::{EXIT_GRACE, KILL_WAIT, STOP_GRACE};
pub use resume::{RESUMED_MARKER, RESUME_NOTE};
pub use session::DEFAULT_CONTEXT_LIMIT;
pub use turn::LOST_SESSION;
pub use turns::{TurnGuard, Turns, BUSY};

use crate::model::{ConversationStatus, EntryBody};
use crate::redact::redact;
use crate::{Context, Result};
use resume::Plan;

pub const RESTARTED: &str = "townhall restarted before this reply finished. send it again";

/// Fixes every conversation a restart left `thinking`, then holds on until aborted;
/// aborting it aborts every running turn (their claude is killed).
pub fn start(ctx: &Context) -> Option<JoinHandle<()>> {
    let ctx = ctx.clone();
    Some(tokio::spawn(async move {
        let _turns = turns::AbortOnDrop(ctx.turns().clone());
        if let Err(e) = recover(&ctx).await {
            tracing::warn!(
                target: "townhall::runner", event = "recover.failed",
                error_kind = e.kind(), error = %redact(&e.to_string()),
                "conversations left thinking were not fixed"
            );
        }
        if let Err(e) = redeliver(&ctx).await {
            tracing::warn!(
                target: "townhall::runner", event = "redeliver.failed",
                error_kind = e.kind(), error = %redact(&e.to_string()),
                "saved task events were not queued"
            );
        }
        std::future::pending::<()>().await
    }))
}

pub async fn recover(ctx: &Context) -> Result<usize> {
    let store = ctx.store()?;
    let mut fixed = 0;
    for c in store.conversations().await? {
        if c.status != ConversationStatus::Thinking || ctx.turns().is_running(&c.id) {
            continue;
        }
        let entries = store.entries(&c.id).await?;
        let why = match resume::plan(&c, &entries) {
            Plan::Resume(entry) if store.claim_resume(&c.id, entry).await? => {
                if let Ok(guard) = ctx.turns().begin(&c.id) {
                    let marker = EntryBody::Event {
                        text: RESUMED_MARKER.into(),
                    };
                    store.append_entry(&c.id, marker).await?;
                    actions::resume_turn(ctx, &c.id, guard);
                    fixed += 1;
                }
                continue;
            }
            Plan::Resume(_) => "this message was already resumed once",
            Plan::Restarted(reason) => reason,
        };
        if !actions::unstick(ctx, &c.id).await? {
            continue;
        }
        tracing::warn!(
            target: "townhall::runner", event = "resume.skipped", conversation = %c.id,
            reason = why, "interrupted reply not resumed"
        );
        let note = EntryBody::Error {
            message: RESTARTED.into(),
        };
        store.append_entry(&c.id, note).await?;
        fixed += 1;
    }
    Ok(fixed)
}
