//! LLM provider abstraction for ClawX.
//!
//! Defines a unified interface for interacting with large language model
//! providers (e.g., Claude, OpenAI) including streaming, tool use, and
//! token accounting.

/// Trait representing an LLM provider backend.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a completion request and return the response.
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, LlmError>;
}

/// A completion request to an LLM.
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    /// The prompt or messages to send.
    pub messages: Vec<String>,
}

/// A completion response from an LLM.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// The generated text.
    pub text: String,
}

/// Errors from LLM operations.
#[derive(Debug)]
pub enum LlmError {
    /// Provider returned an error.
    Provider(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::Provider(msg) => write!(f, "provider error: {msg}"),
        }
    }
}

impl std::error::Error for LlmError {}
