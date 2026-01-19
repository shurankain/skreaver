//! Error types for the unified agent interface.

use thiserror::Error;

/// Errors that can occur when working with unified agents.
#[derive(Debug, Error)]
pub enum AgentError {
    /// The requested protocol is not supported.
    #[error("Protocol not supported: {0}")]
    ProtocolNotSupported(String),

    /// The requested capability is not available.
    #[error("Capability not found: {0}")]
    CapabilityNotFound(String),

    /// Task not found.
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// Agent not found.
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Connection error.
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Timeout error.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Authentication error.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Invalid request.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Invalid response.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// MCP-specific error.
    #[cfg(feature = "mcp")]
    #[error("MCP error: {0}")]
    Mcp(String),

    /// A2A-specific error.
    #[cfg(feature = "a2a")]
    #[error("A2A error: {0}")]
    A2a(#[from] skreaver_a2a::A2aError),
}

/// Result type for agent operations.
pub type AgentResult<T> = Result<T, AgentError>;

impl From<serde_json::Error> for AgentError {
    fn from(err: serde_json::Error) -> Self {
        AgentError::SerializationError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AgentError::ProtocolNotSupported("unknown".to_string());
        assert_eq!(err.to_string(), "Protocol not supported: unknown");
    }

    #[test]
    fn test_result_type() {
        let ok: AgentResult<i32> = Ok(42);
        assert!(ok.is_ok());

        let err: AgentResult<i32> = Err(AgentError::TaskNotFound("123".to_string()));
        assert!(err.is_err());
    }
}
