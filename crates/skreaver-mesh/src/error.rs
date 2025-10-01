//! Error types for mesh operations

use thiserror::Error;

/// Result type for mesh operations
pub type MeshResult<T> = Result<T, MeshError>;

/// Errors that can occur during mesh operations
#[derive(Error, Debug)]
pub enum MeshError {
    /// Connection to messaging backend failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Failed to send message
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// Failed to receive message
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    /// Failed to subscribe to topic
    #[error("Subscribe failed: {0}")]
    SubscribeFailed(String),

    /// Failed to unsubscribe from topic
    #[error("Unsubscribe failed: {0}")]
    UnsubscribeFailed(String),

    /// Message serialization failed
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    /// Message deserialization failed
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),

    /// Queue is full (backpressure)
    #[error("Queue full: capacity {capacity}, current size {current}")]
    QueueFull { capacity: usize, current: usize },

    /// Message size exceeds limit
    #[error("Message too large: {size} bytes (limit: {limit} bytes)")]
    MessageTooLarge { size: usize, limit: usize },

    /// Agent not found
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Topic not found
    #[error("Topic not found: {0}")]
    TopicNotFound(String),

    /// Operation timeout
    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Backend-specific error (Redis, etc.)
    #[error("Backend error: {0}")]
    BackendError(String),

    /// Generic error
    #[error("Mesh error: {0}")]
    Other(String),
}

#[cfg(feature = "redis")]
impl From<redis::RedisError> for MeshError {
    fn from(err: redis::RedisError) -> Self {
        MeshError::BackendError(err.to_string())
    }
}

impl From<serde_json::Error> for MeshError {
    fn from(err: serde_json::Error) -> Self {
        MeshError::SerializationFailed(err.to_string())
    }
}
