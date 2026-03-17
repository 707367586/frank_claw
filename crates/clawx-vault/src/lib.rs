//! APFS snapshots and file-level restore for ClawX.
//!
//! Provides safe rollback capabilities by leveraging macOS APFS snapshots
//! before destructive agent operations, with per-file restore support.

/// Snapshot management.
pub mod snapshot;

/// File-level restore operations.
pub mod restore;
