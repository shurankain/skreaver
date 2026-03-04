//! A2A Protocol Error Types
//!
//! This module defines error types for A2A HTTP handlers, mapping internal
//! errors to appropriate HTTP responses following the A2A protocol specification.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use skreaver_a2a::A2aError;

/// A2A API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aApiError {
    /// Error code (HTTP status code or custom code)
    pub code: i32,
    /// Human-readable error message
    pub message: String,
    /// Additional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl A2aApiError {
    /// Create a new error with code and message
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    /// Add details to the error
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Task not found error
    pub fn task_not_found(task_id: &str) -> Self {
        Self::new(404, format!("Task not found: {}", task_id))
    }

    /// Task already exists error
    pub fn task_already_exists(task_id: &str) -> Self {
        Self::new(409, format!("Task already exists: {}", task_id))
    }

    /// Invalid request error
    pub fn invalid_request(reason: impl Into<String>) -> Self {
        Self::new(400, format!("Invalid request: {}", reason.into()))
    }

    /// Authentication required error
    pub fn authentication_required() -> Self {
        Self::new(401, "Authentication required")
    }

    /// Permission denied error
    pub fn permission_denied(reason: impl Into<String>) -> Self {
        Self::new(403, format!("Permission denied: {}", reason.into()))
    }

    /// Internal error
    pub fn internal_error(reason: impl Into<String>) -> Self {
        Self::new(500, format!("Internal error: {}", reason.into()))
    }

    /// Rate limit exceeded error
    pub fn rate_limit_exceeded(retry_after: u64) -> Self {
        Self::new(429, "Rate limit exceeded").with_details(serde_json::json!({
            "retry_after_seconds": retry_after
        }))
    }

    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self.code {
            400 => StatusCode::BAD_REQUEST,
            401 => StatusCode::UNAUTHORIZED,
            403 => StatusCode::FORBIDDEN,
            404 => StatusCode::NOT_FOUND,
            409 => StatusCode::CONFLICT,
            429 => StatusCode::TOO_MANY_REQUESTS,
            500 => StatusCode::INTERNAL_SERVER_ERROR,
            503 => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for A2aApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        (status, Json(self)).into_response()
    }
}

impl From<A2aError> for A2aApiError {
    fn from(err: A2aError) -> Self {
        match err {
            A2aError::TaskNotFound { task_id } => Self::task_not_found(&task_id),
            A2aError::TaskAlreadyExists { task_id } => Self::task_already_exists(&task_id),
            A2aError::TaskTerminated { task_id, status } => Self::new(
                400,
                format!("Task {} is in terminal state: {}", task_id, status),
            ),
            A2aError::InvalidStateTransition { task_id, from, to } => Self::new(
                400,
                format!(
                    "Invalid state transition for task {}: {} -> {}",
                    task_id, from, to
                ),
            ),
            A2aError::AgentNotFound { agent_id } => {
                Self::new(404, format!("Agent not found: {}", agent_id))
            }
            A2aError::InvalidAgentCard { reason } => {
                Self::new(400, format!("Invalid agent card: {}", reason))
            }
            A2aError::InvalidMessage { reason } => {
                Self::new(400, format!("Invalid message: {}", reason))
            }
            A2aError::AuthenticationRequired => Self::authentication_required(),
            A2aError::AuthenticationFailed { reason } => {
                Self::new(401, format!("Authentication failed: {}", reason))
            }
            A2aError::NotAuthorized { reason } => Self::permission_denied(reason),
            A2aError::RateLimitExceeded {
                retry_after_seconds,
            } => Self::rate_limit_exceeded(retry_after_seconds),
            A2aError::ConnectionError { message } => {
                Self::new(502, format!("Connection error: {}", message))
            }
            A2aError::Timeout { timeout_ms } => {
                Self::new(504, format!("Request timeout after {}ms", timeout_ms))
            }
            A2aError::ProtocolError { message } => {
                Self::new(400, format!("Protocol error: {}", message))
            }
            A2aError::SerializationError(e) => {
                Self::new(400, format!("Serialization error: {}", e))
            }
            A2aError::UrlError(e) => Self::new(400, format!("Invalid URL: {}", e)),
            A2aError::WebSocketError { message } => {
                Self::new(502, format!("WebSocket error: {}", message))
            }
            A2aError::InternalError { message } => Self::internal_error(message),
        }
    }
}

/// Result type for A2A API handlers
pub type A2aApiResult<T> = Result<T, A2aApiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = A2aApiError::task_not_found("task-123");
        assert_eq!(err.code, 404);
        assert!(err.message.contains("task-123"));
    }

    #[test]
    fn test_error_with_details() {
        let err = A2aApiError::rate_limit_exceeded(60);
        assert_eq!(err.code, 429);
        assert!(err.details.is_some());

        let details = err.details.unwrap();
        assert_eq!(details["retry_after_seconds"], 60);
    }

    #[test]
    fn test_status_code_mapping() {
        assert_eq!(
            A2aApiError::new(400, "test").status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            A2aApiError::new(401, "test").status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            A2aApiError::new(404, "test").status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            A2aApiError::new(500, "test").status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_from_a2a_error() {
        let a2a_err = A2aError::task_not_found("task-456");
        let api_err: A2aApiError = a2a_err.into();
        assert_eq!(api_err.code, 404);
        assert!(api_err.message.contains("task-456"));
    }
}
