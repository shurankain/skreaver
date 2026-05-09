//! Error types for guardrail violations.

use skreaver_agent::AgentError;
use thiserror::Error;

/// Errors produced by guardrail checks.
#[derive(Debug, Error)]
pub enum GuardrailError {
    /// Tool is not in the allowlist.
    #[error("Tool not allowed: {tool_name}")]
    ToolNotAllowed { tool_name: String },

    /// Tool is explicitly in the denylist.
    #[error("Tool denied: {tool_name}")]
    ToolDenied { tool_name: String },

    /// Message was rejected by a guardrail check.
    #[error("Message rejected: {reason}")]
    MessageRejected { reason: String },

    /// Underlying agent error.
    #[error(transparent)]
    Agent(#[from] AgentError),
}

impl GuardrailError {
    /// Convert to `AgentError` for use at the `UnifiedAgent` trait boundary.
    pub fn into_agent_error(self) -> AgentError {
        match self {
            GuardrailError::Agent(e) => e,
            other => AgentError::InvalidRequest(other.to_string()),
        }
    }
}

/// Result type alias for guardrail operations.
pub type GuardrailResult<T> = Result<T, GuardrailError>;
