//! LLM provider abstraction for ClawX.
//!
//! Provides concrete implementations of the `LlmProvider` trait
//! defined in `clawx-types` for Anthropic, OpenAI, and Ollama backends.
//! Includes model routing, token accounting, and streaming support.

mod stub;
mod anthropic;
mod openai;
mod zhipu;
mod router;

pub use stub::StubLlmProvider;
pub use anthropic::AnthropicProvider;
pub use openai::OpenAiProvider;
pub use zhipu::ZhipuProvider;
pub use router::LlmRouter;
