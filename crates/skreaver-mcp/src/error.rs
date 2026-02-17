//! MCP error types
//!
//! This module provides comprehensive error types for MCP operations,
//! including mapping from rmcp errors and conversion to Skreaver errors.

use rmcp::model::Content;
use skreaver_core::FailureReason;
use thiserror::Error;

/// MCP operation result type
pub type McpResult<T> = Result<T, McpError>;

/// Errors that can occur during MCP operations
#[derive(Debug, Error)]
pub enum McpError {
    /// Tool execution failed
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Invalid tool parameters
    #[error("Invalid tool parameters: {0}")]
    InvalidParameters(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// MCP protocol error
    #[error("MCP protocol error: {0}")]
    ProtocolError(String),

    /// Transport error
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Server error
    #[error("Server error: {0}")]
    ServerError(String),

    /// Client error (for MCP bridge)
    #[error("Client error: {0}")]
    ClientError(String),

    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Task not found (2025-11-25 spec)
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// Task already in terminal state (2025-11-25 spec)
    #[error("Task already terminal: {0}")]
    TaskTerminal(String),

    /// Task operation timed out (2025-11-25 spec)
    #[error("Task timed out: {0}")]
    TaskTimeout(String),

    /// Elicitation declined by user (2025-11-25 spec)
    #[error("Elicitation declined: {0}")]
    ElicitationDeclined(String),

    /// Elicitation cancelled by user (2025-11-25 spec)
    #[error("Elicitation cancelled: {0}")]
    ElicitationCancelled(String),

    /// Resource not found
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl McpError {
    /// Convert MCP error to Skreaver FailureReason
    ///
    /// This provides a consistent mapping from MCP errors to Skreaver's
    /// error handling system.
    pub fn to_failure_reason(&self) -> FailureReason {
        match self {
            McpError::ToolExecutionFailed(msg) => FailureReason::Custom {
                category: "mcp_tool_error".to_string(),
                message: msg.clone(),
            },
            McpError::ToolNotFound(tool) => FailureReason::Custom {
                category: "mcp_tool_not_found".to_string(),
                message: format!("Tool '{}' not found on MCP server", tool),
            },
            McpError::InvalidParameters(msg) => FailureReason::InvalidInput {
                message: msg.clone(),
            },
            McpError::SerializationError(e) => FailureReason::InvalidInput {
                message: format!("JSON serialization error: {}", e),
            },
            McpError::IoError(e) => FailureReason::NetworkError {
                message: format!("IO error: {}", e),
            },
            McpError::ProtocolError(msg) => FailureReason::Custom {
                category: "mcp_protocol_error".to_string(),
                message: msg.clone(),
            },
            McpError::TransportError(msg) | McpError::ConnectionError(msg) => {
                FailureReason::NetworkError {
                    message: msg.clone(),
                }
            }
            McpError::ConnectionClosed => FailureReason::NetworkError {
                message: "MCP connection closed".to_string(),
            },
            McpError::ServerError(msg) | McpError::InternalError(msg) => {
                FailureReason::InternalError {
                    message: msg.clone(),
                }
            }
            McpError::ClientError(msg) => FailureReason::Custom {
                category: "mcp_client_error".to_string(),
                message: msg.clone(),
            },
            McpError::TaskNotFound(id) => FailureReason::Custom {
                category: "mcp_task_not_found".to_string(),
                message: format!("Task '{}' not found", id),
            },
            McpError::TaskTerminal(id) => FailureReason::Custom {
                category: "mcp_task_terminal".to_string(),
                message: format!("Task '{}' is already in terminal state", id),
            },
            McpError::TaskTimeout(msg) => FailureReason::Timeout {
                operation: msg.clone(),
            },
            McpError::ElicitationDeclined(msg) | McpError::ElicitationCancelled(msg) => {
                FailureReason::Custom {
                    category: "mcp_elicitation_cancelled".to_string(),
                    message: msg.clone(),
                }
            }
            McpError::ResourceNotFound(uri) => FailureReason::Custom {
                category: "mcp_resource_not_found".to_string(),
                message: format!("Resource '{}' not found", uri),
            },
            McpError::PermissionDenied(msg) => FailureReason::Custom {
                category: "mcp_permission_denied".to_string(),
                message: msg.clone(),
            },
            McpError::RateLimitExceeded(msg) => FailureReason::Custom {
                category: "mcp_rate_limit".to_string(),
                message: msg.clone(),
            },
        }
    }

    /// Check if this error is retryable
    ///
    /// Returns true for transient errors that may succeed on retry.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            McpError::TransportError(_)
                | McpError::ConnectionError(_)
                | McpError::TaskTimeout(_)
                | McpError::RateLimitExceeded(_)
        )
    }

    /// Check if this error indicates the connection should be closed
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            McpError::ConnectionClosed | McpError::ProtocolError(_) | McpError::InternalError(_)
        )
    }

    /// Create an error from an rmcp error message
    pub fn from_rmcp_error(error: impl std::fmt::Display) -> Self {
        let msg = error.to_string();

        // Parse common rmcp error patterns
        if msg.contains("not found") || msg.contains("NotFound") {
            McpError::ToolNotFound(msg)
        } else if msg.contains("invalid") || msg.contains("Invalid") {
            McpError::InvalidParameters(msg)
        } else if msg.contains("timeout") || msg.contains("Timeout") {
            McpError::TaskTimeout(msg)
        } else if msg.contains("closed") || msg.contains("Closed") {
            McpError::ConnectionClosed
        } else if msg.contains("denied") || msg.contains("Denied") {
            McpError::PermissionDenied(msg)
        } else if msg.contains("rate") || msg.contains("limit") {
            McpError::RateLimitExceeded(msg)
        } else {
            McpError::ClientError(msg)
        }
    }
}

/// Implement IntoContents for McpError so it can be returned from tool functions
impl rmcp::model::IntoContents for McpError {
    fn into_contents(self) -> Vec<Content> {
        vec![Content::text(self.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_to_failure_reason() {
        let err = McpError::ToolExecutionFailed("test error".to_string());
        let reason = err.to_failure_reason();
        assert!(
            matches!(reason, FailureReason::Custom { category, .. } if category == "mcp_tool_error")
        );

        let err = McpError::InvalidParameters("bad input".to_string());
        let reason = err.to_failure_reason();
        assert!(matches!(reason, FailureReason::InvalidInput { .. }));

        let err = McpError::ConnectionError("connection lost".to_string());
        let reason = err.to_failure_reason();
        assert!(matches!(reason, FailureReason::NetworkError { .. }));
    }

    #[test]
    fn test_error_retryable() {
        assert!(McpError::TransportError("timeout".to_string()).is_retryable());
        assert!(McpError::RateLimitExceeded("too many requests".to_string()).is_retryable());
        assert!(!McpError::ToolNotFound("tool".to_string()).is_retryable());
        assert!(!McpError::InvalidParameters("bad".to_string()).is_retryable());
    }

    #[test]
    fn test_error_fatal() {
        assert!(McpError::ConnectionClosed.is_fatal());
        assert!(McpError::ProtocolError("invalid message".to_string()).is_fatal());
        assert!(!McpError::ToolNotFound("tool".to_string()).is_fatal());
    }

    #[test]
    fn test_from_rmcp_error() {
        let err = McpError::from_rmcp_error("Tool not found: calculator");
        assert!(matches!(err, McpError::ToolNotFound(_)));

        let err = McpError::from_rmcp_error("Invalid parameters");
        assert!(matches!(err, McpError::InvalidParameters(_)));

        let err = McpError::from_rmcp_error("Connection closed");
        assert!(matches!(err, McpError::ConnectionClosed));
    }
}
