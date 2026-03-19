//! Sandbox, DLP, audit, and prompt defense for ClawX.
//!
//! Enforces security boundaries around agent operations including
//! data loss prevention scanning, audit logging, and prompt injection
//! detection.

pub mod sandbox;
pub mod dlp;
pub mod audit;
pub mod prompt_defense;
pub mod rate_limit;
pub mod network;

mod guard;
pub use guard::{PermissiveSecurityGuard, ClawxSecurityGuard};
