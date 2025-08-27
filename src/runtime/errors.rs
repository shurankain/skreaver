//! # Runtime Error System
//!
//! Unified error handling for HTTP runtime with proper HTTP status code mapping,
//! request tracing, and structured error responses.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for tracking requests across the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(String);

impl RequestId {
    /// Create a new unique request ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing string (useful for parsing from headers)
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the underlying string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Structured error response for HTTP APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Machine-readable error code
    pub error: String,
    /// Human-readable error message
    pub message: String,
    /// Optional additional context or details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Request ID for tracking and debugging
    pub request_id: RequestId,
    /// Timestamp when error occurred
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional metadata for debugging
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl ErrorResponse {
    /// Create a new error response
    pub fn new(error: &str, message: &str, request_id: RequestId) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            details: None,
            request_id,
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add additional details to the error
    pub fn with_details<T: serde::Serialize>(mut self, details: T) -> Self {
        self.details = Some(serde_json::to_value(details).unwrap_or_default());
        self
    }

    /// Add metadata for debugging
    pub fn with_metadata<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Add multiple metadata entries
    pub fn with_metadata_map(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata.extend(metadata);
        self
    }
}

/// Runtime error types with proper categorization
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    // Agent-related errors
    #[error("Agent not found: {agent_id}")]
    AgentNotFound {
        agent_id: String,
        request_id: RequestId,
    },

    #[error("Agent creation failed: {reason}")]
    AgentCreationFailed {
        reason: String,
        agent_type: Option<String>,
        request_id: RequestId,
    },

    #[error("Agent operation failed: {operation} on {agent_id}")]
    AgentOperationFailed {
        agent_id: String,
        operation: String,
        reason: String,
        request_id: RequestId,
    },

    // Authentication errors
    #[error("Authentication required")]
    AuthenticationRequired { request_id: RequestId },

    #[error("Invalid authentication: {reason}")]
    InvalidAuthentication {
        reason: String,
        auth_method: Option<String>,
        request_id: RequestId,
    },

    #[error("Insufficient permissions: required {required:?}")]
    InsufficientPermissions {
        required: Vec<String>,
        provided: Vec<String>,
        request_id: RequestId,
    },

    #[error("Token creation failed: {reason}")]
    TokenCreationFailed {
        reason: String,
        request_id: RequestId,
    },

    // Rate limiting errors
    #[error("Rate limit exceeded: {limit_type}")]
    RateLimitExceeded {
        limit_type: String,
        retry_after: u64,
        current_usage: u32,
        limit: u32,
        request_id: RequestId,
    },

    // Input validation errors
    #[error("Invalid input: {field}")]
    InvalidInput {
        field: String,
        reason: String,
        provided_value: Option<String>,
        request_id: RequestId,
    },

    #[error("Missing required field: {field}")]
    MissingRequiredField {
        field: String,
        request_id: RequestId,
    },

    #[error("Invalid JSON: {reason}")]
    InvalidJson {
        reason: String,
        request_id: RequestId,
    },

    // System errors
    #[error("Internal server error: {reason}")]
    InternalError {
        reason: String,
        request_id: RequestId,
    },

    #[error("Service unavailable: {service}")]
    ServiceUnavailable {
        service: String,
        request_id: RequestId,
    },

    #[error("Timeout occurred: {operation}")]
    Timeout {
        operation: String,
        duration_ms: u64,
        request_id: RequestId,
    },

    // Memory/Storage errors
    #[error("Memory operation failed: {operation}")]
    MemoryError {
        operation: String,
        reason: String,
        request_id: RequestId,
    },

    // Tool execution errors
    #[error("Tool execution failed: {tool_name}")]
    ToolExecutionFailed {
        tool_name: String,
        reason: String,
        request_id: RequestId,
    },

    // Configuration errors
    #[error("Configuration error: {setting}")]
    ConfigurationError {
        setting: String,
        reason: String,
        request_id: RequestId,
    },
}

impl RuntimeError {
    /// Get the request ID associated with this error
    pub fn request_id(&self) -> &RequestId {
        match self {
            RuntimeError::AgentNotFound { request_id, .. } => request_id,
            RuntimeError::AgentCreationFailed { request_id, .. } => request_id,
            RuntimeError::AgentOperationFailed { request_id, .. } => request_id,
            RuntimeError::AuthenticationRequired { request_id } => request_id,
            RuntimeError::InvalidAuthentication { request_id, .. } => request_id,
            RuntimeError::InsufficientPermissions { request_id, .. } => request_id,
            RuntimeError::TokenCreationFailed { request_id, .. } => request_id,
            RuntimeError::RateLimitExceeded { request_id, .. } => request_id,
            RuntimeError::InvalidInput { request_id, .. } => request_id,
            RuntimeError::MissingRequiredField { request_id, .. } => request_id,
            RuntimeError::InvalidJson { request_id, .. } => request_id,
            RuntimeError::InternalError { request_id, .. } => request_id,
            RuntimeError::ServiceUnavailable { request_id, .. } => request_id,
            RuntimeError::Timeout { request_id, .. } => request_id,
            RuntimeError::MemoryError { request_id, .. } => request_id,
            RuntimeError::ToolExecutionFailed { request_id, .. } => request_id,
            RuntimeError::ConfigurationError { request_id, .. } => request_id,
        }
    }

    /// Get the appropriate HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            RuntimeError::AgentNotFound { .. } => StatusCode::NOT_FOUND,
            RuntimeError::AgentCreationFailed { .. } => StatusCode::BAD_REQUEST,
            RuntimeError::AgentOperationFailed { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            RuntimeError::AuthenticationRequired { .. } => StatusCode::UNAUTHORIZED,
            RuntimeError::InvalidAuthentication { .. } => StatusCode::UNAUTHORIZED,
            RuntimeError::InsufficientPermissions { .. } => StatusCode::FORBIDDEN,
            RuntimeError::TokenCreationFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            RuntimeError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
            RuntimeError::InvalidInput { .. } => StatusCode::BAD_REQUEST,
            RuntimeError::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RuntimeError::InvalidJson { .. } => StatusCode::BAD_REQUEST,
            RuntimeError::InternalError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            RuntimeError::ServiceUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
            RuntimeError::Timeout { .. } => StatusCode::REQUEST_TIMEOUT,
            RuntimeError::MemoryError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            RuntimeError::ToolExecutionFailed { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            RuntimeError::ConfigurationError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get the error code for this error type
    pub fn error_code(&self) -> &'static str {
        match self {
            RuntimeError::AgentNotFound { .. } => "agent_not_found",
            RuntimeError::AgentCreationFailed { .. } => "agent_creation_failed",
            RuntimeError::AgentOperationFailed { .. } => "agent_operation_failed",
            RuntimeError::AuthenticationRequired { .. } => "authentication_required",
            RuntimeError::InvalidAuthentication { .. } => "invalid_authentication",
            RuntimeError::InsufficientPermissions { .. } => "insufficient_permissions",
            RuntimeError::TokenCreationFailed { .. } => "token_creation_failed",
            RuntimeError::RateLimitExceeded { .. } => "rate_limit_exceeded",
            RuntimeError::InvalidInput { .. } => "invalid_input",
            RuntimeError::MissingRequiredField { .. } => "missing_required_field",
            RuntimeError::InvalidJson { .. } => "invalid_json",
            RuntimeError::InternalError { .. } => "internal_error",
            RuntimeError::ServiceUnavailable { .. } => "service_unavailable",
            RuntimeError::Timeout { .. } => "timeout",
            RuntimeError::MemoryError { .. } => "memory_error",
            RuntimeError::ToolExecutionFailed { .. } => "tool_execution_failed",
            RuntimeError::ConfigurationError { .. } => "configuration_error",
        }
    }

    /// Convert this error into a structured error response
    pub fn to_error_response(&self) -> ErrorResponse {
        let mut response = ErrorResponse::new(
            self.error_code(),
            &self.to_string(),
            self.request_id().clone(),
        );

        // Add specific details based on error type
        match self {
            RuntimeError::AgentNotFound { agent_id, .. } => {
                response = response.with_details(serde_json::json!({
                    "agent_id": agent_id
                }));
            }
            RuntimeError::AgentCreationFailed {
                agent_type, reason, ..
            } => {
                response = response.with_details(serde_json::json!({
                    "agent_type": agent_type,
                    "reason": reason
                }));
            }
            RuntimeError::AgentOperationFailed {
                agent_id,
                operation,
                reason,
                ..
            } => {
                response = response.with_details(serde_json::json!({
                    "agent_id": agent_id,
                    "operation": operation,
                    "reason": reason
                }));
            }
            RuntimeError::InvalidAuthentication {
                reason,
                auth_method,
                ..
            } => {
                response = response.with_details(serde_json::json!({
                    "reason": reason,
                    "auth_method": auth_method
                }));
            }
            RuntimeError::InsufficientPermissions {
                required, provided, ..
            } => {
                response = response.with_details(serde_json::json!({
                    "required_permissions": required,
                    "provided_permissions": provided
                }));
            }
            RuntimeError::RateLimitExceeded {
                limit_type,
                retry_after,
                current_usage,
                limit,
                ..
            } => {
                response = response.with_details(serde_json::json!({
                    "limit_type": limit_type,
                    "retry_after_seconds": retry_after,
                    "current_usage": current_usage,
                    "limit": limit
                }));
            }
            RuntimeError::InvalidInput {
                field,
                reason,
                provided_value,
                ..
            } => {
                response = response.with_details(serde_json::json!({
                    "field": field,
                    "reason": reason,
                    "provided_value": provided_value
                }));
            }
            RuntimeError::MissingRequiredField { field, .. } => {
                response = response.with_details(serde_json::json!({
                    "field": field
                }));
            }
            RuntimeError::Timeout {
                operation,
                duration_ms,
                ..
            } => {
                response = response.with_details(serde_json::json!({
                    "operation": operation,
                    "duration_ms": duration_ms
                }));
            }
            _ => {} // Other errors don't need special details
        }

        response
    }
}

/// Implement IntoResponse for RuntimeError to integrate with Axum
impl IntoResponse for RuntimeError {
    fn into_response(self) -> Response {
        let status_code = self.status_code();
        let error_response = self.to_error_response();

        // Log the error for monitoring
        tracing::error!(
            error_code = self.error_code(),
            request_id = %self.request_id(),
            status_code = %status_code,
            error_message = %self,
            "HTTP runtime error occurred"
        );

        // Create HTTP response
        let mut response = (status_code, Json(error_response)).into_response();

        // Add error-specific headers
        if let RuntimeError::RateLimitExceeded { retry_after, .. } = self {
            response
                .headers_mut()
                .insert("Retry-After", retry_after.to_string().parse().unwrap());
        }

        response
    }
}

/// Result type alias for HTTP runtime operations
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Helper trait for converting other error types to RuntimeError
pub trait IntoRuntimeError<T> {
    fn into_runtime_error(self, request_id: RequestId) -> RuntimeResult<T>;
}

impl<T> IntoRuntimeError<T> for Result<T, crate::error::MemoryError> {
    fn into_runtime_error(self, request_id: RequestId) -> RuntimeResult<T> {
        self.map_err(|e| RuntimeError::MemoryError {
            operation: "memory_operation".to_string(),
            reason: e.to_string(),
            request_id,
        })
    }
}

impl<T> IntoRuntimeError<T> for Result<T, serde_json::Error> {
    fn into_runtime_error(self, request_id: RequestId) -> RuntimeResult<T> {
        self.map_err(|e| RuntimeError::InvalidJson {
            reason: e.to_string(),
            request_id,
        })
    }
}

impl<T> IntoRuntimeError<T> for Result<T, jsonwebtoken::errors::Error> {
    fn into_runtime_error(self, request_id: RequestId) -> RuntimeResult<T> {
        self.map_err(|e| RuntimeError::TokenCreationFailed {
            reason: e.to_string(),
            request_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_creation() {
        let id1 = RequestId::new();
        let id2 = RequestId::new();

        assert_ne!(id1, id2);
        assert!(!id1.as_str().is_empty());
    }

    #[test]
    fn test_error_response_creation() {
        let request_id = RequestId::new();
        let error = RuntimeError::AgentNotFound {
            agent_id: "test-agent".to_string(),
            request_id: request_id.clone(),
        };

        let response = error.to_error_response();

        assert_eq!(response.error, "agent_not_found");
        assert_eq!(response.request_id, request_id);
        assert!(response.details.is_some());
    }

    #[test]
    fn test_status_code_mapping() {
        let request_id = RequestId::new();

        let not_found = RuntimeError::AgentNotFound {
            agent_id: "test".to_string(),
            request_id: request_id.clone(),
        };
        assert_eq!(not_found.status_code(), StatusCode::NOT_FOUND);

        let unauthorized = RuntimeError::AuthenticationRequired {
            request_id: request_id.clone(),
        };
        assert_eq!(unauthorized.status_code(), StatusCode::UNAUTHORIZED);

        let rate_limit = RuntimeError::RateLimitExceeded {
            limit_type: "global".to_string(),
            retry_after: 60,
            current_usage: 100,
            limit: 50,
            request_id,
        };
        assert_eq!(rate_limit.status_code(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_error_details() {
        let request_id = RequestId::new();
        let error = RuntimeError::InvalidInput {
            field: "agent_id".to_string(),
            reason: "must not be empty".to_string(),
            provided_value: Some("".to_string()),
            request_id,
        };

        let response = error.to_error_response();
        let details = response.details.unwrap();

        assert_eq!(details["field"], "agent_id");
        assert_eq!(details["reason"], "must not be empty");
        assert_eq!(details["provided_value"], "");
    }
}
