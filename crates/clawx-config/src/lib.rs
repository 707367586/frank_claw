//! Configuration loading and validation for ClawX.
//!
//! Provides TOML-based configuration parsing, environment overlay,
//! and runtime validation of agent and system settings.

/// Core configuration types and loader.
pub mod loader;

mod loader_impl {
    /// Placeholder configuration struct.
    #[derive(Debug, Clone)]
    pub struct ClawxConfig {
        _private: (),
    }
}

pub use loader_impl::ClawxConfig;
