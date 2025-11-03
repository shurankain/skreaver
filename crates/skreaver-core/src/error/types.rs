//! Core error types and validation structures.
//!
//! This module contains foundational types used throughout the error handling
//! system, including input validation, memory backend types, and error kind
//! classifications.

use std::fmt;

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

impl fmt::Display for ValidatedInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl fmt::Display for InputValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl fmt::Display for MemoryBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl fmt::Display for MemoryOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
