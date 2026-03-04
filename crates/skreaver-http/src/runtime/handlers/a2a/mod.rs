//! A2A (Agent-to-Agent) Protocol HTTP Handlers
//!
//! This module implements the A2A protocol HTTP transport layer, providing
//! REST endpoints for agent-to-agent communication following the A2A specification.
//!
//! ## Endpoints
//!
//! - `GET /a2a/agent-card` - Retrieve the agent's capability card
//! - `POST /a2a/tasks` - Submit a new task
//! - `GET /a2a/tasks/:id` - Get task status
//! - `DELETE /a2a/tasks/:id` - Cancel a task
//! - `POST /a2a/tasks/:id/messages` - Send a message to a task
//! - `GET /a2a/events` - Subscribe to Server-Sent Events stream
//! - `GET /a2a/health` - A2A-specific health check
//!
//! ## Protocol Compliance
//!
//! These handlers implement the A2A protocol as defined in the specification,
//! using JSON-RPC 2.0 style request/response formats where appropriate.

pub mod agent_card;
pub mod errors;
pub mod events;
pub mod tasks;

use axum::{
    Router, middleware,
    routing::{delete, get, post},
};
use skreaver_tools::ToolRegistry;
use std::sync::Arc;

use crate::runtime::HttpAgentRuntime;
use crate::runtime::auth::require_auth;

pub use agent_card::get_agent_card;
pub use errors::A2aApiError;
pub use events::{A2aEventBroadcaster, events_stream};
pub use tasks::{A2aTaskStore, cancel_task, create_task, get_task, send_task_message};

/// State shared by A2A handlers
#[derive(Clone)]
pub struct A2aState<T: ToolRegistry + Clone + Send + Sync + 'static> {
    /// The HTTP agent runtime
    pub runtime: HttpAgentRuntime<T>,
    /// Task storage
    pub task_store: Arc<A2aTaskStore>,
    /// Event broadcaster for SSE
    pub event_broadcaster: Arc<A2aEventBroadcaster>,
    /// Agent card configuration
    pub agent_card_config: A2aAgentCardConfig,
}

/// Configuration for the agent card
#[derive(Clone, Debug)]
pub struct A2aAgentCardConfig {
    /// Agent name
    pub name: String,
    /// Agent description
    pub description: String,
    /// Base URL for the agent
    pub base_url: String,
    /// Agent version
    pub version: String,
    /// Whether streaming is supported
    pub supports_streaming: bool,
    /// Whether push notifications are supported
    pub supports_push_notifications: bool,
}

impl Default for A2aAgentCardConfig {
    fn default() -> Self {
        Self {
            name: "Skreaver Agent".to_string(),
            description: "A Skreaver-powered AI agent".to_string(),
            base_url: "http://localhost:3000".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            supports_streaming: true,
            supports_push_notifications: false,
        }
    }
}

impl<T: ToolRegistry + Clone + Send + Sync + 'static> A2aState<T> {
    /// Create a new A2A state with default configuration
    pub fn new(runtime: HttpAgentRuntime<T>) -> Self {
        Self {
            runtime,
            task_store: Arc::new(A2aTaskStore::new()),
            event_broadcaster: Arc::new(A2aEventBroadcaster::new()),
            agent_card_config: A2aAgentCardConfig::default(),
        }
    }

    /// Create a new A2A state with custom configuration
    pub fn with_config(runtime: HttpAgentRuntime<T>, config: A2aAgentCardConfig) -> Self {
        Self {
            runtime,
            task_store: Arc::new(A2aTaskStore::new()),
            event_broadcaster: Arc::new(A2aEventBroadcaster::new()),
            agent_card_config: config,
        }
    }
}

/// Create the A2A router with all endpoints
///
/// # Arguments
///
/// * `state` - Shared A2A state containing runtime, task store, and configuration
///
/// # Returns
///
/// An Axum router configured with all A2A endpoints
pub fn a2a_router<T: ToolRegistry + Clone + Send + Sync + 'static>(state: A2aState<T>) -> Router {
    // Public endpoints (no auth required)
    let public_routes = Router::new()
        .route("/agent-card", get(get_agent_card::<T>))
        .route("/health", get(a2a_health_check));

    // Protected endpoints (auth required)
    let protected_routes = Router::new()
        .route("/tasks", post(create_task::<T>))
        .route("/tasks/{task_id}", get(get_task::<T>))
        .route("/tasks/{task_id}", delete(cancel_task::<T>))
        .route("/tasks/{task_id}/messages", post(send_task_message::<T>))
        .route("/events", get(events_stream::<T>))
        .route_layer(middleware::from_fn(require_auth));

    Router::new()
        .nest("/a2a", public_routes.merge(protected_routes))
        .with_state(state)
}

/// A2A-specific health check endpoint
///
/// Returns health status specific to the A2A protocol implementation.
pub async fn a2a_health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "healthy",
        "protocol": "a2a",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "capabilities": {
            "streaming": true,
            "push_notifications": false,
            "task_cancellation": true
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_agent_card_config() {
        let config = A2aAgentCardConfig::default();
        assert_eq!(config.name, "Skreaver Agent");
        assert!(config.supports_streaming);
        assert!(!config.supports_push_notifications);
    }

    #[test]
    fn test_custom_agent_card_config() {
        let config = A2aAgentCardConfig {
            name: "Custom Agent".to_string(),
            description: "A custom agent".to_string(),
            base_url: "https://example.com".to_string(),
            version: "1.0.0".to_string(),
            supports_streaming: false,
            supports_push_notifications: true,
        };

        assert_eq!(config.name, "Custom Agent");
        assert!(!config.supports_streaming);
        assert!(config.supports_push_notifications);
    }
}
