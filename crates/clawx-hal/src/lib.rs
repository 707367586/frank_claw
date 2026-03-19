//! Hardware abstraction layer for ClawX.
//!
//! Provides platform-specific abstractions:
//! - FSEvents file watching (via `notify` crate)
//! - macOS Keychain credential storage (via `security-framework` crate)

mod fs_watcher;
mod keychain;

pub use fs_watcher::{FsEvent, FsEventKind, FsWatcher};
pub use keychain::{KeychainError, KeychainStore};
