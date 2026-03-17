//! Publish-subscribe event bus for ClawX.
//!
//! Provides an async, typed event bus that decouples producers and
//! consumers across the agent runtime.

/// The core event bus handle.
#[derive(Debug, Clone)]
pub struct EventBus {
    _private: (),
}

/// Trait for event subscribers.
#[async_trait::async_trait]
pub trait Subscriber: Send + Sync {
    /// Handle an incoming event.
    async fn on_event(&self, event: &clawx_types::event::Event);
}
