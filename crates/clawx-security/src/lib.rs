//! Sandbox, DLP, audit, and prompt defense for ClawX.
//!
//! Enforces security boundaries around agent operations including
//! data loss prevention scanning, audit logging, and prompt injection
//! detection.

/// Sandbox enforcement.
pub mod sandbox;

/// Data loss prevention scanning.
pub mod dlp;

/// Audit trail.
pub mod audit;

/// Prompt injection defense.
pub mod prompt_defense;
