//! MCP error types

use rmcp::model::Content;
use thiserror::Error;

/// MCP operation result type
pub type McpResult<T> = Result<T, McpError>;

/// Errors that can occur during MCP operations
#[derive(Debug, Error)]
pub enum McpError {
    /// Tool execution failed
    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Invalid tool parameters
    #[error("Invalid tool parameters: {0}")]
    InvalidParameters(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// MCP protocol error
    #[error("MCP protocol error: {0}")]
    ProtocolError(String),

    /// Transport error
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Server error
    #[error("Server error: {0}")]
    ServerError(String),

    /// Client error (for MCP bridge)
    #[error("Client error: {0}")]
    ClientError(String),

    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

/// Implement IntoContents for McpError so it can be returned from tool functions
impl rmcp::model::IntoContents for McpError {
    fn into_contents(self) -> Vec<Content> {
        vec![Content::text(self.to_string())]
    }
}
