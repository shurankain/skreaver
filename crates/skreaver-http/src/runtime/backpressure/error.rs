//! Error types for backpressure and queue management.

/// Backpressure and queue management errors
#[derive(Debug, thiserror::Error)]
pub enum BackpressureError {
    #[error("Queue is full for agent {agent_id} (max: {max_size})")]
    QueueFull { agent_id: String, max_size: usize },

    #[error("Request timed out in queue after {timeout_ms}ms")]
    QueueTimeout { timeout_ms: u64 },

    #[error("Processing timeout after {timeout_ms}ms")]
    ProcessingTimeout { timeout_ms: u64 },

    #[error("System overloaded, rejecting requests (load: {load:.2})")]
    SystemOverloaded { load: f64 },

    #[error("Agent {agent_id} not found")]
    AgentNotFound { agent_id: String },

    #[error("Request cancelled")]
    RequestCancelled,

    #[error("Internal error: {message}")]
    Internal { message: String },
}
