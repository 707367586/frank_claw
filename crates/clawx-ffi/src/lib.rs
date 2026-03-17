//! UniFFI bridge to Swift for ClawX.
//!
//! Exposes the Rust runtime to the SwiftUI frontend via Mozilla UniFFI,
//! providing a safe, generated FFI layer for agent control and status.

/// FFI-exported functions and types.
pub mod bridge;
