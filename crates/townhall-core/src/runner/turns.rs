//! Which conversations have a turn running now: at most one each. The routes begin a
//! turn here (a second one is a `409`), `stop` signals it, and the runner's handle
//! aborts every turn when the daemon stops.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::watch;
use tokio::task::AbortHandle;

use crate::{Error, Result};

pub const BUSY: &str = "a turn is running: wait for the reply or stop it";

#[derive(Debug)]
struct Slot {
    generation: u64,
    stop: watch::Sender<bool>,
    done: watch::Receiver<()>,
    task: Option<AbortHandle>,
}

type Slots = Arc<Mutex<HashMap<String, Slot>>>;

/// The running turns of one daemon (`Context::turns`).
#[derive(Debug, Clone, Default)]
pub struct Turns {
    slots: Slots,
    next: Arc<AtomicU64>,
}

/// Holds a conversation's slot while its turn runs; dropping it frees the slot.
#[derive(Debug)]
pub struct TurnGuard {
    slots: Slots,
    id: String,
    generation: u64,
    stop: watch::Receiver<bool>,
    _done: watch::Sender<()>,
}

fn lock(slots: &Slots) -> MutexGuard<'_, HashMap<String, Slot>> {
    slots.lock().unwrap_or_else(|p| p.into_inner())
}

impl Turns {
    /// Takes the conversation's slot, or `Conflict` when a turn runs there.
    pub fn begin(&self, id: &str) -> Result<TurnGuard> {
        let mut slots = lock(&self.slots);
        if slots.contains_key(id) {
            return Err(Error::Conflict(BUSY.into()));
        }
        let generation = self.next.fetch_add(1, Ordering::Relaxed);
        let (stop_tx, stop_rx) = watch::channel(false);
        let (done_tx, done_rx) = watch::channel(());
        let slot = Slot {
            generation,
            stop: stop_tx,
            done: done_rx,
            task: None,
        };
        slots.insert(id.to_string(), slot);
        Ok(TurnGuard {
            slots: Arc::clone(&self.slots),
            id: id.to_string(),
            generation,
            stop: stop_rx,
            _done: done_tx,
        })
    }

    pub fn is_running(&self, id: &str) -> bool {
        lock(&self.slots).contains_key(id)
    }

    /// Remembers the task of the turn `generation`, so `abort_all` can end it.
    pub fn attach(&self, id: &str, generation: u64, task: AbortHandle) {
        let mut slots = lock(&self.slots);
        match slots.get_mut(id) {
            Some(slot) if slot.generation == generation => slot.task = Some(task),
            _ => {}
        }
    }

    /// Asks the running turn to stop. The receiver's `changed()` fails once it ended.
    pub fn stop(&self, id: &str) -> Option<watch::Receiver<()>> {
        let slots = lock(&self.slots);
        let slot = slots.get(id)?;
        slot.stop.send_replace(true);
        Some(slot.done.clone())
    }

    /// Aborts every turn's task (its claude is killed on drop). The daemon is stopping.
    pub fn abort_all(&self) {
        let tasks: Vec<AbortHandle> = lock(&self.slots)
            .values()
            .filter_map(|s| s.task.clone())
            .collect();
        tasks.iter().for_each(AbortHandle::abort);
    }
}

impl TurnGuard {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// A receiver that changes to `true` when someone asked this turn to stop.
    pub fn stop_signal(&self) -> watch::Receiver<bool> {
        self.stop.clone()
    }

    pub fn stop_requested(&self) -> bool {
        *self.stop.borrow()
    }
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        let mut slots = lock(&self.slots);
        if slots.get(&self.id).map(|s| s.generation) == Some(self.generation) {
            slots.remove(&self.id);
        }
    }
}

/// Aborts every running turn when dropped: the runner's background task holds it.
#[derive(Debug)]
pub(super) struct AbortOnDrop(pub(super) Turns);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_turn_per_conversation() {
        let turns = Turns::default();
        let a = turns.begin("a").unwrap();
        assert!(matches!(turns.begin("a"), Err(Error::Conflict(m)) if m == BUSY));
        let b = turns.begin("b").unwrap();
        assert_ne!(a.generation(), b.generation());
        assert!(turns.is_running("a"));
        drop(a);
        assert!(!turns.is_running("a"));
        assert!(turns.begin("a").is_ok());
        drop(b);
    }

    #[tokio::test]
    async fn stop_signals_and_reports_the_end() {
        let turns = Turns::default();
        assert!(turns.stop("a").is_none());
        let guard = turns.begin("a").unwrap();
        let mut signal = guard.stop_signal();
        assert!(!guard.stop_requested());
        let mut done = turns.stop("a").unwrap();
        signal.changed().await.unwrap();
        assert!(*signal.borrow() && guard.stop_requested());
        drop(guard);
        assert!(done.changed().await.is_err());
    }

    #[tokio::test]
    async fn abort_all_ends_attached_tasks() {
        let turns = Turns::default();
        let guard = turns.begin("a").unwrap();
        let generation = guard.generation();
        let task = tokio::spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });
        turns.attach("a", generation, task.abort_handle());
        turns.attach("a", generation + 1, task.abort_handle());
        drop(AbortOnDrop(turns.clone()));
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(!turns.is_running("a"));
    }
}
