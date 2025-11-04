//! Queue metrics HTTP handlers
//!
//! This module provides endpoints for monitoring queue metrics and backpressure status.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use skreaver_tools::ToolRegistry;

use crate::runtime::{
    HttpAgentRuntime,
    backpressure::QueueMetrics,
    types::{ErrorResponse, QueueMetricsResponse},
};

/// GET /agents/{agent_id}/queue/metrics - Get agent-specific queue metrics
#[utoipa::path(
    get,
    path = "/agents/{agent_id}/queue/metrics",
    params(
        ("agent_id" = String, Path, description = "Agent identifier")
    ),
    responses(
        (status = 200, description = "Agent queue metrics", body = QueueMetricsResponse),
        (status = 404, description = "Agent not found", body = ErrorResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn get_agent_queue_metrics<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
    Path(agent_id): Path<String>,
) -> Result<Json<QueueMetricsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Parse and verify agent ID
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

    // Verify agent exists
    {
        let agents = runtime.agents.read().await;
        if !agents.contains_key(&parsed_id) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent_not_found".to_string(),
                    message: format!("Agent with ID '{}' not found", agent_id),
                    details: None,
                }),
            ));
        }
    }

    let metrics = runtime
        .backpressure_manager
        .get_agent_metrics(&agent_id)
        .await
        .unwrap_or(QueueMetrics {
            queue_size: 0,
            active_requests: 0,
            total_processed: 0,
            total_timeouts: 0,
            total_rejections: 0,
            avg_processing_time_ms: 0.0,
            load_factor: 0.0,
        });

    Ok(Json(QueueMetricsResponse {
        agent_id: Some(agent_id),
        queue_size: metrics.queue_size,
        active_requests: metrics.active_requests,
        total_processed: metrics.total_processed,
        total_timeouts: metrics.total_timeouts,
        total_rejections: metrics.total_rejections,
        avg_processing_time_ms: metrics.avg_processing_time_ms,
        load_factor: metrics.load_factor,
        timestamp: chrono::Utc::now(),
    }))
}

/// GET /queue/metrics - Get global queue metrics
#[utoipa::path(
    get,
    path = "/queue/metrics",
    responses(
        (status = 200, description = "Global queue metrics", body = QueueMetricsResponse),
        (status = 401, description = "Authentication required", body = crate::runtime::auth::AuthError)
    ),
    security(
        ("api_key" = []),
        ("bearer_auth" = [])
    )
)]
pub async fn get_global_queue_metrics<T: ToolRegistry + Clone + Send + Sync>(
    State(runtime): State<HttpAgentRuntime<T>>,
) -> Json<QueueMetricsResponse> {
    let metrics = runtime.backpressure_manager.get_global_metrics().await;

    Json(QueueMetricsResponse {
        agent_id: None,
        queue_size: metrics.queue_size,
        active_requests: metrics.active_requests,
        total_processed: metrics.total_processed,
        total_timeouts: metrics.total_timeouts,
        total_rejections: metrics.total_rejections,
        avg_processing_time_ms: metrics.avg_processing_time_ms,
        load_factor: metrics.load_factor,
        timestamp: chrono::Utc::now(),
    })
}
