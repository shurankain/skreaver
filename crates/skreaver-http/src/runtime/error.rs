//! Unified error handling system for HTTP runtime

use axum::{
    extract::Request,
    http::{
        StatusCode,
        header::{self, HeaderValue},
    },
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::runtime::{
    agent_instance::{AgentExecutionError, AgentIdError},
    auth_token::AuthTokenError,
    rate_limit::RateLimitError,
};

/// Request ID for distributed tracing and error correlation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(String);

impl RequestId {
    /// Generate a new random request ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from existing string (for header extraction)
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    /// Get as string slice
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

/// Extension for storing RequestId in request context
#[derive(Clone, Debug)]
pub struct RequestIdExtension(pub RequestId);

/// Middleware for request ID generation and propagation
///
/// Extracts X-Request-ID header or generates new UUID, stores in extensions,
/// and adds to response headers for distributed tracing.
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Extract or generate request ID
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| RequestId::from_string(s.to_string()))
        .unwrap_or_default();

    // Store in extensions
    request
        .extensions_mut()
        .insert(RequestIdExtension(request_id.clone()));

    // Process request
    let mut response = next.run(request).await;

    // Add to response headers
    if let Ok(value) = HeaderValue::from_str(request_id.as_str()) {
        response
            .headers_mut()
            .insert(header::HeaderName::from_static("x-request-id"), value);
    }

    response
}

/// Comprehensive error system for HTTP runtime
#[derive(Debug)]
pub enum RuntimeError {
    /// Agent-related errors
    Agent(AgentError),
    /// Authentication/authorization errors
    Auth(AuthError),
    /// Rate limiting errors
    RateLimit(RateLimitError),
    /// Configuration errors
    Config(ConfigError),
    /// Internal server errors
    Internal(InternalError),
    /// Validation errors
    Validation(ValidationError),
}

/// Agent-specific errors
#[derive(Debug, Clone)]
pub enum AgentError {
    /// Agent not found
    NotFound(String),
    /// Agent ID validation failed
    InvalidId(AgentIdError),
    /// Agent execution failed
    ExecutionFailed(AgentExecutionError),
    /// Agent is in wrong state for operation
    InvalidState {
        agent_id: String,
        current_state: String,
        required_state: String,
    },
    /// Agent creation failed
    CreationFailed(String),
}

/// Authentication/authorization errors
#[derive(Debug, Clone)]
pub enum AuthError {
    /// No authentication provided
    Missing,
    /// Invalid token format or content
    InvalidToken(AuthTokenError),
    /// Token has expired
    Expired,
    /// Insufficient permissions for operation
    Forbidden {
        required_permissions: Vec<String>,
        user_permissions: Vec<String>,
    },
    /// User not found
    UserNotFound(String),
}

/// Configuration errors
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// Invalid rate limit configuration
    InvalidRateLimit(String),
    /// Invalid timeout configuration
    InvalidTimeout(u64),
    /// Missing required configuration
    MissingConfig(String),
}

/// Internal server errors
#[derive(Debug, Clone)]
pub enum InternalError {
    /// Database connection failed
    DatabaseError(String),
    /// Serialization/deserialization failed
    SerializationError(String),
    /// Concurrent access error
    ConcurrencyError(String),
    /// Unexpected system error
    Unexpected(String),
}

/// Input validation errors
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Required field missing
    MissingField(String),
    /// Field value invalid
    InvalidField {
        field: String,
        value: String,
        reason: String,
    },
    /// Request body too large
    RequestTooLarge,
    /// Invalid JSON structure
    InvalidJson(String),
}

/// Structured error response for HTTP API
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// Machine-readable error code
    #[schema(example = "agent_not_found")]
    pub code: String,
    /// Human-readable error message
    #[schema(example = "Agent with ID 'agent-123' was not found")]
    pub message: String,
    /// Additional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
    /// Request ID for tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Timestamp when error occurred
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl RuntimeError {
    /// Get the appropriate HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Agent(err) => match err {
                AgentError::NotFound(_) => StatusCode::NOT_FOUND,
                AgentError::InvalidId(_) => StatusCode::BAD_REQUEST,
                AgentError::ExecutionFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                AgentError::InvalidState { .. } => StatusCode::CONFLICT,
                AgentError::CreationFailed(_) => StatusCode::BAD_REQUEST,
            },
            Self::Auth(err) => match err {
                AuthError::Missing => StatusCode::UNAUTHORIZED,
                AuthError::InvalidToken(_) => StatusCode::UNAUTHORIZED,
                AuthError::Expired => StatusCode::UNAUTHORIZED,
                AuthError::Forbidden { .. } => StatusCode::FORBIDDEN,
                AuthError::UserNotFound(_) => StatusCode::UNAUTHORIZED,
            },
            Self::RateLimit(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Validation(err) => match err {
                ValidationError::RequestTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::BAD_REQUEST,
            },
        }
    }

    /// Get the error code for API responses
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Agent(err) => match err {
                AgentError::NotFound(_) => "agent_not_found",
                AgentError::InvalidId(_) => "invalid_agent_id",
                AgentError::ExecutionFailed(_) => "agent_execution_failed",
                AgentError::InvalidState { .. } => "agent_invalid_state",
                AgentError::CreationFailed(_) => "agent_creation_failed",
            },
            Self::Auth(err) => match err {
                AuthError::Missing => "authentication_required",
                AuthError::InvalidToken(_) => "invalid_token",
                AuthError::Expired => "token_expired",
                AuthError::Forbidden { .. } => "insufficient_permissions",
                AuthError::UserNotFound(_) => "user_not_found",
            },
            Self::RateLimit(_) => "rate_limit_exceeded",
            Self::Config(_) => "configuration_error",
            Self::Internal(_) => "internal_server_error",
            Self::Validation(err) => match err {
                ValidationError::MissingField(_) => "missing_field",
                ValidationError::InvalidField { .. } => "invalid_field",
                ValidationError::RequestTooLarge => "request_too_large",
                ValidationError::InvalidJson(_) => "invalid_json",
            },
        }
    }

    /// Create error response with optional details
    pub fn to_response(&self, request_id: Option<String>) -> ErrorResponse {
        let mut details = HashMap::new();

        // Add error-specific details
        match self {
            Self::Agent(AgentError::InvalidState {
                agent_id,
                current_state,
                required_state,
            }) => {
                details.insert(
                    "agent_id".to_string(),
                    serde_json::Value::String(agent_id.clone()),
                );
                details.insert(
                    "current_state".to_string(),
                    serde_json::Value::String(current_state.clone()),
                );
                details.insert(
                    "required_state".to_string(),
                    serde_json::Value::String(required_state.clone()),
                );
            }
            Self::Auth(AuthError::Forbidden {
                required_permissions,
                user_permissions,
            }) => {
                details.insert(
                    "required_permissions".to_string(),
                    serde_json::Value::Array(
                        required_permissions
                            .iter()
                            .map(|p| serde_json::Value::String(p.clone()))
                            .collect(),
                    ),
                );
                details.insert(
                    "user_permissions".to_string(),
                    serde_json::Value::Array(
                        user_permissions
                            .iter()
                            .map(|p| serde_json::Value::String(p.clone()))
                            .collect(),
                    ),
                );
            }
            Self::RateLimit(rate_error) => {
                details.insert(
                    "retry_after".to_string(),
                    serde_json::Value::Number(rate_error.retry_after.into()),
                );
            }
            Self::Validation(ValidationError::InvalidField {
                field,
                value,
                reason,
            }) => {
                details.insert(
                    "field".to_string(),
                    serde_json::Value::String(field.clone()),
                );
                details.insert(
                    "value".to_string(),
                    serde_json::Value::String(value.clone()),
                );
                details.insert(
                    "reason".to_string(),
                    serde_json::Value::String(reason.clone()),
                );
            }
            _ => {}
        }

        ErrorResponse {
            code: self.error_code().to_string(),
            message: self.to_string(),
            details: if details.is_empty() {
                None
            } else {
                Some(details)
            },
            request_id,
            timestamp: chrono::Utc::now(),
        }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent(err) => write!(f, "{}", err),
            Self::Auth(err) => write!(f, "{}", err),
            Self::RateLimit(err) => write!(f, "Rate limit exceeded: {:?}", err),
            Self::Config(err) => write!(f, "{}", err),
            Self::Internal(err) => write!(f, "{}", err),
            Self::Validation(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "Agent with ID '{}' not found", id),
            Self::InvalidId(err) => write!(f, "Invalid agent ID: {}", err),
            Self::ExecutionFailed(err) => write!(f, "Agent execution failed: {}", err),
            Self::InvalidState {
                agent_id,
                current_state,
                required_state,
            } => {
                write!(
                    f,
                    "Agent '{}' is in state '{}' but operation requires '{}'",
                    agent_id, current_state, required_state
                )
            }
            Self::CreationFailed(reason) => write!(f, "Failed to create agent: {}", reason),
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "Authentication is required"),
            Self::InvalidToken(err) => write!(f, "Invalid authentication token: {}", err),
            Self::Expired => write!(f, "Authentication token has expired"),
            Self::Forbidden {
                required_permissions,
                ..
            } => {
                write!(
                    f,
                    "Insufficient permissions. Required: {}",
                    required_permissions.join(", ")
                )
            }
            Self::UserNotFound(user) => write!(f, "User '{}' not found", user),
        }
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRateLimit(msg) => write!(f, "Invalid rate limit configuration: {}", msg),
            Self::InvalidTimeout(timeout) => {
                write!(f, "Invalid timeout configuration: {}s", timeout)
            }
            Self::MissingConfig(key) => write!(f, "Missing required configuration: {}", key),
        }
    }
}

impl std::fmt::Display for InternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Self::ConcurrencyError(msg) => write!(f, "Concurrency error: {}", msg),
            Self::Unexpected(msg) => write!(f, "Unexpected error: {}", msg),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "Missing required field: {}", field),
            Self::InvalidField {
                field,
                value,
                reason,
            } => {
                write!(
                    f,
                    "Invalid field '{}' with value '{}': {}",
                    field, value, reason
                )
            }
            Self::RequestTooLarge => write!(f, "Request body is too large"),
            Self::InvalidJson(msg) => write!(f, "Invalid JSON: {}", msg),
        }
    }
}

// Conversion implementations for error types
impl From<AgentError> for RuntimeError {
    fn from(err: AgentError) -> Self {
        Self::Agent(err)
    }
}

impl From<AuthError> for RuntimeError {
    fn from(err: AuthError) -> Self {
        Self::Auth(err)
    }
}

impl From<RateLimitError> for RuntimeError {
    fn from(err: RateLimitError) -> Self {
        Self::RateLimit(err)
    }
}

impl From<AgentIdError> for RuntimeError {
    fn from(err: AgentIdError) -> Self {
        Self::Agent(AgentError::InvalidId(err))
    }
}

impl From<AgentExecutionError> for RuntimeError {
    fn from(err: AgentExecutionError) -> Self {
        Self::Agent(AgentError::ExecutionFailed(err))
    }
}

impl From<AuthTokenError> for RuntimeError {
    fn from(err: AuthTokenError) -> Self {
        Self::Auth(AuthError::InvalidToken(err))
    }
}

// Axum response implementation
impl IntoResponse for RuntimeError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        // Note: Request ID is now added by request_id_middleware to response headers
        // The middleware ensures X-Request-ID header is present on all responses
        let response = self.to_response(None);
        (status, Json(response)).into_response()
    }
}

/// Result type for HTTP runtime operations
pub type RuntimeResult<T> = Result<T, RuntimeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_not_found_error() {
        let error = RuntimeError::Agent(AgentError::NotFound("test-agent".to_string()));
        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(error.error_code(), "agent_not_found");
    }

    #[test]
    fn test_auth_forbidden_error() {
        let error = RuntimeError::Auth(AuthError::Forbidden {
            required_permissions: vec!["write".to_string()],
            user_permissions: vec!["read".to_string()],
        });
        assert_eq!(error.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(error.error_code(), "insufficient_permissions");
    }

    #[test]
    fn test_error_response_serialization() {
        let error = RuntimeError::Validation(ValidationError::MissingField("name".to_string()));
        let response = error.to_response(Some("req-123".to_string()));

        assert_eq!(response.code, "missing_field");
        assert!(response.message.contains("name"));
        assert_eq!(response.request_id, Some("req-123".to_string()));
    }
}
