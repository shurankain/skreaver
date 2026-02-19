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

/// Type-safe error codes for runtime errors
///
/// This enum provides compile-time guarantees for error codes, preventing
/// typos and enabling exhaustive pattern matching in client code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Agent not found
    AgentNotFound,
    /// Agent creation failed
    AgentCreationFailed,
    /// Agent operation failed
    AgentOperationFailed,
    /// Authentication required
    AuthenticationRequired,
    /// Invalid authentication credentials
    InvalidAuthentication,
    /// Insufficient permissions
    InsufficientPermissions,
    /// Token creation failed
    TokenCreationFailed,
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Invalid input
    InvalidInput,
    /// Missing required field
    MissingRequiredField,
    /// Invalid JSON
    InvalidJson,
    /// Internal server error
    InternalError,
    /// Service unavailable
    ServiceUnavailable,
    /// Request timeout
    Timeout,
    /// Memory/storage error
    MemoryError,
    /// Tool execution failed
    ToolExecutionFailed,
    /// Configuration error
    ConfigurationError,
}

impl ErrorCode {
    /// Get the string representation of this error code
    ///
    /// This is used for serialization in error responses and logging.
    /// The format matches the original snake_case convention.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentNotFound => "agent_not_found",
            Self::AgentCreationFailed => "agent_creation_failed",
            Self::AgentOperationFailed => "agent_operation_failed",
            Self::AuthenticationRequired => "authentication_required",
            Self::InvalidAuthentication => "invalid_authentication",
            Self::InsufficientPermissions => "insufficient_permissions",
            Self::TokenCreationFailed => "token_creation_failed",
            Self::RateLimitExceeded => "rate_limit_exceeded",
            Self::InvalidInput => "invalid_input",
            Self::MissingRequiredField => "missing_required_field",
            Self::InvalidJson => "invalid_json",
            Self::InternalError => "internal_error",
            Self::ServiceUnavailable => "service_unavailable",
            Self::Timeout => "timeout",
            Self::MemoryError => "memory_error",
            Self::ToolExecutionFailed => "tool_execution_failed",
            Self::ConfigurationError => "configuration_error",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
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
    /// Timestamp when error occurred (LOW-1: Optional to avoid overhead for internal errors)
    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "ErrorResponse::default_timestamp"
    )]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    /// Additional metadata for debugging
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl ErrorResponse {
    /// Provides current timestamp for deserialization when missing
    fn default_timestamp() -> Option<chrono::DateTime<chrono::Utc>> {
        Some(chrono::Utc::now())
    }

    /// Create a new error response without timestamp (LOW-1: optimized for internal errors)
    ///
    /// Use this for internal errors that may not be sent to clients. The timestamp
    /// will only be created if/when the error is serialized for a response.
    pub fn new(error: &str, message: &str, request_id: RequestId) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            details: None,
            request_id,
            timestamp: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new error response with immediate timestamp
    ///
    /// Use this when you need an immediate timestamp (e.g., for client-facing errors).
    pub fn new_with_timestamp(error: &str, message: &str, request_id: RequestId) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            details: None,
            request_id,
            timestamp: Some(chrono::Utc::now()),
            metadata: HashMap::new(),
        }
    }

    /// Ensure timestamp is set, creating it if needed
    pub fn ensure_timestamp(&mut self) {
        if self.timestamp.is_none() {
            self.timestamp = Some(chrono::Utc::now());
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

/// Specific error information for different error categories
#[derive(Debug, Clone)]
pub enum RuntimeErrorKind {
    // Agent-related errors
    /// Agent not found
    AgentNotFound { agent_id: String },

    /// Agent creation failed
    AgentCreationFailed {
        reason: String,
        agent_type: Option<String>,
    },

    /// Agent operation failed
    AgentOperationFailed {
        agent_id: String,
        operation: String,
        reason: String,
    },

    // Authentication errors
    /// Authentication required
    AuthenticationRequired,

    /// Invalid authentication credentials
    InvalidAuthentication {
        reason: String,
        auth_method: Option<String>,
    },

    /// Insufficient permissions
    InsufficientPermissions {
        required: Vec<String>,
        provided: Vec<String>,
    },

    /// Token creation failed
    TokenCreationFailed { reason: String },

    // Rate limiting errors
    /// Rate limit exceeded
    RateLimitExceeded {
        limit_type: String,
        retry_after: u64,
        current_usage: u32,
        limit: u32,
    },

    // Input validation errors
    /// Invalid input field
    InvalidInput {
        field: String,
        reason: String,
        provided_value: Option<String>,
    },

    /// Missing required field
    MissingRequiredField { field: String },

    /// Invalid JSON
    InvalidJson { reason: String },

    // System errors
    /// Internal server error
    InternalError { reason: String },

    /// Service unavailable
    ServiceUnavailable { service: String },

    /// Timeout occurred
    Timeout { operation: String, duration_ms: u64 },

    // Memory/Storage errors
    /// Memory operation failed
    MemoryError { operation: String, reason: String },

    // Tool execution errors
    /// Tool execution failed
    ToolExecutionFailed { tool_name: String, reason: String },

    // Configuration errors
    /// Configuration error
    ConfigurationError { setting: String, reason: String },
}

/// Runtime error types with proper categorization
///
/// This struct uses a wrapper pattern with common fields (request_id) extracted
/// to the top level, avoiding repetitive match statements for accessing common data.
#[derive(Debug)]
pub struct RuntimeError {
    /// Request ID for tracking and correlation
    pub request_id: RequestId,
    /// Specific error information
    pub kind: RuntimeErrorKind,
}

impl RuntimeError {
    /// Create a new runtime error with the given kind and request ID
    pub fn new(kind: RuntimeErrorKind, request_id: RequestId) -> Self {
        Self { request_id, kind }
    }

    // ==================== Constructor helpers ====================

    /// Create an AgentNotFound error
    pub fn agent_not_found(agent_id: impl Into<String>, request_id: RequestId) -> Self {
        Self::new(
            RuntimeErrorKind::AgentNotFound {
                agent_id: agent_id.into(),
            },
            request_id,
        )
    }

    /// Create an AgentCreationFailed error
    pub fn agent_creation_failed(
        reason: impl Into<String>,
        agent_type: Option<String>,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::AgentCreationFailed {
                reason: reason.into(),
                agent_type,
            },
            request_id,
        )
    }

    /// Create an AgentOperationFailed error
    pub fn agent_operation_failed(
        agent_id: impl Into<String>,
        operation: impl Into<String>,
        reason: impl Into<String>,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::AgentOperationFailed {
                agent_id: agent_id.into(),
                operation: operation.into(),
                reason: reason.into(),
            },
            request_id,
        )
    }

    /// Create an AuthenticationRequired error
    pub fn authentication_required(request_id: RequestId) -> Self {
        Self::new(RuntimeErrorKind::AuthenticationRequired, request_id)
    }

    /// Create an InvalidAuthentication error
    pub fn invalid_authentication(
        reason: impl Into<String>,
        auth_method: Option<String>,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::InvalidAuthentication {
                reason: reason.into(),
                auth_method,
            },
            request_id,
        )
    }

    /// Create an InsufficientPermissions error
    pub fn insufficient_permissions(
        required: Vec<String>,
        provided: Vec<String>,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::InsufficientPermissions { required, provided },
            request_id,
        )
    }

    /// Create a TokenCreationFailed error
    pub fn token_creation_failed(reason: impl Into<String>, request_id: RequestId) -> Self {
        Self::new(
            RuntimeErrorKind::TokenCreationFailed {
                reason: reason.into(),
            },
            request_id,
        )
    }

    /// Create a RateLimitExceeded error
    pub fn rate_limit_exceeded(
        limit_type: impl Into<String>,
        retry_after: u64,
        current_usage: u32,
        limit: u32,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::RateLimitExceeded {
                limit_type: limit_type.into(),
                retry_after,
                current_usage,
                limit,
            },
            request_id,
        )
    }

    /// Create an InvalidInput error
    pub fn invalid_input(
        field: impl Into<String>,
        reason: impl Into<String>,
        provided_value: Option<String>,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::InvalidInput {
                field: field.into(),
                reason: reason.into(),
                provided_value,
            },
            request_id,
        )
    }

    /// Create a MissingRequiredField error
    pub fn missing_required_field(field: impl Into<String>, request_id: RequestId) -> Self {
        Self::new(
            RuntimeErrorKind::MissingRequiredField {
                field: field.into(),
            },
            request_id,
        )
    }

    /// Create an InvalidJson error
    pub fn invalid_json(reason: impl Into<String>, request_id: RequestId) -> Self {
        Self::new(
            RuntimeErrorKind::InvalidJson {
                reason: reason.into(),
            },
            request_id,
        )
    }

    /// Create an InternalError
    pub fn internal_error(reason: impl Into<String>, request_id: RequestId) -> Self {
        Self::new(
            RuntimeErrorKind::InternalError {
                reason: reason.into(),
            },
            request_id,
        )
    }

    /// Create a ServiceUnavailable error
    pub fn service_unavailable(service: impl Into<String>, request_id: RequestId) -> Self {
        Self::new(
            RuntimeErrorKind::ServiceUnavailable {
                service: service.into(),
            },
            request_id,
        )
    }

    /// Create a Timeout error
    pub fn timeout(operation: impl Into<String>, duration_ms: u64, request_id: RequestId) -> Self {
        Self::new(
            RuntimeErrorKind::Timeout {
                operation: operation.into(),
                duration_ms,
            },
            request_id,
        )
    }

    /// Create a MemoryError
    pub fn memory_error(
        operation: impl Into<String>,
        reason: impl Into<String>,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::MemoryError {
                operation: operation.into(),
                reason: reason.into(),
            },
            request_id,
        )
    }

    /// Create a ToolExecutionFailed error
    pub fn tool_execution_failed(
        tool_name: impl Into<String>,
        reason: impl Into<String>,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::ToolExecutionFailed {
                tool_name: tool_name.into(),
                reason: reason.into(),
            },
            request_id,
        )
    }

    /// Create a ConfigurationError
    pub fn configuration_error(
        setting: impl Into<String>,
        reason: impl Into<String>,
        request_id: RequestId,
    ) -> Self {
        Self::new(
            RuntimeErrorKind::ConfigurationError {
                setting: setting.into(),
                reason: reason.into(),
            },
            request_id,
        )
    }
}

// Additional methods on RuntimeError
impl RuntimeError {
    /// Get the request ID associated with this error (direct field access!)
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Get the error kind
    pub fn kind(&self) -> &RuntimeErrorKind {
        &self.kind
    }

    /// Get the appropriate HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match &self.kind {
            RuntimeErrorKind::AgentNotFound { .. } => StatusCode::NOT_FOUND,
            RuntimeErrorKind::AgentCreationFailed { .. } => StatusCode::BAD_REQUEST,
            RuntimeErrorKind::AgentOperationFailed { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            RuntimeErrorKind::AuthenticationRequired => StatusCode::UNAUTHORIZED,
            RuntimeErrorKind::InvalidAuthentication { .. } => StatusCode::UNAUTHORIZED,
            RuntimeErrorKind::InsufficientPermissions { .. } => StatusCode::FORBIDDEN,
            RuntimeErrorKind::TokenCreationFailed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            RuntimeErrorKind::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
            RuntimeErrorKind::InvalidInput { .. } => StatusCode::BAD_REQUEST,
            RuntimeErrorKind::MissingRequiredField { .. } => StatusCode::BAD_REQUEST,
            RuntimeErrorKind::InvalidJson { .. } => StatusCode::BAD_REQUEST,
            RuntimeErrorKind::InternalError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            RuntimeErrorKind::ServiceUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
            RuntimeErrorKind::Timeout { .. } => StatusCode::REQUEST_TIMEOUT,
            RuntimeErrorKind::MemoryError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            RuntimeErrorKind::ToolExecutionFailed { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            RuntimeErrorKind::ConfigurationError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Get the error code for this error type
    ///
    /// Returns a type-safe `ErrorCode` enum instead of a string, providing
    /// compile-time guarantees and enabling pattern matching in client code.
    pub fn error_code(&self) -> ErrorCode {
        match &self.kind {
            RuntimeErrorKind::AgentNotFound { .. } => ErrorCode::AgentNotFound,
            RuntimeErrorKind::AgentCreationFailed { .. } => ErrorCode::AgentCreationFailed,
            RuntimeErrorKind::AgentOperationFailed { .. } => ErrorCode::AgentOperationFailed,
            RuntimeErrorKind::AuthenticationRequired => ErrorCode::AuthenticationRequired,
            RuntimeErrorKind::InvalidAuthentication { .. } => ErrorCode::InvalidAuthentication,
            RuntimeErrorKind::InsufficientPermissions { .. } => ErrorCode::InsufficientPermissions,
            RuntimeErrorKind::TokenCreationFailed { .. } => ErrorCode::TokenCreationFailed,
            RuntimeErrorKind::RateLimitExceeded { .. } => ErrorCode::RateLimitExceeded,
            RuntimeErrorKind::InvalidInput { .. } => ErrorCode::InvalidInput,
            RuntimeErrorKind::MissingRequiredField { .. } => ErrorCode::MissingRequiredField,
            RuntimeErrorKind::InvalidJson { .. } => ErrorCode::InvalidJson,
            RuntimeErrorKind::InternalError { .. } => ErrorCode::InternalError,
            RuntimeErrorKind::ServiceUnavailable { .. } => ErrorCode::ServiceUnavailable,
            RuntimeErrorKind::Timeout { .. } => ErrorCode::Timeout,
            RuntimeErrorKind::MemoryError { .. } => ErrorCode::MemoryError,
            RuntimeErrorKind::ToolExecutionFailed { .. } => ErrorCode::ToolExecutionFailed,
            RuntimeErrorKind::ConfigurationError { .. } => ErrorCode::ConfigurationError,
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
        // LOW-1: Use new_with_timestamp for client-facing errors
        let mut response = ErrorResponse::new_with_timestamp(
            self.error_code().as_str(),
            &user_message,
            self.request_id.clone(),
        );

        // SECURITY: Only add safe, non-sensitive details to client responses
        // Internal details are logged server-side with request_id for debugging
        match &self.kind {
            // Agent errors: Don't expose internal agent IDs or operation details
            RuntimeErrorKind::AgentNotFound { .. } => {
                // No additional details - agent_id could leak internal structure
            }
            RuntimeErrorKind::AgentCreationFailed {
                agent_type: Some(t),
                ..
            } => {
                // Only expose agent_type if it's a user-provided value, not internal reason
                response = response.with_details(serde_json::json!({
                    "agent_type": t
                }));
            }
            RuntimeErrorKind::AgentCreationFailed {
                agent_type: None, ..
            } => {
                // No additional details if agent_type is not provided
            }
            RuntimeErrorKind::AgentOperationFailed { .. } => {
                // SECURITY: Don't expose agent_id, operation, or reason - may contain
                // stack traces, file paths, or other sensitive internal details
            }

            // Auth errors: Minimal info to prevent enumeration attacks
            RuntimeErrorKind::InvalidAuthentication {
                auth_method: Some(method),
                ..
            } => {
                // Don't expose the specific reason - could help attackers
                response = response.with_details(serde_json::json!({
                    "auth_method": method
                }));
            }
            RuntimeErrorKind::InvalidAuthentication {
                auth_method: None, ..
            } => {
                // No additional details if auth_method is not provided
            }
            RuntimeErrorKind::InsufficientPermissions { required, .. } => {
                // Only show required permissions, NOT what user provided
                // (provided permissions could leak user's role structure)
                response = response.with_details(serde_json::json!({
                    "required_permissions": required
                }));
            }

            // Rate limiting: Safe to expose limits (helps clients implement backoff)
            RuntimeErrorKind::RateLimitExceeded {
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
            RuntimeErrorKind::InvalidInput { field, reason, .. } => {
                // SECURITY: Never expose provided_value - may contain sensitive user data
                // like passwords, API keys, or PII that shouldn't be echoed back
                response = response.with_details(serde_json::json!({
                    "field": field,
                    "reason": reason
                }));
            }
            RuntimeErrorKind::MissingRequiredField { field, .. } => {
                response = response.with_details(serde_json::json!({
                    "field": field
                }));
            }

            // Timeouts: Only generic info
            RuntimeErrorKind::Timeout { .. } => {
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
        match &self.kind {
            RuntimeErrorKind::AgentNotFound { .. } => {
                "The requested agent was not found.".to_string()
            }
            RuntimeErrorKind::AgentCreationFailed { .. } => {
                "Failed to create the agent. Please check your configuration.".to_string()
            }
            RuntimeErrorKind::AgentOperationFailed { .. } => {
                "The agent operation could not be completed.".to_string()
            }
            RuntimeErrorKind::AuthenticationRequired => {
                "Authentication is required to access this resource.".to_string()
            }
            RuntimeErrorKind::InvalidAuthentication { .. } => {
                "The provided authentication credentials are invalid.".to_string()
            }
            RuntimeErrorKind::InsufficientPermissions { .. } => {
                "You don't have permission to perform this action.".to_string()
            }
            RuntimeErrorKind::TokenCreationFailed { .. } => {
                "Failed to create authentication token.".to_string()
            }
            RuntimeErrorKind::RateLimitExceeded { .. } => {
                "Rate limit exceeded. Please try again later.".to_string()
            }
            RuntimeErrorKind::InvalidInput { field, .. } => {
                format!("Invalid value provided for field '{}'.", field)
            }
            RuntimeErrorKind::MissingRequiredField { field, .. } => {
                format!("Required field '{}' is missing.", field)
            }
            RuntimeErrorKind::InvalidJson { .. } => "Invalid JSON in request body.".to_string(),
            RuntimeErrorKind::InternalError { .. } => {
                "An internal error occurred. Please try again later.".to_string()
            }
            RuntimeErrorKind::ServiceUnavailable { .. } => {
                "Service is temporarily unavailable. Please try again later.".to_string()
            }
            RuntimeErrorKind::Timeout { .. } => {
                "The request timed out. Please try again.".to_string()
            }
            RuntimeErrorKind::MemoryError { .. } => {
                "A storage error occurred. Please try again later.".to_string()
            }
            RuntimeErrorKind::ToolExecutionFailed { .. } => {
                "Tool execution failed. Please check your request.".to_string()
            }
            RuntimeErrorKind::ConfigurationError { .. } => {
                "A configuration error occurred.".to_string()
            }
        }
    }
}

/// Implement Display for RuntimeError
impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            RuntimeErrorKind::AgentNotFound { agent_id } => {
                write!(f, "Agent not found: {}", agent_id)
            }
            RuntimeErrorKind::AgentCreationFailed { reason, .. } => {
                write!(f, "Agent creation failed: {}", reason)
            }
            RuntimeErrorKind::AgentOperationFailed {
                agent_id,
                operation,
                ..
            } => {
                write!(f, "Agent operation failed: {} on {}", operation, agent_id)
            }
            RuntimeErrorKind::AuthenticationRequired => {
                write!(f, "Authentication required")
            }
            RuntimeErrorKind::InvalidAuthentication { reason, .. } => {
                write!(f, "Invalid authentication: {}", reason)
            }
            RuntimeErrorKind::InsufficientPermissions { required, .. } => {
                write!(f, "Insufficient permissions: required {:?}", required)
            }
            RuntimeErrorKind::TokenCreationFailed { reason } => {
                write!(f, "Token creation failed: {}", reason)
            }
            RuntimeErrorKind::RateLimitExceeded { limit_type, .. } => {
                write!(f, "Rate limit exceeded: {}", limit_type)
            }
            RuntimeErrorKind::InvalidInput { field, .. } => {
                write!(f, "Invalid input: {}", field)
            }
            RuntimeErrorKind::MissingRequiredField { field } => {
                write!(f, "Missing required field: {}", field)
            }
            RuntimeErrorKind::InvalidJson { reason } => {
                write!(f, "Invalid JSON: {}", reason)
            }
            RuntimeErrorKind::InternalError { reason } => {
                write!(f, "Internal server error: {}", reason)
            }
            RuntimeErrorKind::ServiceUnavailable { service } => {
                write!(f, "Service unavailable: {}", service)
            }
            RuntimeErrorKind::Timeout { operation, .. } => {
                write!(f, "Timeout occurred: {}", operation)
            }
            RuntimeErrorKind::MemoryError { operation, .. } => {
                write!(f, "Memory operation failed: {}", operation)
            }
            RuntimeErrorKind::ToolExecutionFailed { tool_name, .. } => {
                write!(f, "Tool execution failed: {}", tool_name)
            }
            RuntimeErrorKind::ConfigurationError { setting, .. } => {
                write!(f, "Configuration error: {}", setting)
            }
        }
    }
}

/// Implement Error trait for RuntimeError
impl std::error::Error for RuntimeError {}

/// Implement IntoResponse for RuntimeError to integrate with Axum
impl IntoResponse for RuntimeError {
    fn into_response(self) -> Response {
        let status_code = self.status_code();
        let error_response = self.to_error_response();

        // Log the error for monitoring
        tracing::error!(
            error_code = %self.error_code(),
            request_id = %self.request_id(),
            status_code = %status_code,
            error_message = %self,
            "HTTP runtime error occurred"
        );

        // Create HTTP response
        let mut response = (status_code, Json(error_response)).into_response();

        // Add error-specific headers
        if let RuntimeErrorKind::RateLimitExceeded { retry_after, .. } = &self.kind {
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
        self.map_err(|e| RuntimeError::memory_error("memory_operation", e.to_string(), request_id))
    }
}

impl<T> IntoRuntimeError<T> for Result<T, serde_json::Error> {
    fn into_runtime_error(self, request_id: RequestId) -> RuntimeResult<T> {
        self.map_err(|e| RuntimeError::invalid_json(e.to_string(), request_id))
    }
}

impl<T> IntoRuntimeError<T> for Result<T, jsonwebtoken::errors::Error> {
    fn into_runtime_error(self, request_id: RequestId) -> RuntimeResult<T> {
        self.map_err(|e| RuntimeError::token_creation_failed(e.to_string(), request_id))
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
        let error = RuntimeError::agent_not_found("test-agent", request_id.clone());

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
        let error = RuntimeError::invalid_input(
            "password",
            "Must be at least 8 characters",
            Some("secret123".to_string()), // SENSITIVE!
            request_id.clone(),
        );
        let response = error.to_error_response();

        // Verify provided_value is NOT in the response
        if let Some(details) = &response.details {
            assert!(
                details.get("provided_value").is_none(),
                "SECURITY: provided_value should not be exposed in error response"
            );
        }

        // Test that internal operation details are NOT exposed
        let error = RuntimeError::agent_operation_failed(
            "internal-agent-12345",
            "load_from_file(/etc/passwd)",
            "Stack trace:\n  at main.rs:42\n  at lib.rs:100",
            request_id.clone(),
        );
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

        let error = RuntimeError::agent_not_found("test", request_id.clone());
        assert_eq!(error.status_code(), StatusCode::NOT_FOUND);

        let error = RuntimeError::authentication_required(request_id.clone());
        assert_eq!(error.status_code(), StatusCode::UNAUTHORIZED);

        let error = RuntimeError::rate_limit_exceeded("global", 60, 100, 100, request_id);
        assert_eq!(error.status_code(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_error_code() {
        let request_id = RequestId::generate();

        let error = RuntimeError::agent_not_found("test", request_id);
        assert_eq!(error.error_code(), ErrorCode::AgentNotFound);
        assert_eq!(error.error_code().as_str(), "agent_not_found");
    }

    #[test]
    fn test_direct_request_id_access() {
        // Test that request_id() provides direct field access (no match needed)
        let request_id = RequestId::generate();
        let error = RuntimeError::internal_error("test reason", request_id.clone());

        // Direct field access - no match statement needed!
        assert_eq!(error.request_id(), &request_id);
        assert_eq!(&error.request_id, &request_id);
    }

    #[test]
    fn test_error_kind_access() {
        let request_id = RequestId::generate();
        let error = RuntimeError::timeout("fetch_data", 5000, request_id);

        // Can access the kind directly for pattern matching when needed
        match error.kind() {
            RuntimeErrorKind::Timeout {
                operation,
                duration_ms,
            } => {
                assert_eq!(operation, "fetch_data");
                assert_eq!(*duration_ms, 5000);
            }
            _ => panic!("Expected Timeout error kind"),
        }
    }

    #[test]
    fn test_error_code_serialization() {
        // Test that ErrorCode serializes to the expected string format
        let code = ErrorCode::AgentNotFound;
        let serialized = serde_json::to_string(&code).unwrap();
        assert_eq!(serialized, r#""agent_not_found""#);

        // Test deserialization
        let deserialized: ErrorCode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, ErrorCode::AgentNotFound);
    }

    #[test]
    fn test_error_code_display() {
        let code = ErrorCode::RateLimitExceeded;
        assert_eq!(format!("{}", code), "rate_limit_exceeded");
        assert_eq!(code.to_string(), "rate_limit_exceeded");
    }
}
