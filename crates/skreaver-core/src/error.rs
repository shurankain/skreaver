//! # Error Types
//!
//! This module defines custom error types for domain-specific failures
//! throughout the Skreaver framework. These errors provide structured
//! information about what went wrong and enable better error handling
//! and debugging.

use crate::tool::{ToolDispatch, ToolName};
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

/// Validated input wrapper that prevents empty or excessively large inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedInput(String);

impl ValidatedInput {
    /// Maximum input size (1MB)
    pub const MAX_SIZE: usize = 1024 * 1024;

    /// Create validated input with size and content checks.
    pub fn new(input: String) -> Result<Self, InputValidationError> {
        if input.is_empty() {
            return Err(InputValidationError::Empty);
        }

        if input.len() > Self::MAX_SIZE {
            return Err(InputValidationError::TooLarge {
                size: input.len(),
                max_size: Self::MAX_SIZE,
            });
        }

        // Check for potentially problematic binary content
        if input
            .bytes()
            .filter(|&b| b < 32 && b != b'\n' && b != b'\t' && b != b'\r')
            .count()
            > input.len() / 10
        {
            return Err(InputValidationError::BinaryContent);
        }

        Ok(ValidatedInput(input))
    }

    /// Create validated input without checks (for internal use).
    pub(crate) fn new_unchecked(input: String) -> Self {
        ValidatedInput(input)
    }

    /// Get the input as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the input length.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if input is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert to owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for ValidatedInput {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ValidatedInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Truncate display for very long inputs
        if self.0.len() > 100 {
            write!(f, "{}...", &self.0[..97])
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Errors that can occur during input validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputValidationError {
    Empty,
    TooLarge { size: usize, max_size: usize },
    BinaryContent,
}

impl std::fmt::Display for InputValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputValidationError::Empty => write!(f, "Input cannot be empty"),
            InputValidationError::TooLarge { size, max_size } => {
                write!(
                    f,
                    "Input too large: {} bytes (max: {} bytes)",
                    size, max_size
                )
            }
            InputValidationError::BinaryContent => {
                write!(f, "Input contains excessive binary content")
            }
        }
    }
}

impl std::error::Error for InputValidationError {}

/// Strongly-typed memory backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBackend {
    InMemory,
    File,
    Redis,
    Sqlite,
    Postgres,
}

impl std::fmt::Display for MemoryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryBackend::InMemory => write!(f, "in-memory"),
            MemoryBackend::File => write!(f, "file"),
            MemoryBackend::Redis => write!(f, "redis"),
            MemoryBackend::Sqlite => write!(f, "sqlite"),
            MemoryBackend::Postgres => write!(f, "postgres"),
        }
    }
}

/// Strongly-typed memory operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryOperation {
    Store,
    Load,
    Delete,
    List,
    Snapshot,
    Restore,
    Connect,
    Disconnect,
}

impl std::fmt::Display for MemoryOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryOperation::Store => write!(f, "store"),
            MemoryOperation::Load => write!(f, "load"),
            MemoryOperation::Delete => write!(f, "delete"),
            MemoryOperation::List => write!(f, "list"),
            MemoryOperation::Snapshot => write!(f, "snapshot"),
            MemoryOperation::Restore => write!(f, "restore"),
            MemoryOperation::Connect => write!(f, "connect"),
            MemoryOperation::Disconnect => write!(f, "disconnect"),
        }
    }
}

/// Strongly-typed memory error categories
#[derive(Debug, Clone)]
pub enum MemoryErrorKind {
    /// Key validation failed
    InvalidKey { validation_error: String },

    /// Value validation failed
    InvalidValue { validation_error: String },

    /// Key not found during load operation
    KeyNotFound,

    /// Key already exists during store operation
    KeyAlreadyExists,

    /// Network connectivity issues
    NetworkError { details: String },

    /// Disk I/O issues
    IoError { details: String },

    /// Serialization/deserialization issues
    SerializationError { details: String },

    /// Resource constraints (memory, disk space, etc.)
    ResourceExhausted { resource: String, limit: String },

    /// Backend-specific authentication/authorization
    AccessDenied { reason: String },

    /// Backend service unavailable
    ServiceUnavailable { retry_after_ms: Option<u64> },

    /// Internal backend error
    InternalError { backend_error: String },
}

/// Errors that can occur during memory operations.
#[derive(Debug, Clone)]
pub enum MemoryError {
    /// Failed to store data in memory.
    StoreFailed {
        key: crate::memory::MemoryKey,
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    },

    /// Failed to load data from memory.
    LoadFailed {
        key: crate::memory::MemoryKey,
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    },

    /// Failed to delete data from memory.
    DeleteFailed {
        key: crate::memory::MemoryKey,
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    },

    /// Snapshot creation failed.
    SnapshotFailed {
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    },

    /// Snapshot restoration failed.
    RestoreFailed {
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    },

    /// Memory backend connection failed.
    ConnectionFailed {
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    },

    /// Generic operation failed with structured information.
    OperationFailed {
        operation: MemoryOperation,
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    },
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

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryError::StoreFailed { key, backend, kind } => {
                write!(
                    f,
                    "Failed to store key '{}' on {} backend: {}",
                    key.as_str(),
                    backend,
                    kind
                )
            }
            MemoryError::LoadFailed { key, backend, kind } => {
                write!(
                    f,
                    "Failed to load key '{}' from {} backend: {}",
                    key.as_str(),
                    backend,
                    kind
                )
            }
            MemoryError::DeleteFailed { key, backend, kind } => {
                write!(
                    f,
                    "Failed to delete key '{}' from {} backend: {}",
                    key.as_str(),
                    backend,
                    kind
                )
            }
            MemoryError::SnapshotFailed { backend, kind } => {
                write!(
                    f,
                    "Snapshot creation failed on {} backend: {}",
                    backend, kind
                )
            }
            MemoryError::RestoreFailed { backend, kind } => {
                write!(
                    f,
                    "Snapshot restoration failed on {} backend: {}",
                    backend, kind
                )
            }
            MemoryError::ConnectionFailed { backend, kind } => {
                write!(f, "Connection to {} backend failed: {}", backend, kind)
            }
            MemoryError::OperationFailed {
                operation,
                backend,
                kind,
            } => {
                write!(
                    f,
                    "Memory {} operation failed on {} backend: {}",
                    operation, backend, kind
                )
            }
        }
    }
}

impl fmt::Display for MemoryErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryErrorKind::InvalidKey { validation_error } => {
                write!(f, "invalid key: {}", validation_error)
            }
            MemoryErrorKind::InvalidValue { validation_error } => {
                write!(f, "invalid value: {}", validation_error)
            }
            MemoryErrorKind::KeyNotFound => {
                write!(f, "key not found")
            }
            MemoryErrorKind::KeyAlreadyExists => {
                write!(f, "key already exists")
            }
            MemoryErrorKind::NetworkError { details } => {
                write!(f, "network error: {}", details)
            }
            MemoryErrorKind::IoError { details } => {
                write!(f, "I/O error: {}", details)
            }
            MemoryErrorKind::SerializationError { details } => {
                write!(f, "serialization error: {}", details)
            }
            MemoryErrorKind::ResourceExhausted { resource, limit } => {
                write!(f, "{} exhausted (limit: {})", resource, limit)
            }
            MemoryErrorKind::AccessDenied { reason } => {
                write!(f, "access denied: {}", reason)
            }
            MemoryErrorKind::ServiceUnavailable { retry_after_ms } => {
                if let Some(retry_ms) = retry_after_ms {
                    write!(f, "service unavailable (retry after {}ms)", retry_ms)
                } else {
                    write!(f, "service unavailable")
                }
            }
            MemoryErrorKind::InternalError { backend_error } => {
                write!(f, "internal error: {}", backend_error)
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
            backend: MemoryBackend::InMemory,
            kind: MemoryErrorKind::InvalidKey {
                validation_error: err.to_string(),
            },
        })
    }
}

impl From<crate::tool::InvalidToolName> for ToolError {
    fn from(err: crate::tool::InvalidToolName) -> Self {
        ToolError::InvalidToolName {
            attempted_name: "unknown".to_string(),
            validation_error: err,
        }
    }
}

impl From<InputValidationError> for ToolError {
    fn from(err: InputValidationError) -> Self {
        // Create a fallback tool dispatch for cases where we don't have context
        let fallback_tool =
            ToolDispatch::Custom(ToolName::new("unknown").expect("'unknown' is a valid tool name"));
        let fallback_input = ValidatedInput::new_unchecked("".to_string());

        ToolError::InvalidInput {
            tool: fallback_tool,
            input: fallback_input,
            reason: err.to_string(),
        }
    }
}

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

impl MemoryError {
    /// Create a store failed error with structured information.
    pub fn store_failed(
        key: crate::memory::MemoryKey,
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    ) -> Self {
        MemoryError::StoreFailed { key, backend, kind }
    }

    /// Create a load failed error with structured information.
    pub fn load_failed(
        key: crate::memory::MemoryKey,
        backend: MemoryBackend,
        kind: MemoryErrorKind,
    ) -> Self {
        MemoryError::LoadFailed { key, backend, kind }
    }

    /// Create a key not found error.
    pub fn key_not_found(key: crate::memory::MemoryKey, backend: MemoryBackend) -> Self {
        MemoryError::LoadFailed {
            key,
            backend,
            kind: MemoryErrorKind::KeyNotFound,
        }
    }

    /// Create a network error.
    pub fn network_error(
        operation: MemoryOperation,
        backend: MemoryBackend,
        details: String,
    ) -> Self {
        MemoryError::OperationFailed {
            operation,
            backend,
            kind: MemoryErrorKind::NetworkError { details },
        }
    }

    /// Create an I/O error.
    pub fn io_error(operation: MemoryOperation, backend: MemoryBackend, details: String) -> Self {
        MemoryError::OperationFailed {
            operation,
            backend,
            kind: MemoryErrorKind::IoError { details },
        }
    }

    /// Create a serialization error.
    pub fn serialization_error(
        operation: MemoryOperation,
        backend: MemoryBackend,
        details: String,
    ) -> Self {
        MemoryError::OperationFailed {
            operation,
            backend,
            kind: MemoryErrorKind::SerializationError { details },
        }
    }

    /// Create a connection failed error.
    pub fn connection_failed(backend: MemoryBackend, kind: MemoryErrorKind) -> Self {
        MemoryError::ConnectionFailed { backend, kind }
    }

    /// Create a snapshot failed error.
    pub fn snapshot_failed(backend: MemoryBackend, kind: MemoryErrorKind) -> Self {
        MemoryError::SnapshotFailed { backend, kind }
    }

    /// Create a restore failed error.
    pub fn restore_failed(backend: MemoryBackend, kind: MemoryErrorKind) -> Self {
        MemoryError::RestoreFailed { backend, kind }
    }

    /// Get the backend associated with this error.
    pub fn backend(&self) -> MemoryBackend {
        match self {
            MemoryError::StoreFailed { backend, .. }
            | MemoryError::LoadFailed { backend, .. }
            | MemoryError::DeleteFailed { backend, .. }
            | MemoryError::SnapshotFailed { backend, .. }
            | MemoryError::RestoreFailed { backend, .. }
            | MemoryError::ConnectionFailed { backend, .. }
            | MemoryError::OperationFailed { backend, .. } => *backend,
        }
    }

    /// Get the error kind associated with this error.
    pub fn kind(&self) -> &MemoryErrorKind {
        match self {
            MemoryError::StoreFailed { kind, .. }
            | MemoryError::LoadFailed { kind, .. }
            | MemoryError::DeleteFailed { kind, .. }
            | MemoryError::SnapshotFailed { kind, .. }
            | MemoryError::RestoreFailed { kind, .. }
            | MemoryError::ConnectionFailed { kind, .. }
            | MemoryError::OperationFailed { kind, .. } => kind,
        }
    }

    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self.kind() {
            MemoryErrorKind::NetworkError { .. }
            | MemoryErrorKind::ServiceUnavailable { .. }
            | MemoryErrorKind::ResourceExhausted { .. } => true,
            MemoryErrorKind::InvalidKey { .. }
            | MemoryErrorKind::InvalidValue { .. }
            | MemoryErrorKind::KeyNotFound
            | MemoryErrorKind::KeyAlreadyExists
            | MemoryErrorKind::AccessDenied { .. }
            | MemoryErrorKind::SerializationError { .. }
            | MemoryErrorKind::IoError { .. }
            | MemoryErrorKind::InternalError { .. } => false,
        }
    }

    /// Get retry delay in milliseconds, if applicable.
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self.kind() {
            MemoryErrorKind::ServiceUnavailable { retry_after_ms } => *retry_after_ms,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::StandardTool;

    #[test]
    fn test_tool_error_not_found_with_standard_tool() {
        let tool = ToolDispatch::Standard(StandardTool::HttpGet);
        let error = ToolError::not_found(tool);

        assert_eq!(error.tool_name(), Some("http_get"));
        assert!(error.to_string().contains("http_get"));
        assert!(error.to_string().contains("not found"));
    }

    #[test]
    fn test_tool_error_not_found_by_name() {
        let error = ToolError::not_found_by_name("file_read");
        assert_eq!(error.tool_name(), Some("file_read"));

        // Test invalid name
        let error = ToolError::not_found_by_name("invalid tool name!");
        assert!(matches!(error, ToolError::InvalidToolName { .. }));
        assert_eq!(error.tool_name(), None);
    }

    #[test]
    fn test_tool_error_execution_failed() {
        let tool = ToolDispatch::Standard(StandardTool::JsonParse);
        let error = ToolError::execution_failed(tool, "Invalid JSON format".to_string());

        assert_eq!(error.tool_name(), Some("json_parse"));
        assert!(error.to_string().contains("json_parse"));
        assert!(error.to_string().contains("execution failed"));
        assert!(error.to_string().contains("Invalid JSON format"));
    }

    #[test]
    fn test_tool_error_invalid_input() {
        let tool = ToolDispatch::Standard(StandardTool::HttpPost);
        let error = ToolError::invalid_input(
            tool,
            "test input".to_string(),
            "Missing required field".to_string(),
        );

        assert_eq!(error.tool_name(), Some("http_post"));
        assert!(error.to_string().contains("invalid input"));
        assert!(error.to_string().contains("test input"));
        assert!(error.to_string().contains("Missing required field"));
    }

    #[test]
    fn test_tool_error_timeout() {
        let tool = ToolDispatch::Custom(ToolName::new("custom_tool").expect("Valid tool name"));
        let error = ToolError::timeout(tool, 5000);

        assert_eq!(error.tool_name(), Some("custom_tool"));
        assert!(error.to_string().contains("custom_tool"));
        assert!(error.to_string().contains("timed out"));
        assert!(error.to_string().contains("5000ms"));
    }

    #[test]
    fn test_validated_input() {
        // Valid input
        let input = ValidatedInput::new("Hello, world!".to_string()).unwrap();
        assert_eq!(input.as_str(), "Hello, world!");
        assert_eq!(input.len(), 13);
        assert!(!input.is_empty());

        // Empty input
        assert!(matches!(
            ValidatedInput::new("".to_string()),
            Err(InputValidationError::Empty)
        ));

        // Too large input
        let large_input = "x".repeat(ValidatedInput::MAX_SIZE + 1);
        assert!(matches!(
            ValidatedInput::new(large_input),
            Err(InputValidationError::TooLarge { .. })
        ));

        // Binary content (lots of null bytes and control characters)
        let binary_input = (0..20u8)
            .cycle()
            .take(100)
            .map(|b| b as char)
            .collect::<String>();
        assert!(matches!(
            ValidatedInput::new(binary_input),
            Err(InputValidationError::BinaryContent)
        ));
    }

    #[test]
    fn test_validated_input_display_truncation() {
        let short_input = ValidatedInput::new_unchecked("short".to_string());
        assert_eq!(short_input.to_string(), "short");

        let long_input = ValidatedInput::new_unchecked("x".repeat(200));
        let display = long_input.to_string();
        assert!(display.len() <= 100);
        assert!(display.ends_with("..."));
    }

    #[test]
    fn test_tool_error_conversions() {
        // Test InvalidToolName conversion
        let invalid_name = crate::tool::InvalidToolName::Empty;
        let tool_error: ToolError = invalid_name.into();
        assert!(matches!(tool_error, ToolError::InvalidToolName { .. }));

        // Test InputValidationError conversion
        let input_error = InputValidationError::Empty;
        let tool_error: ToolError = input_error.into();
        assert!(matches!(tool_error, ToolError::InvalidInput { .. }));
    }

    #[test]
    fn test_error_hierarchy() {
        let tool_error = ToolError::not_found_by_name("missing_tool");
        let skrever_error: SkreverError = tool_error.into();

        assert!(matches!(skrever_error, SkreverError::Tool(_)));
        assert!(skrever_error.to_string().contains("Tool error"));
    }
}
