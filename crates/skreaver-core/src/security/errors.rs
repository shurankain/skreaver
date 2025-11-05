//! Security error types and handling

use crate::identifiers::{AgentId, ToolId};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

/// Security-related errors
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum SecurityError {
    #[error("Access denied: {reason}")]
    AccessDenied { reason: String },

    #[error("Path not allowed: {path}")]
    PathNotAllowed { path: String },

    #[error("Invalid path: {path}")]
    InvalidPath { path: String },

    #[error("Path denied by policy: {path}")]
    PathDenied { path: String },

    #[error("Domain not allowed: {domain}")]
    DomainNotAllowed { domain: String },

    #[error("HTTP method not allowed: {method}")]
    MethodNotAllowed { method: String },

    #[error("Resource limit exceeded: {limit_type}")]
    ResourceLimitExceeded { limit_type: String },

    #[error("Memory limit exceeded: requested {requested}MB, limit {limit}MB")]
    MemoryLimitExceeded { requested: u64, limit: u64 },

    #[error("CPU limit exceeded: {usage}% > {limit}%")]
    CpuLimitExceeded { usage: f64, limit: f64 },

    #[error("Timeout exceeded: operation took longer than {timeout_ms}ms")]
    TimeoutExceeded { timeout_ms: u64 },

    #[error("File size limit exceeded: {size} bytes > {limit} bytes")]
    FileSizeLimitExceeded { size: u64, limit: u64 },

    #[error("Too many concurrent operations: {count} > {limit}")]
    ConcurrencyLimitExceeded { count: u32, limit: u32 },

    #[error("Input validation failed: {reason}")]
    ValidationFailed { reason: String },

    #[error("Suspicious activity detected: {description}")]
    SuspiciousActivity { description: String },

    #[error("Secret detected in input")]
    SecretInInput,

    #[error("Network access denied: {target}")]
    NetworkAccessDenied { target: String },

    #[error("Tool disabled by security policy: {tool_name}")]
    ToolDisabled { tool_name: String },

    #[error("Authentication required")]
    AuthenticationRequired,

    #[error("Authorization failed: insufficient permissions")]
    AuthorizationFailed,

    #[error("Rate limit exceeded: {requests} requests in {window_seconds}s")]
    RateLimitExceeded { requests: u32, window_seconds: u32 },

    #[error("Security configuration error: {message}")]
    ConfigError { message: String },

    #[error("Emergency lockdown active")]
    EmergencyLockdown,
}

/// Security violation details for audit logging
///
/// **Security**: As of v0.6.0, uses validated `AgentId` and `ToolId` types
/// to prevent path traversal and injection attacks in audit logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityViolation {
    pub violation_type: String,
    pub severity: ViolationSeverity,
    pub description: String,
    pub agent_id: AgentId,
    pub tool_name: ToolId,
    pub input_hash: Option<String>,
    pub timestamp: OffsetDateTime,
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl From<SecurityError> for SecurityViolation {
    fn from(error: SecurityError) -> Self {
        let (violation_type, severity, description) = match &error {
            SecurityError::AccessDenied { reason } => (
                "access_denied".to_string(),
                ViolationSeverity::High,
                reason.clone(),
            ),
            SecurityError::PathNotAllowed { path } => (
                "path_traversal".to_string(),
                ViolationSeverity::High,
                format!("Attempted access to restricted path: {}", path),
            ),
            SecurityError::DomainNotAllowed { domain } => (
                "ssrf_attempt".to_string(),
                ViolationSeverity::Critical,
                format!("Attempted access to restricted domain: {}", domain),
            ),
            SecurityError::ResourceLimitExceeded { limit_type } => (
                "resource_abuse".to_string(),
                ViolationSeverity::Medium,
                format!("Resource limit exceeded: {}", limit_type),
            ),
            SecurityError::SuspiciousActivity { description } => (
                "suspicious_activity".to_string(),
                ViolationSeverity::High,
                description.clone(),
            ),
            SecurityError::SecretInInput => (
                "secret_exposure".to_string(),
                ViolationSeverity::Critical,
                "Secret data detected in input".to_string(),
            ),
            _ => (
                "security_violation".to_string(),
                ViolationSeverity::Medium,
                error.to_string(),
            ),
        };

        Self {
            violation_type,
            severity,
            description,
            agent_id: AgentId::new_unchecked("unknown"),  // Will be set by caller
            tool_name: ToolId::new_unchecked("unknown"), // Will be set by caller
            input_hash: None,
            timestamp: OffsetDateTime::now_utc(),
            remediation: None,
        }
    }
}

impl SecurityViolation {
    /// Set the security context (agent and tool) for this violation
    ///
    /// # Security
    ///
    /// As of v0.6.0, requires validated `AgentId` and `ToolId` types,
    /// preventing path traversal and injection attacks in audit logs.
    pub fn with_context(mut self, agent_id: AgentId, tool_name: ToolId) -> Self {
        self.agent_id = agent_id;
        self.tool_name = tool_name;
        self
    }

    pub fn with_input_hash(mut self, input_hash: String) -> Self {
        self.input_hash = Some(input_hash);
        self
    }

    pub fn with_remediation(mut self, remediation: String) -> Self {
        self.remediation = Some(remediation);
        self
    }
}
