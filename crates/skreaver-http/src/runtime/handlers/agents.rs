//! Agent management HTTP handlers
//!
//! This module provides CRUD operations for managing agents through HTTP endpoints.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use skreaver_tools::ToolRegistry;

use crate::runtime::{
    AgentFactoryError, HttpAgentRuntime,
    api_types::CreateAgentRequest,
    types::{AgentStatus, AgentsListResponse, CreateAgentResponse, ErrorResponse},
};

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
pub async fn list_agents<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(runtime): State<HttpAgentRuntime<T>>,
) -> Result<Json<AgentsListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let agents = runtime.agents.read().await;

    // Collect agent statuses with actual creation time and last activity
    let mut agent_statuses = Vec::new();
    for (id, instance) in agents.iter() {
        agent_statuses.push(AgentStatus {
            agent_id: id.to_string(),
            agent_type: instance.coordinator.get_agent_type().to_string(),
            status: "running".to_string(),
            created_at: instance.created_at,
            last_activity: Some(instance.get_last_activity().await),
        });
    }

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
        (status = 500, description = "Agent creation failed", body = ErrorResponse)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn create_agent<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Json(request): Json<CreateAgentRequest>,
) -> Result<Json<CreateAgentResponse>, (StatusCode, Json<ErrorResponse>)> {
    match runtime.create_agent(request.spec, None).await {
        Ok(response) => {
            // Convert the factory response to the HTTP response format
            Ok(Json(CreateAgentResponse {
                agent_id: response.agent_id,
                agent_type: response.spec.agent_type.to_string(),
                status: response.status.simple_name().to_string(),
            }))
        }
        Err(AgentFactoryError::UnknownAgentType(agent_type)) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unknown_agent_type".to_string(),
                message: format!("Unknown agent type: {}", agent_type),
                details: None,
            }),
        )),
        Err(AgentFactoryError::InvalidConfiguration { field, reason }) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_configuration".to_string(),
                message: format!("Invalid configuration for field '{}': {}", field, reason),
                details: None,
            }),
        )),
        Err(AgentFactoryError::CreationFailed { agent_type, reason }) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "creation_failed".to_string(),
                message: format!("Failed to create {} agent: {}", agent_type, reason),
                details: None,
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "agent_creation_failed".to_string(),
                message: e.to_string(),
                details: None,
            }),
        )),
    }
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
pub async fn get_agent_status<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
) -> Result<Json<AgentStatus>, (StatusCode, Json<ErrorResponse>)> {
    let parsed_id = match skreaver_core::AgentId::parse(&agent_id) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid_agent_id".to_string(),
                    message: format!("Invalid agent ID: {}", e),
                    details: None,
                }),
            ));
        }
    };

    let agents = runtime.agents.read().await;

    match agents.get(&parsed_id) {
        Some(instance) => {
            let last_activity = instance.get_last_activity().await;
            Ok(Json(AgentStatus {
                agent_id, // No need to clone, we own it
                agent_type: instance.coordinator.get_agent_type().to_string(),
                status: "running".to_string(),
                created_at: instance.created_at,
                last_activity: Some(last_activity),
            }))
        }
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
pub async fn delete_agent<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match runtime.remove_agent(&agent_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(AgentFactoryError::AgentNotFound(_)) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "agent_not_found".to_string(),
                message: format!("Agent with ID '{}' not found", agent_id),
                details: None,
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "deletion_failed".to_string(),
                message: e.to_string(),
                details: None,
            }),
        )),
    }
}
