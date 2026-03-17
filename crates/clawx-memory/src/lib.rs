//! Three-layer memory system for ClawX.
//!
//! Implements working memory (in-context), short-term memory (session-scoped),
//! and long-term memory (persistent vector/SQLite store) to give agents
//! durable recall across conversations.

/// Working memory: current context window.
pub mod working;

/// Short-term memory: session-scoped storage.
pub mod short_term;

/// Long-term memory: persistent storage with embeddings.
pub mod long_term;
