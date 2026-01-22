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
    Mcp(#[from] skreaver_mcp::McpError),

    /// A2A-specific error.
    #[cfg(feature = "a2a")]
    #[error("A2A error: {0}")]
    A2a(#[from] skreaver_a2a::A2aError),
}

impl AgentError {
    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AgentError::ConnectionError(_) | AgentError::Timeout(_)
        )
    }

    /// Get the error code suitable for logging or reporting.
    pub fn error_code(&self) -> &'static str {
        match self {
            AgentError::ProtocolNotSupported(_) => "PROTOCOL_NOT_SUPPORTED",
            AgentError::CapabilityNotFound(_) => "CAPABILITY_NOT_FOUND",
            AgentError::TaskNotFound(_) => "TASK_NOT_FOUND",
            AgentError::AgentNotFound(_) => "AGENT_NOT_FOUND",
            AgentError::ConnectionError(_) => "CONNECTION_ERROR",
            AgentError::Timeout(_) => "TIMEOUT",
            AgentError::AuthenticationFailed(_) => "AUTH_FAILED",
            AgentError::InvalidRequest(_) => "INVALID_REQUEST",
            AgentError::InvalidResponse(_) => "INVALID_RESPONSE",
            AgentError::SerializationError(_) => "SERIALIZATION_ERROR",
            AgentError::Internal(_) => "INTERNAL_ERROR",
            #[cfg(feature = "mcp")]
            AgentError::Mcp(_) => "MCP_ERROR",
            #[cfg(feature = "a2a")]
            AgentError::A2a(_) => "A2A_ERROR",
        }
    }
}

/// Result type for agent operations.
pub type AgentResult<T> = Result<T, AgentError>;

impl From<serde_json::Error> for AgentError {
    fn from(err: serde_json::Error) -> Self {
        AgentError::SerializationError(err.to_string())
    }
}

impl From<std::io::Error> for AgentError {
    fn from(err: std::io::Error) -> Self {
        AgentError::Internal(format!("IO error: {}", err))
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

    #[test]
    fn test_is_retryable() {
        assert!(AgentError::ConnectionError("failed".to_string()).is_retryable());
        assert!(AgentError::Timeout("timeout".to_string()).is_retryable());
        assert!(!AgentError::TaskNotFound("123".to_string()).is_retryable());
    }

    #[test]
    fn test_error_code() {
        assert_eq!(
            AgentError::TaskNotFound("123".to_string()).error_code(),
            "TASK_NOT_FOUND"
        );
        assert_eq!(
            AgentError::ConnectionError("failed".to_string()).error_code(),
            "CONNECTION_ERROR"
        );
    }
}
