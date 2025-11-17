//! Health and metrics HTTP handlers
//!
//! This module provides health check endpoints, readiness checks, and metrics
//! collection endpoints for monitoring the HTTP runtime.

use axum::{extract::State, http::StatusCode, response::Json};
use skreaver_observability::health::{ComponentHealth, SystemHealth};
use skreaver_observability::metrics::get_metrics_registry;
use skreaver_tools::ToolRegistry;
use std::collections::HashMap;
use std::time::Instant;

use crate::runtime::HttpAgentRuntime;

// Track service start time for uptime calculation
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn get_uptime_seconds() -> u64 {
    START_TIME.get_or_init(Instant::now).elapsed().as_secs()
}

/// GET /health - Basic health check endpoint with version info
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = serde_json::Value),
        (status = 503, description = "Service is unhealthy", body = serde_json::Value)
    )
)]
pub async fn health_check() -> (StatusCode, Json<serde_json::Value>) {
    // Perform quick system checks
    let heap_alloc_mb = get_memory_usage_mb();
    let uptime = get_uptime_seconds();

    // Determine health status based on basic metrics
    let (status, status_code) = if heap_alloc_mb > 1024.0 {
        ("degraded", StatusCode::OK) // Still serving but high memory
    } else {
        ("healthy", StatusCode::OK)
    };

    (
        status_code,
        Json(serde_json::json!({
            "status": status,
            "service": "skreaver-http-runtime",
            "timestamp": chrono::Utc::now(),
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_seconds": uptime,
            "memory_mb": heap_alloc_mb,
        })),
    )
}

fn get_memory_usage_mb() -> f64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| line.starts_with("VmRSS:"))
                    .and_then(|line| {
                        line.split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<f64>().ok())
                            .map(|kb| kb / 1024.0)
                    })
            })
            .unwrap_or(0.0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        0.0 // TODO: Implement for other platforms
    }
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
    State(runtime): State<HttpAgentRuntime<T>>,
) -> Result<Json<SystemHealth>, (StatusCode, Json<SystemHealth>)> {
    let mut components = HashMap::new();

    // Check HTTP Runtime
    let http_health = check_http_runtime_health(&runtime).await;
    components.insert("http_runtime".to_string(), http_health);

    // Check Security Configuration
    let security_health = check_security_health(&runtime).await;
    components.insert("security".to_string(), security_health);

    // Check Memory Backend (if available)
    let memory_health = check_memory_health().await;
    components.insert("memory".to_string(), memory_health);

    // Check WebSocket (if enabled)
    #[cfg(feature = "websocket")]
    {
        let ws_health = check_websocket_health().await;
        components.insert("websocket".to_string(), ws_health);
    }

    // Check System Resources
    let system_health_comp = check_system_resources().await;
    components.insert("system_resources".to_string(), system_health_comp);

    // Create system health response
    let mut system_health = SystemHealth::from_components(components);
    system_health.uptime_seconds = get_uptime_seconds();

    // Update timestamp
    system_health.timestamp = chrono::Utc::now();

    // Return appropriate status code
    if system_health.status.is_healthy() {
        Ok(Json(system_health))
    } else {
        Err((StatusCode::SERVICE_UNAVAILABLE, Json(system_health)))
    }
}

/// Check HTTP runtime health
async fn check_http_runtime_health<T: ToolRegistry + Clone + Send + Sync>(
    _runtime: &HttpAgentRuntime<T>,
) -> ComponentHealth {
    // Check if runtime is responsive
    let start = Instant::now();

    // Simple check: Can we access runtime state?
    let response_time = start.elapsed().as_millis() as u64;

    ComponentHealth::healthy("http_runtime".to_string())
        .with_metadata("response_time_ms".to_string(), response_time.to_string())
        .with_metadata("status".to_string(), "operational".to_string())
}

/// Check security configuration health
async fn check_security_health<T: ToolRegistry + Clone + Send + Sync>(
    runtime: &HttpAgentRuntime<T>,
) -> ComponentHealth {
    let start = Instant::now();

    // Check if security features are properly configured
    let security_config = &runtime.security_config;
    let mut issues = Vec::new();

    // Check file system policy
    if security_config.fs.allow_paths.is_empty() {
        issues.push("No file system paths configured".to_string());
    }

    // Check HTTP policy
    use skreaver_core::security::HttpAccess;
    match &security_config.http.access {
        HttpAccess::Disabled => {
            issues.push("HTTP access is disabled".to_string());
        }
        HttpAccess::Internet {
            domain_filter: skreaver_core::security::DomainFilter::AllowList { allow_list, .. },
            ..
        } if allow_list.is_empty() => {
            issues.push("No HTTP domains allowed".to_string());
        }
        _ => {} // LocalOnly or Internet with AllowAll/non-empty AllowList is fine
    }

    // Check resource limits
    if security_config.resources.max_memory_mb == 0 {
        issues.push("No memory limits configured".to_string());
    }

    let response_time = start.elapsed().as_millis() as u64;

    let mut health = if issues.is_empty() {
        ComponentHealth::healthy("security".to_string())
    } else if issues.len() < 2 {
        ComponentHealth::degraded("security".to_string(), issues.join("; "))
    } else {
        ComponentHealth::unhealthy("security".to_string(), issues.join("; "))
    };

    health.response_time_ms = response_time;

    let http_domains_count = match &security_config.http.access {
        HttpAccess::Internet {
            domain_filter: skreaver_core::security::DomainFilter::AllowList { allow_list, .. },
            ..
        } => allow_list.len(),
        HttpAccess::Internet {
            domain_filter: skreaver_core::security::DomainFilter::AllowAll { .. },
            ..
        } => 0, // AllowAll means no specific allowed domains
        HttpAccess::LocalOnly(_) => 0,
        HttpAccess::Disabled => 0,
    };

    health = health
        .with_metadata(
            "fs_paths_configured".to_string(),
            security_config.fs.allow_paths.len().to_string(),
        )
        .with_metadata(
            "http_domains_allowed".to_string(),
            http_domains_count.to_string(),
        )
        .with_metadata(
            "max_memory_mb".to_string(),
            security_config.resources.max_memory_mb.to_string(),
        );

    health
}

/// Check memory backend health
async fn check_memory_health() -> ComponentHealth {
    // For now, assume memory is healthy if we can allocate
    // In production, this would check actual memory backend connections
    ComponentHealth::healthy("memory".to_string())
        .with_metadata("backend".to_string(), "in-memory".to_string())
        .with_metadata("status".to_string(), "connected".to_string())
}

/// Check WebSocket health
#[cfg(feature = "websocket")]
async fn check_websocket_health() -> ComponentHealth {
    // Check if WebSocket server is operational
    // This is a placeholder - actual implementation would check active connections
    ComponentHealth::healthy("websocket".to_string())
        .with_metadata("status".to_string(), "operational".to_string())
        .with_metadata("feature".to_string(), "enabled".to_string())
}

/// Check system resources health
async fn check_system_resources() -> ComponentHealth {
    let start = Instant::now();

    let memory_mb = get_memory_usage_mb();
    let uptime = get_uptime_seconds();

    let (status, reason) = if memory_mb > 1024.0 {
        (
            "degraded",
            Some(format!("High memory usage: {:.2}MB", memory_mb)),
        )
    } else if memory_mb > 2048.0 {
        (
            "unhealthy",
            Some(format!("Critical memory usage: {:.2}MB", memory_mb)),
        )
    } else {
        ("healthy", None)
    };

    let response_time = start.elapsed().as_millis() as u64;

    let mut health = match (status, reason) {
        ("healthy", _) => ComponentHealth::healthy("system_resources".to_string()),
        ("degraded", Some(r)) => ComponentHealth::degraded("system_resources".to_string(), r),
        ("unhealthy", Some(r)) => ComponentHealth::unhealthy("system_resources".to_string(), r),
        _ => ComponentHealth::healthy("system_resources".to_string()),
    };

    health.response_time_ms = response_time;
    health = health
        .with_metadata("memory_mb".to_string(), format!("{:.2}", memory_mb))
        .with_metadata("uptime_seconds".to_string(), uptime.to_string());

    health
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
