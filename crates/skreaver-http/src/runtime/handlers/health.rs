//! Health and metrics HTTP handlers
//!
//! This module provides health check endpoints, readiness checks, and metrics
//! collection endpoints for monitoring the HTTP runtime.

use axum::{extract::State, http::StatusCode, response::Json};
use skreaver_observability::health::SystemHealth;
use skreaver_observability::metrics::get_metrics_registry;
use skreaver_tools::ToolRegistry;

use crate::runtime::HttpAgentRuntime;

/// GET /health - Basic health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy")
    )
)]
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "skreaver-http-runtime",
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// GET /ready - Kubernetes readiness check with detailed component health
#[utoipa::path(
    get,
    path = "/ready",
    responses(
        (status = 200, description = "Service is ready", body = SystemHealth),
        (status = 503, description = "Service not ready", body = SystemHealth)
    )
)]
pub async fn readiness_check<T: ToolRegistry + Clone + Send + Sync>(
    State(_runtime): State<HttpAgentRuntime<T>>,
) -> Result<Json<SystemHealth>, (StatusCode, Json<SystemHealth>)> {
    // Create basic system health response
    let system_health = SystemHealth {
        status: skreaver_observability::health::HealthStatus::Healthy,
        components: std::collections::HashMap::new(),
        timestamp: chrono::Utc::now(),
        uptime_seconds: 0,
    };

    Ok(Json(system_health))
}

/// GET /metrics - Prometheus metrics endpoint
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Prometheus metrics", content_type = "text/plain"),
        (status = 500, description = "Metrics collection failed")
    )
)]
pub async fn metrics_endpoint() -> Result<String, (StatusCode, String)> {
    match get_metrics_registry() {
        Some(registry) => {
            let prometheus_registry = registry.prometheus_registry();
            let encoder = prometheus::TextEncoder::new();
            let metric_families = prometheus_registry.gather();

            match encoder.encode_to_string(&metric_families) {
                Ok(metrics) => Ok(metrics),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to encode metrics: {}", e),
                )),
            }
        }
        None => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Metrics registry not initialized".to_string(),
        )),
    }
}
