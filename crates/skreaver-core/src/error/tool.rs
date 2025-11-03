//! Tool execution and dispatch errors.
//!
//! This module defines errors related to tool operations, including tool
//! lookup, execution, validation, and timeout handling. All tool errors
//! use validated types to ensure compile-time safety.

use crate::tool::ToolDispatch;
use std::fmt;

use super::types::ValidatedInput;

/// Errors that can occur during tool operations with compile-time safety.
#[derive(Debug, Clone)]
pub enum ToolError {
    /// Tool was not found in the registry.
    NotFound {
        /// Validated tool identifier
        tool: ToolDispatch,
    },

    /// Tool execution failed with an error message.
    ExecutionFailed {
        /// Validated tool identifier
        tool: ToolDispatch,
        /// Error message from the tool execution
        message: String,
    },

    /// Tool input was invalid or malformed.
    InvalidInput {
        /// Validated tool identifier
        tool: ToolDispatch,
        /// The invalid input that was provided
        input: ValidatedInput,
        /// Reason why the input was invalid
        reason: String,
    },

    /// Tool timed out during execution.
    Timeout {
        /// Validated tool identifier
        tool: ToolDispatch,
        /// Timeout duration in milliseconds
        duration_ms: u64,
    },

    /// Tool registry is full or cannot accept more tools.
    RegistryFull,

    /// Tool name validation failed during dispatch.
    InvalidToolName {
        /// The invalid tool name that was provided
        attempted_name: String,
        /// Validation error details
        validation_error: crate::tool::InvalidToolName,
    },
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::NotFound { tool } => {
                write!(f, "Tool '{}' not found in registry", tool.name())
            }
            ToolError::ExecutionFailed { tool, message } => {
                write!(f, "Tool '{}' execution failed: {}", tool.name(), message)
            }
            ToolError::InvalidInput {
                tool,
                input,
                reason,
            } => {
                write!(
                    f,
                    "Tool '{}' received invalid input '{}': {}",
                    tool.name(),
                    input,
                    reason
                )
            }
            ToolError::Timeout { tool, duration_ms } => {
                write!(
                    f,
                    "Tool '{}' timed out after {}ms",
                    tool.name(),
                    duration_ms
                )
            }
            ToolError::RegistryFull => write!(f, "Tool registry is full"),
            ToolError::InvalidToolName {
                attempted_name,
                validation_error,
            } => {
                write!(
                    f,
                    "Invalid tool name '{}': {}",
                    attempted_name, validation_error
                )
            }
        }
    }
}

impl std::error::Error for ToolError {}

impl ToolError {
    /// Create a NotFound error for a validated tool.
    pub fn not_found(tool: ToolDispatch) -> Self {
        ToolError::NotFound { tool }
    }

    /// Create a NotFound error from a tool name string.
    pub fn not_found_by_name(name: &str) -> Self {
        match ToolDispatch::from_name(name) {
            Ok(tool) => ToolError::NotFound { tool },
            Err(validation_error) => ToolError::InvalidToolName {
                attempted_name: name.to_string(),
                validation_error,
            },
        }
    }

    /// Create an ExecutionFailed error for a validated tool.
    pub fn execution_failed(tool: ToolDispatch, message: String) -> Self {
        ToolError::ExecutionFailed { tool, message }
    }

    /// Create an ExecutionFailed error from a tool name string.
    pub fn execution_failed_by_name(name: &str, message: String) -> Self {
        match ToolDispatch::from_name(name) {
            Ok(tool) => ToolError::ExecutionFailed { tool, message },
            Err(validation_error) => ToolError::InvalidToolName {
                attempted_name: name.to_string(),
                validation_error,
            },
        }
    }

    /// Create an InvalidInput error with validation.
    pub fn invalid_input(tool: ToolDispatch, input: String, reason: String) -> Self {
        let validated_input = ValidatedInput::new(input)
            .unwrap_or_else(|_| ValidatedInput::new_unchecked("invalid".to_string()));

        ToolError::InvalidInput {
            tool,
            input: validated_input,
            reason,
        }
    }

    /// Create a Timeout error for a validated tool.
    pub fn timeout(tool: ToolDispatch, duration_ms: u64) -> Self {
        ToolError::Timeout { tool, duration_ms }
    }

    /// Create a Timeout error from a tool name string.
    pub fn timeout_by_name(name: &str, duration_ms: u64) -> Self {
        match ToolDispatch::from_name(name) {
            Ok(tool) => ToolError::Timeout { tool, duration_ms },
            Err(validation_error) => ToolError::InvalidToolName {
                attempted_name: name.to_string(),
                validation_error,
            },
        }
    }

    /// Get the tool dispatch associated with this error, if available.
    pub fn tool(&self) -> Option<&ToolDispatch> {
        match self {
            ToolError::NotFound { tool }
            | ToolError::ExecutionFailed { tool, .. }
            | ToolError::InvalidInput { tool, .. }
            | ToolError::Timeout { tool, .. } => Some(tool),
            ToolError::RegistryFull | ToolError::InvalidToolName { .. } => None,
        }
    }

    /// Get the tool name as a string, if available.
    pub fn tool_name(&self) -> Option<&str> {
        self.tool().map(|tool| tool.name())
    }
}

/// Result type alias for tool operations.
pub type ToolResult<T> = Result<T, ToolError>;
