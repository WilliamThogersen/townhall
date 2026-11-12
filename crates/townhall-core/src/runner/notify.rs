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

use tokio::task::JoinHandle;

use super::actions::open_turn;
use crate::context::WeakContext;
use crate::model::{EntryBody, NewMessage};
use crate::redact::redact;
use crate::{Context, Result};

/// What the planner reads before the events, so it knows nobody typed them.
pub const EVENT_INTRO: &str = "townhall event (automatic, the user did not write this):";

#[derive(Debug)]
struct Pending {
    seq: i64,
    text: String,
    saved: Option<JoinHandle<()>>,
}

type Waiting = HashMap<String, Vec<Pending>>;

/// The events waiting per conversation. A conversation with an entry has a flusher task.
#[derive(Debug, Clone, Default)]
pub struct Notes {
    waiting: Arc<Mutex<Waiting>>,
    last_seq: Arc<Mutex<i64>>,
}

fn lock<T>(map: &Mutex<T>) -> MutexGuard<'_, T> {
    map.lock().unwrap_or_else(|p| p.into_inner())
}

impl Notes {
    fn next_seq(&self) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX));
        let mut last = lock(&self.last_seq);
        *last = now.max(last.saturating_add(1));
        *last
    }

    fn push(&self, id: &str, pending: Pending) -> bool {
        let mut waiting = lock(&self.waiting);
        let first = !waiting.contains_key(id);
        waiting.entry(id.to_string()).or_default().push(pending);
        first
    }

    fn holds(&self, id: &str, seq: i64) -> bool {
        lock(&self.waiting)
            .get(id)
            .is_some_and(|all| all.iter().any(|p| p.seq == seq))
    }

    /// Takes everything queued and retires the flusher.
    fn take(&self, id: &str) -> Vec<Pending> {
        lock(&self.waiting).remove(id).unwrap_or_default()
    }

    /// What waits for the conversation's planner now, oldest first; nothing is taken.
    pub fn waiting(&self, id: &str) -> Vec<String> {
        let waiting = lock(&self.waiting);
        let all = waiting.get(id).map(|all| all.iter());
        all.into_iter().flatten().map(|p| p.text.clone()).collect()
    }

    pub fn is_waiting(&self, id: &str) -> bool {
        lock(&self.waiting).contains_key(id)
    }
}

pub fn notify(ctx: &Context, id: &str, text: String) {
    let seq = ctx.notes().next_seq();
    let saved = save(ctx.clone(), id.to_string(), seq, text.clone());
    let pending = Pending {
        seq,
        text,
        saved: Some(saved),
    };
    if ctx.notes().push(id, pending) {
        tokio::spawn(flush(ctx.downgrade(), ctx.notify_delay(), id.to_string()));
    }
}

fn save(ctx: Context, id: String, seq: i64, text: String) -> JoinHandle<()> {
    tokio::spawn(async move {
        let saved = match ctx.store() {
            Ok(store) => store.add_pending_event(&id, seq, &text).await,
            Err(e) => Err(e),
        };
        if let Err(e) = saved {
            tracing::warn!(
                target: "townhall::runner", event = "notify.not_saved", conversation = %id,
                error_kind = e.kind(), error = %redact(&e.to_string()),
                "task event not saved, a restart would lose it"
            );
        }
    })
}

pub async fn redeliver(ctx: &Context) -> Result<usize> {
    let mut queued = 0;
    for saved in ctx.store()?.pending_events().await? {
        if ctx.notes().holds(&saved.conversation_id, saved.seq) {
            continue;
        }
        let id = saved.conversation_id;
        let pending = Pending {
            seq: saved.seq,
            text: saved.text,
            saved: None,
        };
        if ctx.notes().push(&id, pending) {
            tokio::spawn(flush(ctx.downgrade(), ctx.notify_delay(), id));
        }
        queued += 1;
    }
    Ok(queued)
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
    let pending = ctx.notes().take(&id);
    let texts = pending.iter().map(|p| p.text.clone()).collect();
    match deliver(&ctx, &id, texts, guard).await {
        Ok(()) => forget(&ctx, &id, pending).await,
        Err(e) => tracing::warn!(
            target: "townhall::runner", event = "notify.not_delivered", conversation = %id,
            error_kind = e.kind(), error = %redact(&e.to_string()),
            "task events not delivered"
        ),
    }
}

async fn forget(ctx: &Context, id: &str, pending: Vec<Pending>) {
    let mut seqs = Vec::with_capacity(pending.len());
    for p in pending {
        if let Some(saved) = p.saved {
            if let Err(e) = saved.await {
                tracing::warn!(
                    target: "townhall::runner", event = "notify.save_aborted", conversation = %id,
                    error = %e, "saving a task event did not finish"
                );
            }
        }
        seqs.push(p.seq);
    }
    let removed = match ctx.store() {
        Ok(store) => store.remove_pending_events(id, &seqs).await,
        Err(e) => Err(e),
    };
    if let Err(e) = removed {
        tracing::warn!(
            target: "townhall::runner", event = "notify.not_forgotten", conversation = %id,
            error_kind = e.kind(), error = %redact(&e.to_string()),
            "delivered task events stay saved"
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

    fn pending(seq: i64, text: &str) -> Pending {
        Pending {
            seq,
            text: text.into(),
            saved: None,
        }
    }

    #[test]
    fn the_first_event_of_a_conversation_starts_the_flusher() {
        let notes = Notes::default();
        assert!(notes.push("a", pending(1, "one")));
        assert!(!notes.push("a", pending(2, "two")));
        assert!(notes.push("b", pending(3, "other")));
        assert!(notes.is_waiting("a"));
        assert!(notes.holds("a", 2));
        assert!(!notes.holds("a", 3));
        assert_eq!(notes.waiting("a"), vec!["one", "two"]);
        let taken: Vec<String> = notes.take("a").into_iter().map(|p| p.text).collect();
        assert_eq!(taken, vec!["one", "two"]);
        assert!(!notes.is_waiting("a"));
        assert!(notes.push("a", pending(4, "three")));
    }

    #[test]
    fn sequence_numbers_only_grow() {
        let notes = Notes::default();
        let seqs: Vec<i64> = (0..50).map(|_| notes.next_seq()).collect();
        assert!(seqs.windows(2).all(|w| w[0] < w[1]));
    }
}
