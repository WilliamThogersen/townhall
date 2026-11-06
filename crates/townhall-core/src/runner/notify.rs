//! Task events that wake townhall's own session. The agent watcher calls `notify` when a
//! task of a conversation ends, fails or asks something. Events of one conversation
//! gather for `Context::notify_delay` (so a burst makes one turn), wait while a turn
//! runs, then become `event` entries and one planner turn that answers them in the thread.
//!
//! Nothing here reacts to a planner reply, so a reply never wakes the planner again: only
//! a new task change does. Events are kept in memory: a restart drops the ones not yet
//! delivered (their `agent` and `needsYou` entries are in the thread all the same). An
//! ended conversation is not woken.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use super::actions::open_turn;
use crate::context::WeakContext;
use crate::model::{EntryBody, NewMessage};
use crate::redact::redact;
use crate::{Context, Result};

/// What the planner reads before the events, so it knows nobody typed them.
pub const EVENT_INTRO: &str = "townhall event (automatic, the user did not write this):";

/// The events waiting per conversation. A conversation with an entry has a flusher task.
#[derive(Debug, Clone, Default)]
pub struct Notes {
    waiting: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

fn lock(map: &Mutex<HashMap<String, Vec<String>>>) -> MutexGuard<'_, HashMap<String, Vec<String>>> {
    map.lock().unwrap_or_else(|p| p.into_inner())
}

impl Notes {
    /// Queues `text`; true when no flusher was waiting yet, so the caller starts one.
    fn push(&self, id: &str, text: String) -> bool {
        let mut waiting = lock(&self.waiting);
        let first = !waiting.contains_key(id);
        waiting.entry(id.to_string()).or_default().push(text);
        first
    }

    /// Takes everything queued and retires the flusher.
    fn take(&self, id: &str) -> Vec<String> {
        lock(&self.waiting).remove(id).unwrap_or_default()
    }

    /// What waits for the conversation's planner now, oldest first; nothing is taken.
    pub fn waiting(&self, id: &str) -> Vec<String> {
        lock(&self.waiting).get(id).cloned().unwrap_or_default()
    }

    pub fn is_waiting(&self, id: &str) -> bool {
        lock(&self.waiting).contains_key(id)
    }
}

/// Queues a task event for the conversation's planner.
pub fn notify(ctx: &Context, id: &str, text: String) {
    if ctx.notes().push(id, text) {
        tokio::spawn(flush(ctx.downgrade(), ctx.notify_delay(), id.to_string()));
    }
}

/// Waits out the delay, then for a free turn slot. Holds the context only between waits,
/// so a daemon that stopped meanwhile is not kept alive.
async fn flush(weak: WeakContext, delay: Duration, id: String) {
    let (ctx, guard) = loop {
        tokio::time::sleep(delay).await;
        let Some(ctx) = weak.upgrade() else {
            return;
        };
        if let Ok(guard) = ctx.turns().begin(&id) {
            break (ctx, guard);
        }
    };
    let texts = ctx.notes().take(&id);
    if let Err(e) = deliver(&ctx, &id, texts, guard).await {
        tracing::warn!(
            target: "townhall::runner", event = "notify.not_delivered", conversation = %id,
            error_kind = e.kind(), error = %redact(&e.to_string()),
            "task events not delivered"
        );
    }
}

async fn deliver(
    ctx: &Context,
    id: &str,
    texts: Vec<String>,
    guard: super::TurnGuard,
) -> Result<()> {
    if texts.is_empty() || ctx.store()?.conversation(id).await?.ended {
        return Ok(());
    }
    let said = format!("{EVENT_INTRO}\n{}", texts.join("\n"));
    let message = NewMessage {
        text: said,
        attachments: vec![],
        reply_to: None,
    };
    let entries = texts
        .into_iter()
        .map(|text| EntryBody::Event { text })
        .collect();
    open_turn(ctx, id, guard, entries, message, true).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_event_of_a_conversation_starts_the_flusher() {
        let notes = Notes::default();
        assert!(notes.push("a", "one".into()));
        assert!(!notes.push("a", "two".into()));
        assert!(notes.push("b", "other".into()));
        assert!(notes.is_waiting("a"));
        assert_eq!(notes.take("a"), vec!["one", "two"]);
        assert!(!notes.is_waiting("a"));
        assert!(notes.push("a", "three".into()));
    }
}
