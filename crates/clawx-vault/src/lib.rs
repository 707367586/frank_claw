//! APFS snapshots and file-level restore for ClawX.
//!
//! Provides safe rollback capabilities by leveraging macOS APFS snapshots
//! before destructive agent operations, with per-file restore support.

pub mod snapshot;

mod stub;
pub use stub::StubVaultService;
pub use snapshot::{SqliteVaultService, cleanup_old_snapshots};
