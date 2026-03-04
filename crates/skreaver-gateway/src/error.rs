//! Gateway error types
//!
//! This module defines error types for the protocol gateway, covering
//! protocol detection, translation, and connection management failures.

use thiserror::Error;

/// Gateway error type
#[derive(Debug, Error)]
pub enum GatewayError {
    /// Failed to detect protocol from message
    #[error("Failed to detect protocol: {0}")]
    ProtocolDetectionFailed(String),

    /// Translation error between protocols
    #[error("Translation error: {0}")]
    TranslationError(String),

    /// Connection not found in registry
    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),

    /// Connection already exists
    #[error("Connection already exists: {0}")]
    ConnectionAlreadyExists(String),

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Unsupported protocol version
    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// MCP protocol error
    #[error("MCP error: {0}")]
    McpError(#[from] skreaver_mcp::McpError),

    /// A2A protocol error
    #[error("A2A error: {0}")]
    A2aError(#[from] skreaver_a2a::A2aError),

    /// Internal gateway error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for gateway operations
pub type GatewayResult<T> = Result<T, GatewayError>;
