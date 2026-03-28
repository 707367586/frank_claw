use thiserror::Error;

/// Unified error type for the ClawX system.
#[derive(Debug, Error)]
pub enum ClawxError {
    #[error("not found: {entity} {id}")]
    NotFound { entity: String, id: String },

    #[error("unauthorized: {reason}")]
    Unauthorized { reason: String },

    #[error("conflict: {reason}")]
    Conflict { reason: String },

    #[error("validation error: {0}")]
    Validation(String),

    #[error("LLM provider error: {0}")]
    LlmProvider(String),

    #[error("LLM rate limited: retry after {retry_after_secs}s")]
    LlmRateLimited { retry_after_secs: u64 },

    #[error("security denied: {reason}")]
    SecurityDenied { reason: String },

    #[error("DLP violation: {rule} matched on {direction}")]
    DlpViolation { rule: String, direction: String },

    #[error("database error: {0}")]
    Database(String),

    #[error("vector store error: {0}")]
    VectorStore(String),

    #[error("snapshot error: {0}")]
    Snapshot(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("task error: {0}")]
    Task(String),

    #[error("channel error: {0}")]
    Channel(String),

    #[error("skill error: {0}")]
    Skill(String),

    #[error("permission denied: {reason}")]
    PermissionDenied { reason: String },

    #[error("resource locked: {0}")]
    ResourceLocked(String),

    #[error("prompt injection detected: {0}")]
    PromptInjection(String),

    #[error("sandbox error: {0}")]
    Sandbox(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience Result alias used throughout ClawX.
pub type Result<T> = std::result::Result<T, ClawxError>;
