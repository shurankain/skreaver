//! Request type definitions for HTTP endpoints
//!
//! This module contains all the request DTOs used by the HTTP runtime endpoints.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Stream response mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamMode {
    /// Complete response returned at once
    Complete,
    /// Stream response chunks as they become available
    Streaming,
    /// Stream with debug information included
    Debug,
}

impl StreamMode {
    /// Check if streaming is enabled
    pub fn is_streaming(self) -> bool {
        matches!(self, Self::Streaming | Self::Debug)
    }

    /// Check if debug mode is enabled
    pub fn is_debug(self) -> bool {
        matches!(self, Self::Debug)
    }
}

impl Default for StreamMode {
    fn default() -> Self {
        Self::Complete
    }
}

/// Request body for creating a new agent
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAgentRequest {
    /// Type of agent to create
    #[schema(example = "simple_agent")]
    pub agent_type: String,
    /// Optional name for the agent
    #[schema(example = "my-agent")]
    pub name: Option<String>,
}

/// Request body for sending observations to an agent
#[derive(Debug, Deserialize, ToSchema)]
pub struct ObserveRequest {
    /// Input observation for the agent
    #[schema(example = "Hello, agent!")]
    pub input: String,
    /// Response streaming mode
    #[serde(default)]
    pub stream_mode: StreamMode,
    /// Priority for request processing (Low, Normal, High, Critical)
    #[serde(default)]
    #[schema(default = "Normal")]
    pub priority: Option<String>,
    /// Timeout for the operation in seconds
    #[serde(default)]
    #[schema(default = 30)]
    pub timeout_seconds: Option<u64>,
}

/// Request body for creating a JWT token
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTokenRequest {
    /// User identifier for the token
    #[schema(example = "test-user")]
    pub user_id: String,
    /// Permissions to grant to the user
    pub permissions: Vec<String>,
}

/// Query parameters for streaming endpoint
#[derive(Debug, Deserialize)]
pub struct StreamRequest {
    /// Optional input to send to the agent
    pub input: Option<String>,
    /// Stream mode configuration
    #[serde(default)]
    pub stream_mode: StreamMode,
    /// Custom timeout in seconds for the operation
    pub timeout_seconds: Option<u64>,
}

/// Request for batch operations
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchObserveRequest {
    /// List of inputs to process
    pub inputs: Vec<String>,
    /// Response streaming mode
    #[serde(default)]
    pub stream_mode: StreamMode,
    /// Maximum parallel operations
    #[serde(default = "default_parallel_limit")]
    pub parallel_limit: usize,
    /// Timeout per individual operation in seconds
    #[serde(default = "default_operation_timeout")]
    pub timeout_seconds: u64,
}

fn default_parallel_limit() -> usize {
    5
}

fn default_operation_timeout() -> u64 {
    30
}
