//! Unified error handling system for HTTP runtime
//!
//! This module provides comprehensive error handling with proper HTTP status code mapping,
//! request tracing, and structured error responses.

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

// Re-export unified RequestId from skreaver-core
pub use skreaver_core::RequestId;

/// Extension for storing RequestId in Axum request extensions
///
/// This allows request handlers and error responses to access the request ID
/// that was generated or extracted by the middleware.
#[derive(Debug, Clone)]
pub struct RequestIdExtension(pub RequestId);

/// MEDIUM-41: Maximum length for client-provided request IDs
const MAX_REQUEST_ID_LENGTH: usize = 128;

/// MEDIUM-41: Validate client-provided request IDs
///
/// Valid request IDs must:
/// - Be non-empty and <= MAX_REQUEST_ID_LENGTH characters
/// - Contain only alphanumeric characters, hyphens, and underscores
/// - Not contain control characters (prevents log injection)
///
/// # Security Note (HIGH-2)
///
/// Colons are explicitly NOT allowed because they are commonly used as field
/// separators in structured logging (e.g., `key:value`). Allowing colons in
/// request IDs could enable log injection attacks where attackers craft
/// request IDs that look like log fields.
///
/// UUID format (8-4-4-4-12 hex with hyphens) is preferred but not required.
fn validate_request_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_REQUEST_ID_LENGTH
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Middleware that generates or extracts request IDs for distributed tracing
///
/// This middleware:
/// - Tries to extract request ID from the `X-Request-ID` header
/// - Generates a new UUID if no header is present
/// - Stores the ID in request extensions for handlers and error responses
/// - Adds the ID to the response `X-Request-ID` header
///
/// # Example
///
/// ```rust,ignore
/// use axum::{Router, routing::get, middleware};
/// use skreaver_http::runtime::error::request_id_middleware;
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(middleware::from_fn(request_id_middleware));
/// ```
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Try to extract request ID from X-Request-ID header
    // MEDIUM-41: Validate client-provided request IDs to prevent:
    // - Log injection attacks via control characters
    // - Correlation confusion via invalid formats
    // - Storage key collisions via oversized IDs
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| validate_request_id(s))
        .map(|s| RequestId::new_unchecked(s.to_string()))
        .unwrap_or_else(RequestId::generate);

    // Store in extensions for handlers and error responses
    request
        .extensions_mut()
        .insert(RequestIdExtension(request_id.clone()));

    // Process request
    let mut response = next.run(request).await;

    // Add request ID to response header
    if let Ok(header_value) = HeaderValue::from_str(request_id.as_str()) {
        response.headers_mut().insert(
            header::HeaderName::from_static("x-request-id"),
            header_value,
        );
    }

    response
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
        // Use ok() instead of unwrap_or_default to avoid silently discarding valid data
        self.details = serde_json::to_value(details).ok();
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
    ///
    /// SECURITY: This method sanitizes error details to prevent information disclosure.
    /// Internal details like stack traces, file paths, internal IDs, and user-provided
    /// values are NOT exposed in API responses. Full details are logged server-side
    /// with the request_id for debugging.
    pub fn to_error_response(&self) -> ErrorResponse {
        // SECURITY: Use generic user-facing messages instead of internal error details
        let user_message = self.user_facing_message();
        let mut response =
            ErrorResponse::new(self.error_code(), &user_message, self.request_id().clone());

        // SECURITY: Only add safe, non-sensitive details to client responses
        // Internal details are logged server-side with request_id for debugging
        match self {
            // Agent errors: Don't expose internal agent IDs or operation details
            RuntimeError::AgentNotFound { .. } => {
                // No additional details - agent_id could leak internal structure
            }
            RuntimeError::AgentCreationFailed {
                agent_type: Some(t),
                ..
            } => {
                // Only expose agent_type if it's a user-provided value, not internal reason
                response = response.with_details(serde_json::json!({
                    "agent_type": t
                }));
            }
            RuntimeError::AgentCreationFailed {
                agent_type: None, ..
            } => {
                // No additional details if agent_type is not provided
            }
            RuntimeError::AgentOperationFailed { .. } => {
                // SECURITY: Don't expose agent_id, operation, or reason - may contain
                // stack traces, file paths, or other sensitive internal details
            }

            // Auth errors: Minimal info to prevent enumeration attacks
            RuntimeError::InvalidAuthentication {
                auth_method: Some(method),
                ..
            } => {
                // Don't expose the specific reason - could help attackers
                response = response.with_details(serde_json::json!({
                    "auth_method": method
                }));
            }
            RuntimeError::InvalidAuthentication {
                auth_method: None, ..
            } => {
                // No additional details if auth_method is not provided
            }
            RuntimeError::InsufficientPermissions { required, .. } => {
                // Only show required permissions, NOT what user provided
                // (provided permissions could leak user's role structure)
                response = response.with_details(serde_json::json!({
                    "required_permissions": required
                }));
            }

            // Rate limiting: Safe to expose limits (helps clients implement backoff)
            RuntimeError::RateLimitExceeded {
                limit_type,
                retry_after,
                ..
            } => {
                // Don't expose current_usage or exact limit - could help DoS planning
                response = response.with_details(serde_json::json!({
                    "limit_type": limit_type,
                    "retry_after_seconds": retry_after
                }));
            }

            // Input validation: Only expose field name, not the invalid value
            RuntimeError::InvalidInput { field, reason, .. } => {
                // SECURITY: Never expose provided_value - may contain sensitive user data
                // like passwords, API keys, or PII that shouldn't be echoed back
                response = response.with_details(serde_json::json!({
                    "field": field,
                    "reason": reason
                }));
            }
            RuntimeError::MissingRequiredField { field, .. } => {
                response = response.with_details(serde_json::json!({
                    "field": field
                }));
            }

            // Timeouts: Only generic info
            RuntimeError::Timeout { .. } => {
                // Don't expose internal operation names or exact durations
            }

            // All other errors: No additional details to prevent information disclosure
            _ => {}
        }

        response
    }

    /// Get a sanitized user-facing message that doesn't expose internal details
    ///
    /// SECURITY: These messages are safe to show to end users and don't leak
    /// internal implementation details, file paths, or stack traces.
    fn user_facing_message(&self) -> String {
        match self {
            RuntimeError::AgentNotFound { .. } => "The requested agent was not found.".to_string(),
            RuntimeError::AgentCreationFailed { .. } => {
                "Failed to create the agent. Please check your configuration.".to_string()
            }
            RuntimeError::AgentOperationFailed { .. } => {
                "The agent operation could not be completed.".to_string()
            }
            RuntimeError::AuthenticationRequired { .. } => {
                "Authentication is required to access this resource.".to_string()
            }
            RuntimeError::InvalidAuthentication { .. } => {
                "The provided authentication credentials are invalid.".to_string()
            }
            RuntimeError::InsufficientPermissions { .. } => {
                "You don't have permission to perform this action.".to_string()
            }
            RuntimeError::TokenCreationFailed { .. } => {
                "Failed to create authentication token.".to_string()
            }
            RuntimeError::RateLimitExceeded { .. } => {
                "Rate limit exceeded. Please try again later.".to_string()
            }
            RuntimeError::InvalidInput { field, .. } => {
                format!("Invalid value provided for field '{}'.", field)
            }
            RuntimeError::MissingRequiredField { field, .. } => {
                format!("Required field '{}' is missing.", field)
            }
            RuntimeError::InvalidJson { .. } => "Invalid JSON in request body.".to_string(),
            RuntimeError::InternalError { .. } => {
                "An internal error occurred. Please try again later.".to_string()
            }
            RuntimeError::ServiceUnavailable { .. } => {
                "Service is temporarily unavailable. Please try again later.".to_string()
            }
            RuntimeError::Timeout { .. } => "The request timed out. Please try again.".to_string(),
            RuntimeError::MemoryError { .. } => {
                "A storage error occurred. Please try again later.".to_string()
            }
            RuntimeError::ToolExecutionFailed { .. } => {
                "Tool execution failed. Please check your request.".to_string()
            }
            RuntimeError::ConfigurationError { .. } => {
                "A configuration error occurred.".to_string()
            }
        }
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
            // Only insert header if parsing succeeds
            if let Ok(header_value) = retry_after.to_string().parse() {
                response.headers_mut().insert("Retry-After", header_value);
            } else {
                tracing::warn!(retry_after, "Failed to parse Retry-After header value");
            }
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

impl<T> IntoRuntimeError<T> for Result<T, skreaver_core::SkreverError> {
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
        let id1 = RequestId::generate();
        let id2 = RequestId::generate();

        assert_ne!(id1, id2);
        assert!(!id1.as_str().is_empty());
    }

    #[test]
    fn test_error_response_creation() {
        let request_id = RequestId::generate();
        let error = RuntimeError::AgentNotFound {
            agent_id: "test-agent".to_string(),
            request_id: request_id.clone(),
        };

        let response = error.to_error_response();

        assert_eq!(response.error, "agent_not_found");
        assert_eq!(response.request_id, request_id);
        // SECURITY: AgentNotFound no longer includes agent_id in details
        // to prevent information disclosure
        assert!(response.details.is_none());
        assert_eq!(response.message, "The requested agent was not found.");
    }

    #[test]
    fn test_error_response_sanitization() {
        let request_id = RequestId::generate();

        // Test that sensitive data is NOT exposed in error responses
        let error = RuntimeError::InvalidInput {
            field: "password".to_string(),
            reason: "Must be at least 8 characters".to_string(),
            provided_value: Some("secret123".to_string()), // SENSITIVE!
            request_id: request_id.clone(),
        };
        let response = error.to_error_response();

        // Verify provided_value is NOT in the response
        if let Some(details) = &response.details {
            assert!(
                details.get("provided_value").is_none(),
                "SECURITY: provided_value should not be exposed in error response"
            );
        }

        // Test that internal operation details are NOT exposed
        let error = RuntimeError::AgentOperationFailed {
            agent_id: "internal-agent-12345".to_string(),
            operation: "load_from_file(/etc/passwd)".to_string(),
            reason: "Stack trace:\n  at main.rs:42\n  at lib.rs:100".to_string(),
            request_id: request_id.clone(),
        };
        let response = error.to_error_response();

        // Verify no internal details are exposed
        assert!(
            response.details.is_none(),
            "SECURITY: AgentOperationFailed should not expose internal details"
        );
        assert!(
            !response.message.contains("internal-agent"),
            "SECURITY: agent_id should not be in message"
        );
        assert!(
            !response.message.contains("/etc/passwd"),
            "SECURITY: operation details should not be in message"
        );
        assert!(
            !response.message.contains("Stack trace"),
            "SECURITY: stack traces should not be in message"
        );
    }

    #[test]
    fn test_status_code_mapping() {
        let request_id = RequestId::generate();

        let error = RuntimeError::AgentNotFound {
            agent_id: "test".to_string(),
            request_id: request_id.clone(),
        };
        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);

        let error = RuntimeError::AuthenticationRequired {
            request_id: request_id.clone(),
        };
        assert_eq!(error.status_code(), StatusCode::UNAUTHORIZED);

        let error = RuntimeError::RateLimitExceeded {
            limit_type: "global".to_string(),
            retry_after: 60,
            current_usage: 100,
            limit: 100,
            request_id,
        };
        assert_eq!(error.status_code(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_error_code() {
        let request_id = RequestId::generate();

        let error = RuntimeError::AgentNotFound {
            agent_id: "test".to_string(),
            request_id,
        };
        assert_eq!(error.error_code(), "agent_not_found");
    }
}
