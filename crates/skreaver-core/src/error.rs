//! # Error Types
//!
//! This module defines custom error types for domain-specific failures
//! throughout the Skreaver framework. These errors provide structured
//! information about what went wrong and enable better error handling
//! and debugging.

use std::fmt;

/// Main error type for Skreaver operations.
///
/// This enum covers all major error categories that can occur during
/// agent execution, tool usage, and memory operations.
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

/// Errors that can occur during tool operations.
#[derive(Debug, Clone)]
pub enum ToolError {
    /// Tool was not found in the registry.
    NotFound { name: String },

    /// Tool execution failed with an error message.
    ExecutionFailed { name: String, message: String },

    /// Tool input was invalid or malformed.
    InvalidInput {
        name: String,
        input: String,
        reason: String,
    },

    /// Tool timed out during execution.
    Timeout { name: String, duration_ms: u64 },

    /// Tool registry is full or cannot accept more tools.
    RegistryFull,
}

/// Errors that can occur during memory operations.
#[derive(Debug, Clone)]
pub enum MemoryError {
    /// Failed to store data in memory.
    StoreFailed {
        key: crate::memory::MemoryKey,
        reason: String,
    },

    /// Failed to load data from memory.
    LoadFailed {
        key: crate::memory::MemoryKey,
        reason: String,
    },

    /// Snapshot creation failed.
    SnapshotFailed { reason: String },

    /// Snapshot restoration failed.
    RestoreFailed { reason: String },

    /// Memory backend connection failed.
    ConnectionFailed { backend: String, reason: String },

    /// Serialization/deserialization error.
    SerializationError { reason: String },
}

/// Errors that can occur during transactional memory operations.
#[derive(Debug, Clone)]
pub enum TransactionError {
    /// Transaction failed and was rolled back.
    TransactionFailed { reason: String },

    /// Transaction was aborted by user code.
    TransactionAborted { reason: String },

    /// Underlying memory operation failed within transaction.
    MemoryError(MemoryError),

    /// Transaction deadlock detected.
    Deadlock { timeout_ms: u64 },

    /// Transaction conflicts with concurrent operations.
    ConflictDetected { conflicting_keys: Vec<String> },
}

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

impl fmt::Display for SkreverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkreverError::Tool(e) => write!(f, "Tool error: {}", e),
            SkreverError::Memory(e) => write!(f, "Memory error: {}", e),
            SkreverError::Agent(e) => write!(f, "Agent error: {}", e),
            SkreverError::Coordinator(e) => write!(f, "Coordinator error: {}", e),
        }
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::NotFound { name } => write!(f, "Tool '{}' not found in registry", name),
            ToolError::ExecutionFailed { name, message } => {
                write!(f, "Tool '{}' execution failed: {}", name, message)
            }
            ToolError::InvalidInput {
                name,
                input,
                reason,
            } => write!(
                f,
                "Tool '{}' received invalid input '{}': {}",
                name, input, reason
            ),
            ToolError::Timeout { name, duration_ms } => {
                write!(f, "Tool '{}' timed out after {}ms", name, duration_ms)
            }
            ToolError::RegistryFull => write!(f, "Tool registry is full"),
        }
    }
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::StoreFailed { key, reason } => {
                write!(f, "Failed to store key '{}': {}", key.as_str(), reason)
            }
            MemoryError::LoadFailed { key, reason } => {
                write!(f, "Failed to load key '{}': {}", key.as_str(), reason)
            }
            MemoryError::SnapshotFailed { reason } => {
                write!(f, "Snapshot creation failed: {}", reason)
            }
            MemoryError::RestoreFailed { reason } => {
                write!(f, "Snapshot restoration failed: {}", reason)
            }
            MemoryError::ConnectionFailed { backend, reason } => {
                write!(f, "Connection to {} backend failed: {}", backend, reason)
            }
            MemoryError::SerializationError { reason } => {
                write!(f, "Serialization error: {}", reason)
            }
        }
    }
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionError::TransactionFailed { reason } => {
                write!(f, "Transaction failed: {}", reason)
            }
            TransactionError::TransactionAborted { reason } => {
                write!(f, "Transaction aborted: {}", reason)
            }
            TransactionError::MemoryError(err) => {
                write!(f, "Memory error in transaction: {}", err)
            }
            TransactionError::Deadlock { timeout_ms } => {
                write!(f, "Transaction deadlock detected after {}ms", timeout_ms)
            }
            TransactionError::ConflictDetected { conflicting_keys } => {
                write!(
                    f,
                    "Transaction conflict on keys: {}",
                    conflicting_keys.join(", ")
                )
            }
        }
    }
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

impl std::error::Error for SkreverError {}
impl std::error::Error for ToolError {}
impl std::error::Error for MemoryError {}
impl std::error::Error for TransactionError {}
impl std::error::Error for AgentError {}
impl std::error::Error for CoordinatorError {}

// Convenience conversions
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
            reason: err.to_string(),
        })
    }
}

/// Result type alias for Skreaver operations.
pub type SkreverResult<T> = Result<T, SkreverError>;

/// Result type alias for tool operations.
pub type ToolResult<T> = Result<T, ToolError>;

/// Result type alias for memory operations.
pub type MemoryResult<T> = Result<T, MemoryError>;

/// Result type alias for agent operations.
pub type AgentResult<T> = Result<T, AgentError>;

/// Result type alias for coordinator operations.
pub type CoordinatorResult<T> = Result<T, CoordinatorError>;

/// Result type alias for transaction operations.
pub type TransactionResult<T> = Result<T, TransactionError>;
