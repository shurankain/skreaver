//! Agent management HTTP handlers
//!
//! This module provides CRUD operations for managing agents through HTTP endpoints.

use axum::{extract::{Path, State}, http::StatusCode, response::Json};
use skreaver_tools::ToolRegistry;

use crate::runtime::{HttpAgentRuntime, types::{CreateAgentRequest, CreateAgentResponse, AgentStatus, AgentsListResponse, ErrorResponse}};

/// GET /agents - List all agents
#[utoipa::path(
    get,
    path = "/agents",
    responses(
        (status = 200, description = "List of all agents", body = AgentsListResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn list_agents<T: ToolRegistry + Clone + Send + Sync>(
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
            last_activity: None, // TODO: Track last activity
        })
        .collect();

    Ok(Json(AgentsListResponse {
        total: agent_statuses.len(),
        agents: agent_statuses,
    }))
}

/// POST /agents - Create a new agent
#[utoipa::path(
    post,
    path = "/agents",
    request_body = CreateAgentRequest,
    responses(
        (status = 201, description = "Agent created successfully", body = CreateAgentResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError),
        (status = 501, description = "Not implemented", body = ErrorResponse)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn create_agent<T: ToolRegistry + Clone + Send + Sync>(
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
            details: None,
        }),
    ))
}

/// GET /agents/{agent_id}/status - Get agent status
#[utoipa::path(
    get,
    path = "/agents/{agent_id}/status",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    responses(
        (status = 200, description = "Agent status information", body = AgentStatus),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn get_agent_status<T: ToolRegistry + Clone + Send + Sync>(
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
            last_activity: None, // TODO: Track last activity
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "agent_not_found".to_string(),
                message: format!("Agent with ID '{}' not found", agent_id),
                details: None,
            }),
        )),
    }
}

/// DELETE /agents/{agent_id} - Remove an agent
#[utoipa::path(
    delete,
    path = "/agents/{agent_id}",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    responses(
        (status = 204, description = "Agent deleted successfully"),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn delete_agent<T: ToolRegistry + Clone + Send + Sync>(
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
                details: None,
            }),
        )),
    }
}