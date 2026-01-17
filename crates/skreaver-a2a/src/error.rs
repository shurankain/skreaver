//! A2A Protocol Error Types
//!
//! This module defines error types for the A2A protocol implementation.

use thiserror::Error;

/// Result type for A2A operations
pub type A2aResult<T> = Result<T, A2aError>;

/// Errors that can occur in A2A protocol operations
#[derive(Debug, Error)]
pub enum A2aError {
    /// Task not found
    #[error("Task not found: {task_id}")]
    TaskNotFound { task_id: String },

    /// Task already exists
    #[error("Task already exists: {task_id}")]
    TaskAlreadyExists { task_id: String },

    /// Task is in a terminal state and cannot be modified
    #[error("Task {task_id} is in terminal state: {status}")]
    TaskTerminated { task_id: String, status: String },

    /// Invalid task state transition
    #[error("Invalid state transition for task {task_id}: {from} -> {to}")]
    InvalidStateTransition {
        task_id: String,
        from: String,
        to: String,
    },

    /// Agent not found
    #[error("Agent not found: {agent_id}")]
    AgentNotFound { agent_id: String },

    /// Agent card validation failed
    #[error("Invalid agent card: {reason}")]
    InvalidAgentCard { reason: String },

    /// Message validation failed
    #[error("Invalid message: {reason}")]
    InvalidMessage { reason: String },

    /// Authentication required
    #[error("Authentication required")]
    AuthenticationRequired,

    /// Authentication failed
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    /// Authorization failed
    #[error("Not authorized: {reason}")]
    NotAuthorized { reason: String },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: retry after {retry_after_seconds} seconds")]
    RateLimitExceeded { retry_after_seconds: u64 },

    /// Connection error
    #[error("Connection error: {message}")]
    ConnectionError { message: String },

    /// Request timeout
    #[error("Request timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Protocol error
    #[error("Protocol error: {message}")]
    ProtocolError { message: String },

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// URL parsing error
    #[error("Invalid URL: {0}")]
    UrlError(#[from] url::ParseError),

    /// HTTP error (when client feature is enabled)
    #[cfg(feature = "client")]
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// WebSocket error
    #[error("WebSocket error: {message}")]
    WebSocketError { message: String },

    /// Internal error
    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl A2aError {
    /// Create a task not found error
    pub fn task_not_found(task_id: impl Into<String>) -> Self {
        Self::TaskNotFound {
            task_id: task_id.into(),
        }
    }

    /// Create a task terminated error
    pub fn task_terminated(task_id: impl Into<String>, status: impl Into<String>) -> Self {
        Self::TaskTerminated {
            task_id: task_id.into(),
            status: status.into(),
        }
    }

    /// Create an agent not found error
    pub fn agent_not_found(agent_id: impl Into<String>) -> Self {
        Self::AgentNotFound {
            agent_id: agent_id.into(),
        }
    }

    /// Create an invalid agent card error
    pub fn invalid_agent_card(reason: impl Into<String>) -> Self {
        Self::InvalidAgentCard {
            reason: reason.into(),
        }
    }

    /// Create an invalid message error
    pub fn invalid_message(reason: impl Into<String>) -> Self {
        Self::InvalidMessage {
            reason: reason.into(),
        }
    }

    /// Create a connection error
    pub fn connection_error(message: impl Into<String>) -> Self {
        Self::ConnectionError {
            message: message.into(),
        }
    }

    /// Create a protocol error
    pub fn protocol_error(message: impl Into<String>) -> Self {
        Self::ProtocolError {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            A2aError::ConnectionError { .. }
                | A2aError::Timeout { .. }
                | A2aError::RateLimitExceeded { .. }
                | A2aError::WebSocketError { .. }
        )
    }

    /// Get suggested retry delay in seconds, if applicable
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            A2aError::RateLimitExceeded {
                retry_after_seconds,
            } => Some(*retry_after_seconds),
            A2aError::ConnectionError { .. } | A2aError::Timeout { .. } => Some(1), // Default 1 second
            _ => None,
        }
    }
}

/// A2A protocol error response format (matches JSON-RPC style)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ErrorResponse {
    /// Create a new error response
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Add data to the error response
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
}

impl From<A2aError> for ErrorResponse {
    fn from(err: A2aError) -> Self {
        let (code, message) = match &err {
            A2aError::TaskNotFound { .. } => (404, err.to_string()),
            A2aError::TaskAlreadyExists { .. } => (409, err.to_string()),
            A2aError::TaskTerminated { .. } => (400, err.to_string()),
            A2aError::InvalidStateTransition { .. } => (400, err.to_string()),
            A2aError::AgentNotFound { .. } => (404, err.to_string()),
            A2aError::InvalidAgentCard { .. } => (400, err.to_string()),
            A2aError::InvalidMessage { .. } => (400, err.to_string()),
            A2aError::AuthenticationRequired => (401, err.to_string()),
            A2aError::AuthenticationFailed { .. } => (401, err.to_string()),
            A2aError::NotAuthorized { .. } => (403, err.to_string()),
            A2aError::RateLimitExceeded { .. } => (429, err.to_string()),
            A2aError::ConnectionError { .. } => (502, err.to_string()),
            A2aError::Timeout { .. } => (504, err.to_string()),
            A2aError::ProtocolError { .. } => (400, err.to_string()),
            A2aError::SerializationError(_) => (400, err.to_string()),
            A2aError::UrlError(_) => (400, err.to_string()),
            #[cfg(feature = "client")]
            A2aError::HttpError(_) => (502, err.to_string()),
            A2aError::WebSocketError { .. } => (502, err.to_string()),
            A2aError::InternalError { .. } => (500, err.to_string()),
        };

        ErrorResponse::new(code, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = A2aError::task_not_found("task-123");
        assert!(matches!(err, A2aError::TaskNotFound { .. }));
        assert_eq!(err.to_string(), "Task not found: task-123");
    }

    #[test]
    fn test_error_retryable() {
        let connection_err = A2aError::connection_error("connection refused");
        assert!(connection_err.is_retryable());

        let not_found = A2aError::task_not_found("task-123");
        assert!(!not_found.is_retryable());
    }

    #[test]
    fn test_retry_after() {
        let rate_limit = A2aError::RateLimitExceeded {
            retry_after_seconds: 60,
        };
        assert_eq!(rate_limit.retry_after(), Some(60));

        let timeout = A2aError::Timeout { timeout_ms: 5000 };
        assert_eq!(timeout.retry_after(), Some(1));
    }

    #[test]
    fn test_error_response_conversion() {
        let err = A2aError::task_not_found("task-123");
        let response: ErrorResponse = err.into();

        assert_eq!(response.code, 404);
        assert!(response.message.contains("task-123"));
    }
}
