//! Three-layer memory system for ClawX.
//!
//! Implements working memory (in-context), short-term memory (session-scoped),
//! and long-term memory (persistent vector/SQLite store) to give agents
//! durable recall across conversations.

pub mod working;
pub mod short_term;
pub mod long_term;
pub mod decay;
pub mod extraction;
pub mod consolidation;

#[cfg(test)]
mod long_term_tests;

mod stub;
pub use stub::{StubMemoryService, StubWorkingMemoryManager};
pub use long_term::SqliteMemoryService;
pub use decay::run_memory_decay;
pub use extraction::{LlmMemoryExtractor, StubMemoryExtractor};
pub use working::{RealWorkingMemoryManager, WorkingMemoryConfig};
