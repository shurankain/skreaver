//! Error type conversions and From trait implementations.
//!
//! This module provides automatic conversions between different error types,
//! enabling ergonomic error propagation with the ? operator throughout the
//! codebase.

use super::agent::{AgentError, CoordinatorError};
use super::memory::{MemoryError, TransactionError};
use super::tool::ToolError;
use super::types::{InputValidationError, MemoryBackend, MemoryErrorKind, ValidatedInput};
use crate::tool::ToolDispatch;

/// Main error type for Skreaver operations - defined here to avoid circular dependencies
/// with conversions.
#[derive(Debug, Clone)]
pub enum SkreverError {
    /// Tool-related errors during execution or dispatch.
    Tool(ToolError),

    /// Memory-related errors during storage or retrieval operations.
    Memory(MemoryError),

    /// Agent-related errors during lifecycle operations.
    Agent(AgentError),

    /// Coordinator-related errors during orchestration.
    Coordinator(CoordinatorError),
}

impl std::fmt::Display for SkreverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkreverError::Tool(e) => write!(f, "Tool error: {}", e),
            SkreverError::Memory(e) => write!(f, "Memory error: {}", e),
            SkreverError::Agent(e) => write!(f, "Agent error: {}", e),
            SkreverError::Coordinator(e) => write!(f, "Coordinator error: {}", e),
        }
    }
}

impl std::error::Error for SkreverError {}

// Conversions to SkreverError
impl From<ToolError> for SkreverError {
    fn from(err: ToolError) -> Self {
        SkreverError::Tool(err)
    }
}

impl From<MemoryError> for SkreverError {
    fn from(err: MemoryError) -> Self {
        SkreverError::Memory(err)
    }
}

impl From<AgentError> for SkreverError {
    fn from(err: AgentError) -> Self {
        SkreverError::Agent(err)
    }
}

impl From<CoordinatorError> for SkreverError {
    fn from(err: CoordinatorError) -> Self {
        SkreverError::Coordinator(err)
    }
}

// Conversions to TransactionError
impl From<MemoryError> for TransactionError {
    fn from(err: MemoryError) -> Self {
        TransactionError::MemoryError(err)
    }
}

impl From<crate::memory::InvalidMemoryKey> for TransactionError {
    fn from(err: crate::memory::InvalidMemoryKey) -> Self {
        let fallback_key = crate::memory::MemoryKey::new("fallback").expect("fallback is valid");
        TransactionError::MemoryError(MemoryError::StoreFailed {
            key: fallback_key,
            backend: MemoryBackend::InMemory,
            kind: MemoryErrorKind::InvalidKey {
                validation_error: err.to_string(),
            },
        })
    }
}

// Conversions to ToolError
impl From<crate::validation::ValidationError> for ToolError {
    fn from(err: crate::validation::ValidationError) -> Self {
        // Convert to IdValidationError for backward compatibility
        #[allow(deprecated)]
        let legacy_err: crate::IdValidationError = err.into();
        ToolError::InvalidToolId {
            attempted_name: "unknown".to_string(),
            validation_error: legacy_err,
        }
    }
}

#[allow(deprecated)]
impl From<crate::IdValidationError> for ToolError {
    fn from(err: crate::IdValidationError) -> Self {
        ToolError::InvalidToolId {
            attempted_name: "unknown".to_string(),
            validation_error: err,
        }
    }
}

impl From<InputValidationError> for ToolError {
    fn from(err: InputValidationError) -> Self {
        // Create a fallback tool dispatch for cases where we don't have context
        let fallback_tool = ToolDispatch::Custom(crate::ToolId::new_unchecked("unknown"));
        let fallback_input = ValidatedInput::new_unchecked("".to_string());

        ToolError::InvalidInput {
            tool: fallback_tool,
            input: fallback_input,
            reason: err.to_string(),
        }
    }
}

/// Result type alias for Skreaver operations.
pub type SkreverResult<T> = Result<T, SkreverError>;
