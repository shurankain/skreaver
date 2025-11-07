//! Memory backend and transaction errors.
//!
//! This module defines errors for memory operations including storage,
//! retrieval, transactions, and backend-specific failures. All errors
//! use strongly-typed backend and operation enums for clarity.

use std::fmt;

use super::types::{MemoryBackend, MemoryErrorKind, MemoryOperation};

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

impl std::error::Error for MemoryError {}

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
            | MemoryErrorKind::Timeout { .. } // Timeouts are generally not retryable
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

impl std::error::Error for TransactionError {}

/// Result type alias for memory operations.
pub type MemoryResult<T> = Result<T, MemoryError>;

/// Result type alias for transaction operations.
pub type TransactionResult<T> = Result<T, TransactionError>;
