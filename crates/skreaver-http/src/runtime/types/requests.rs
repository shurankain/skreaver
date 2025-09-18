//! Request type definitions for HTTP endpoints
//!
//! This module contains all the request DTOs used by the HTTP runtime endpoints.

use serde::Deserialize;
use utoipa::ToSchema;

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
    /// Whether to stream the response in real-time
    #[serde(default)]
    #[schema(default = false)]
    pub stream: bool,
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
    /// Whether to include debug information in stream
    #[serde(default)]
    pub debug: bool,
    /// Custom timeout in seconds for the operation
    pub timeout_seconds: Option<u64>,
}

/// Request for batch operations
#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchObserveRequest {
    /// List of inputs to process
    pub inputs: Vec<String>,
    /// Whether to return results as stream
    #[serde(default)]
    pub stream: bool,
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
