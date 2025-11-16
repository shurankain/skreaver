//! Response type definitions for HTTP endpoints
//!
//! This module contains all the response DTOs used by the HTTP runtime endpoints.

use serde::Serialize;
use utoipa::ToSchema;

/// Response for agent creation
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateAgentResponse {
    /// Unique identifier for the created agent
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Type of the created agent
    #[schema(example = "simple_agent")]
    pub agent_type: String,
    /// Current status of the agent
    #[schema(example = "running")]
    pub status: String,
}

/// Response from agent observation
#[derive(Debug, Serialize, ToSchema)]
pub struct ObserveResponse {
    /// ID of the agent that processed the observation
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Agent's response to the observation
    #[schema(example = "Hello! How can I help you?")]
    pub response: String,
    /// Timestamp when the response was generated
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Agent status information
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentStatus {
    /// Unique identifier of the agent
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Type of the agent
    #[schema(example = "simple_agent")]
    pub agent_type: String,
    /// Current operational status
    #[schema(example = "running")]
    pub status: String,
    /// Timestamp when the agent was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity timestamp
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// Response containing list of agents
#[derive(Debug, Serialize, ToSchema)]
pub struct AgentsListResponse {
    /// List of agent status information
    pub agents: Vec<AgentStatus>,
    /// Total number of agents
    #[schema(example = 5)]
    pub total: usize,
}

/// Error response format
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Error code identifier
    #[schema(example = "agent_not_found")]
    pub error: String,
    /// Human-readable error message
    #[schema(example = "Agent with ID 'agent-12345' not found")]
    pub message: String,
    /// Additional context or details about the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Response for JWT token creation
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateTokenResponse {
    /// JWT access token
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub token: String,
    /// Token expiration time in seconds
    #[schema(example = 86400)]
    pub expires_in: u64,
    /// Token type
    #[schema(example = "Bearer")]
    pub token_type: String,
}

/// Response for batch observe operations
#[derive(Debug, Serialize, ToSchema)]
pub struct BatchObserveResponse {
    /// Agent identifier
    pub agent_id: String,
    /// Results for each input
    pub results: Vec<BatchResult>,
    /// Total processing time
    pub total_time_ms: u64,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Outcome of a batch item processing
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum BatchOutcome {
    /// Processing succeeded
    Success {
        /// Agent's response to the input
        response: String,
    },
    /// Processing failed
    Failure {
        /// Error message
        error: String,
    },
}

/// Individual result in batch operation
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BatchResult {
    /// Input index
    pub index: usize,
    /// Input that was processed
    pub input: String,
    /// Processing outcome
    #[serde(flatten)]
    pub outcome: BatchOutcome,
    /// Processing time for this input in milliseconds
    pub processing_time_ms: u64,
}

/// Response for queue metrics
#[derive(Debug, Serialize, ToSchema)]
pub struct QueueMetricsResponse {
    /// Agent ID (if for specific agent)
    pub agent_id: Option<String>,
    /// Number of requests in queue
    pub queue_size: usize,
    /// Number of active/processing requests
    pub active_requests: usize,
    /// Total requests processed
    pub total_processed: u64,
    /// Total requests that timed out
    pub total_timeouts: u64,
    /// Total requests rejected
    pub total_rejections: u64,
    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Current load factor (0.0-1.0)
    pub load_factor: f64,
    /// Timestamp when metrics were collected
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
