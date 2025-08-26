//! # HTTP Runtime
//!
//! This module provides an HTTP server runtime for Skreaver agents, enabling
//! remote interaction with agents through RESTful APIs. The runtime manages
//! agent lifecycle, handles observations, and provides status information.

use crate::{agent::Agent, runtime::Coordinator, tool::registry::ToolRegistry};
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

/// Unique identifier for an agent instance
pub type AgentId = String;

/// HTTP server state containing all running agents
#[derive(Clone)]
pub struct HttpAgentRuntime<T: ToolRegistry> {
    pub agents: Arc<RwLock<HashMap<AgentId, AgentInstance>>>,
    pub tool_registry: Arc<T>,
}

/// Container for agent and its coordinator
pub struct AgentInstance {
    pub coordinator: Box<dyn CoordinatorTrait + Send + Sync>,
}

/// Trait to allow dynamic dispatch of coordinators
pub trait CoordinatorTrait {
    fn step(&mut self, input: String) -> String;
    fn get_agent_type(&self) -> &'static str;
}

impl<A: Agent + Send + Sync + 'static, T: ToolRegistry + Clone> CoordinatorTrait
    for Coordinator<A, T>
where
    A::Observation: From<String> + std::fmt::Display,
    A::Action: ToString,
{
    fn step(&mut self, input: String) -> String {
        let observation = A::Observation::from(input);
        let action = self.step(observation);
        action.to_string()
    }

    fn get_agent_type(&self) -> &'static str {
        std::any::type_name::<A>()
    }
}

/// Request body for creating a new agent
#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub agent_type: String,
    pub name: Option<String>,
}

/// Response for agent creation
#[derive(Debug, Serialize)]
pub struct CreateAgentResponse {
    pub agent_id: String,
    pub agent_type: String,
    pub status: String,
}

/// Request body for sending observations to an agent
#[derive(Debug, Deserialize)]
pub struct ObserveRequest {
    pub input: String,
}

/// Response from agent observation
#[derive(Debug, Serialize)]
pub struct ObserveResponse {
    pub agent_id: String,
    pub response: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Agent status information
#[derive(Debug, Serialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub agent_type: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// List of all agents
#[derive(Debug, Serialize)]
pub struct AgentsListResponse {
    pub agents: Vec<AgentStatus>,
    pub total: usize,
}

/// Error response format
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

impl<T: ToolRegistry + Clone + Send + Sync + 'static> HttpAgentRuntime<T> {
    /// Create a new HTTP agent runtime
    pub fn new(tool_registry: T) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            tool_registry: Arc::new(tool_registry),
        }
    }

    /// Create the Axum router with all endpoints
    pub fn router(self) -> Router {
        Router::new()
            .route("/agents", get(list_agents).post(create_agent))
            .route("/agents/:agent_id/status", get(get_agent_status))
            .route("/agents/:agent_id/observe", post(observe_agent))
            .route("/agents/:agent_id", axum::routing::delete(delete_agent))
            .route("/health", get(health_check))
            .with_state(self)
    }

    /// Add an agent instance to the runtime
    pub async fn add_agent<A>(&self, agent_id: String, agent: A) -> Result<(), String>
    where
        A: Agent + Send + Sync + 'static,
        A::Observation: From<String> + std::fmt::Display,
        A::Action: ToString,
    {
        let coordinator = Coordinator::new(agent, (*self.tool_registry).clone());
        let instance = AgentInstance {
            coordinator: Box::new(coordinator),
        };

        let mut agents = self.agents.write().await;
        agents.insert(agent_id, instance);
        Ok(())
    }
}

/// GET /agents - List all agents
async fn list_agents<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
) -> Result<Json<AgentsListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let agents = runtime.agents.read().await;

    let agent_statuses: Vec<AgentStatus> = agents
        .iter()
        .map(|(id, instance)| AgentStatus {
            agent_id: id.clone(),
            agent_type: instance.coordinator.get_agent_type().to_string(),
            status: "running".to_string(),
            created_at: chrono::Utc::now(), // TODO: Track actual creation time
        })
        .collect();

    Ok(Json(AgentsListResponse {
        total: agent_statuses.len(),
        agents: agent_statuses,
    }))
}

/// POST /agents - Create a new agent
async fn create_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(_runtime): State<HttpAgentRuntime<T>>,
    Json(_request): Json<CreateAgentRequest>,
) -> Result<Json<CreateAgentResponse>, (StatusCode, Json<ErrorResponse>)> {
    // For now, we'll return an error since we need a factory pattern to create agents dynamically
    Err((
        StatusCode::NOT_IMPLEMENTED,
        Json(ErrorResponse {
            error: "not_implemented".to_string(),
            message:
                "Dynamic agent creation not yet implemented. Use runtime.add_agent() directly."
                    .to_string(),
        }),
    ))
}

/// GET /agents/{agent_id}/status - Get agent status
async fn get_agent_status<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentStatus>, (StatusCode, Json<ErrorResponse>)> {
    let agents = runtime.agents.read().await;

    match agents.get(&agent_id) {
        Some(instance) => Ok(Json(AgentStatus {
            agent_id: agent_id.clone(),
            agent_type: instance.coordinator.get_agent_type().to_string(),
            status: "running".to_string(),
            created_at: chrono::Utc::now(), // TODO: Track actual creation time
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "agent_not_found".to_string(),
                message: format!("Agent with ID '{}' not found", agent_id),
            }),
        )),
    }
}

/// POST /agents/{agent_id}/observe - Send observation to agent
async fn observe_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
    Json(request): Json<ObserveRequest>,
) -> Result<Json<ObserveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut agents = runtime.agents.write().await;

    match agents.get_mut(&agent_id) {
        Some(instance) => {
            let response = instance.coordinator.step(request.input);

            Ok(Json(ObserveResponse {
                agent_id: agent_id.clone(),
                response,
                timestamp: chrono::Utc::now(),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "agent_not_found".to_string(),
                message: format!("Agent with ID '{}' not found", agent_id),
            }),
        )),
    }
}

/// DELETE /agents/{agent_id} - Remove an agent
async fn delete_agent<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let mut agents = runtime.agents.write().await;

    match agents.remove(&agent_id) {
        Some(_) => Ok(StatusCode::NO_CONTENT),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "agent_not_found".to_string(),
                message: format!("Agent with ID '{}' not found", agent_id),
            }),
        )),
    }
}

/// GET /health - Health check endpoint
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "skreaver-http-runtime",
        "timestamp": chrono::Utc::now()
    }))
}
