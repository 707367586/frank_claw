use thiserror::Error;

/// Unified error type for the ClawX system.
#[derive(Debug, Error)]
pub enum ClawxError {
    #[error("LLM provider error: {0}")]
    LlmProvider(String),

    #[error("LLM rate limited: retry after {retry_after_secs}s")]
    LlmRateLimited { retry_after_secs: u64 },

    #[error("Security denied: {reason}")]
    SecurityDenied { reason: String },

    #[error("DLP violation: {rule} matched on {direction}")]
    DlpViolation { rule: String, direction: String },

    #[error("Prompt injection detected: score={score:.2}")]
    PromptInjection { score: f64 },

    #[error("Database error: {0}")]
    Database(String),

    #[error("Vector store error: {0}")]
    VectorStore(String),

    #[error("WASM execution error: {0}")]
    WasmExecution(String),

    #[error("WASM timeout after {elapsed_ms}ms (limit {limit_ms}ms)")]
    WasmTimeout { elapsed_ms: u64, limit_ms: u64 },

    #[error("Snapshot error: {0}")]
    Snapshot(String),

    #[error("Channel connection error: {0}")]
    ChannelConnection(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Convenience Result alias used throughout ClawX.
pub type Result<T> = std::result::Result<T, ClawxError>;
