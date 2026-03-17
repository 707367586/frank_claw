//! Hardware abstraction layer for ClawX (v0.4 stub).
//!
//! Will abstract over platform-specific hardware interfaces (sensors,
//! peripherals, system APIs) for portable agent-hardware interaction.

/// Placeholder HAL trait.
#[async_trait::async_trait]
pub trait HardwareInterface: Send + Sync {
    /// Query hardware capabilities.
    async fn capabilities(&self) -> Vec<String>;
}
