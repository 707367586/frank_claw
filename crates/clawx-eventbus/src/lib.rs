//! Publish-subscribe event bus for ClawX.
//!
//! Provides an async, typed event bus that decouples producers and
//! consumers across the agent runtime.

use async_trait::async_trait;
use clawx_types::event::{Event, EventFilter, EventKind};
use tokio::sync::broadcast;
use tracing::debug;

/// Trait for event subscribers.
#[async_trait]
pub trait Subscriber: Send + Sync {
    /// Handle an incoming event.
    async fn on_event(&self, event: &Event);
}

/// Trait defining the event bus interface.
#[async_trait]
pub trait EventBusPort: Send + Sync {
    /// Publish an event to all subscribers.
    async fn publish(&self, event: Event);

    /// Subscribe to events matching the given filter.
    /// Returns a receiver that will receive matching events.
    fn subscribe(&self, filter: EventFilter) -> broadcast::Receiver<Event>;
}

/// A no-op event bus that silently discards all events.
/// Used as a default when no real event bus is needed.
#[derive(Debug, Clone)]
pub struct NoopEventBus;

#[async_trait]
impl EventBusPort for NoopEventBus {
    async fn publish(&self, _event: Event) {
        // Silently discard
    }

    fn subscribe(&self, _filter: EventFilter) -> broadcast::Receiver<Event> {
        let (_, rx) = broadcast::channel(1);
        rx
    }
}

/// Broadcast-based event bus implementation.
#[derive(Debug, Clone)]
pub struct BroadcastEventBus {
    sender: broadcast::Sender<Event>,
}

impl BroadcastEventBus {
    /// Create a new event bus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }
}

impl Default for BroadcastEventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

#[async_trait]
impl EventBusPort for BroadcastEventBus {
    async fn publish(&self, event: Event) {
        debug!(kind = ?event.kind, source = %event.source, "publishing event");
        // Ignore send error (no receivers)
        let _ = self.sender.send(event);
    }

    fn subscribe(&self, _filter: EventFilter) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

/// Helper to create a test event.
pub fn make_event(kind: EventKind, source: &str) -> Event {
    Event {
        id: clawx_types::ids::EventId::new(),
        timestamp: chrono::Utc::now(),
        source: source.to_string(),
        kind,
        payload: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_eventbus_discards_events() {
        let bus = NoopEventBus;
        let event = make_event(EventKind::HealthCheck, "test");
        // Should not panic
        bus.publish(event).await;
    }

    #[tokio::test]
    async fn noop_eventbus_subscribe_returns_receiver() {
        let bus = NoopEventBus;
        let _rx = bus.subscribe(EventFilter::default());
    }

    #[tokio::test]
    async fn broadcast_eventbus_publishes_and_receives() {
        let bus = BroadcastEventBus::default();
        let mut rx = bus.subscribe(EventFilter::default());

        let event = make_event(EventKind::AgentStarted, "test");
        bus.publish(event.clone()).await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.kind, EventKind::AgentStarted);
        assert_eq!(received.source, "test");
    }

    #[tokio::test]
    async fn broadcast_eventbus_multiple_subscribers() {
        let bus = BroadcastEventBus::new(16);
        let mut rx1 = bus.subscribe(EventFilter::default());
        let mut rx2 = bus.subscribe(EventFilter::default());

        let event = make_event(EventKind::Shutdown, "system");
        bus.publish(event).await;

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert_eq!(e1.kind, EventKind::Shutdown);
        assert_eq!(e2.kind, EventKind::Shutdown);
    }

    #[tokio::test]
    async fn broadcast_eventbus_no_subscribers_ok() {
        let bus = BroadcastEventBus::default();
        // No subscribers — should not panic
        bus.publish(make_event(EventKind::ConfigReloaded, "config")).await;
    }

    #[test]
    fn eventbus_port_is_object_safe() {
        fn _assert_object_safe(_: &dyn EventBusPort) {}
        fn _assert_arc(_: std::sync::Arc<dyn EventBusPort>) {}
    }
}
