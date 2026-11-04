//! The in-process event bus. Every store write publishes here; `/api/events` sockets subscribe.

use tokio::sync::broadcast;

use crate::model::ServerEvent;

/// A subscriber further behind than this gets `Lagged` and should send `resync`.
pub const CAPACITY: usize = 1024;

#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<ServerEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CAPACITY);
        Self { tx }
    }

    /// Delivers to every current subscriber and returns how many there were.
    /// With none the event is not needed by anyone, which is not an error.
    pub fn publish(&self, event: ServerEvent) -> usize {
        self.tx.send(event).unwrap_or(0)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_reaches_subscribers_only() {
        let bus = EventBus::new();
        assert_eq!(bus.publish(ServerEvent::Resync), 0);
        let mut rx = bus.subscribe();
        assert_eq!(bus.clone().publish(ServerEvent::Resync), 1);
        assert_eq!(rx.recv().await.unwrap(), ServerEvent::Resync);
    }
}
