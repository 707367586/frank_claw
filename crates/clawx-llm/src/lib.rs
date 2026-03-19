//! LLM provider abstraction for ClawX.
//!
//! Provides concrete implementations of the `LlmProvider` trait
//! defined in `clawx-types` for Anthropic, OpenAI, and Ollama backends.
//! Includes model routing, token accounting, and streaming support.

mod stub;
mod anthropic;
mod openai;
mod router;

pub use stub::StubLlmProvider;
pub use anthropic::AnthropicProvider;
pub use openai::OpenAiProvider;
pub use router::LlmRouter;
