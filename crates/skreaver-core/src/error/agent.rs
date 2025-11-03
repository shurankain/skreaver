//! Agent and coordinator execution errors.
//!
//! This module defines errors for agent lifecycle operations and coordinator
//! orchestration, including observation processing, action generation, and
//! context management.

use std::fmt;

/// Errors that can occur during agent operations.
#[derive(Debug, Clone)]
pub enum AgentError {
    /// Agent failed to process an observation.
    ObservationFailed { reason: String },

    /// Agent failed to generate an action.
    ActionFailed { reason: String },

    /// Agent's memory access failed.
    MemoryAccessFailed { operation: String, reason: String },

    /// Agent is in an invalid state for the requested operation.
    InvalidState {
        current_state: String,
        operation: String,
    },
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::ObservationFailed { reason } => {
                write!(f, "Observation processing failed: {}", reason)
            }
            AgentError::ActionFailed { reason } => {
                write!(f, "Action generation failed: {}", reason)
            }
            AgentError::MemoryAccessFailed { operation, reason } => {
                write!(f, "Memory {} failed: {}", operation, reason)
            }
            AgentError::InvalidState {
                current_state,
                operation,
            } => write!(f, "Cannot {} in state '{}'", operation, current_state),
        }
    }
}

impl std::error::Error for AgentError {}

/// Errors that can occur during coordinator operations.
#[derive(Debug, Clone)]
pub enum CoordinatorError {
    /// Agent step execution failed.
    StepFailed { reason: String },

    /// Tool dispatch failed for all requested tools.
    ToolDispatchFailed { failed_tools: Vec<String> },

    /// Context update failed.
    ContextUpdateFailed {
        key: crate::memory::MemoryKey,
        reason: String,
    },
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoordinatorError::StepFailed { reason } => write!(f, "Agent step failed: {}", reason),
            CoordinatorError::ToolDispatchFailed { failed_tools } => {
                write!(f, "Tool dispatch failed for: {}", failed_tools.join(", "))
            }
            CoordinatorError::ContextUpdateFailed { key, reason } => {
                write!(
                    f,
                    "Context update for '{}' failed: {}",
                    key.as_str(),
                    reason
                )
            }
        }
    }
}

impl std::error::Error for CoordinatorError {}

/// Result type alias for agent operations.
pub type AgentResult<T> = Result<T, AgentError>;

/// Result type alias for coordinator operations.
pub type CoordinatorResult<T> = Result<T, CoordinatorError>;
