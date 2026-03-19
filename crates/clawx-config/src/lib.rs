//! Configuration loading and validation for ClawX.
//!
//! Provides TOML-based configuration parsing, directory initialization,
//! and the `ConfigService` trait implementation.

mod loader;

pub use loader::{ConfigLoader, expand_tilde};
