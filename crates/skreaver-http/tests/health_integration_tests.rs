//! Integration tests for health check endpoints
//!
//! Tests the /health and /ready endpoints with various runtime configurations

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use skreaver_http::runtime::{HttpAgentRuntime, HttpRuntimeConfig};
use skreaver_tools::InMemoryToolRegistry;
use tower::ServiceExt;

/// Helper to create test app
fn create_test_app() -> axum::Router {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);
    runtime.router_with_config(HttpRuntimeConfig::default())
}

#[tokio::test]
async fn test_health_endpoint_returns_ok() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "healthy");
    assert_eq!(json["service"], "skreaver-http-runtime");
    assert!(json["version"].is_string());
    assert!(json["uptime_seconds"].is_number());
    assert!(json["memory_mb"].is_number());
    assert!(json["timestamp"].is_string());
}

#[tokio::test]
async fn test_health_endpoint_memory_tracking() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Memory should be a number >= 0
    let memory_mb = json["memory_mb"]
        .as_f64()
        .expect("memory_mb should be a number");
    assert!(memory_mb >= 0.0, "Memory usage should be non-negative");
}

#[tokio::test]
async fn test_ready_endpoint_returns_health_status() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // May return 200 (healthy) or 503 (degraded/unhealthy)
    // depending on configuration
    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::SERVICE_UNAVAILABLE,
        "Expected 200 or 503, got {}",
        response.status()
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Status can be either a string "Healthy" or an object like {"Degraded": {"reason": "..."}}
    assert!(json["status"].is_string() || json["status"].is_object());
    assert!(json["components"].is_object());
    assert!(json["uptime_seconds"].is_number());
    assert!(json["timestamp"].is_string());
}

#[tokio::test]
async fn test_ready_endpoint_component_checks() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    let components = json["components"]
        .as_object()
        .expect("components should be an object");

    // Verify expected components are present
    assert!(
        components.contains_key("http_runtime"),
        "Should check http_runtime"
    );
    assert!(components.contains_key("security"), "Should check security");
    assert!(components.contains_key("memory"), "Should check memory");
    assert!(
        components.contains_key("system_resources"),
        "Should check system_resources"
    );

    // Each component should have a status (string "Healthy" or object {"Degraded": ...} or {"Unhealthy": ...})
    for (name, component) in components.iter() {
        let status_value = &component["status"];
        assert!(
            status_value.is_string() || status_value.is_object(),
            "{} should have status field",
            name
        );

        // Check it's a valid status type
        if status_value.is_string() {
            assert_eq!(
                status_value.as_str().unwrap(),
                "Healthy",
                "{} string status should be 'Healthy'",
                name
            );
        } else if status_value.is_object() {
            let has_degraded = status_value.get("Degraded").is_some();
            let has_unhealthy = status_value.get("Unhealthy").is_some();
            assert!(
                has_degraded || has_unhealthy,
                "{} object status should have Degraded or Unhealthy",
                name
            );
        }
    }
}

#[tokio::test]
async fn test_ready_endpoint_security_component_details() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    let security = &json["components"]["security"];

    // Security component should include metadata about configuration
    assert!(
        security["metadata"].is_object(),
        "Security should have metadata"
    );
    let metadata = security["metadata"].as_object().unwrap();

    assert!(
        metadata.contains_key("fs_paths_configured"),
        "Should track filesystem paths"
    );
    assert!(
        metadata.contains_key("http_domains_allowed"),
        "Should track HTTP domains"
    );
    assert!(
        metadata.contains_key("max_memory_mb"),
        "Should track memory limits"
    );
}

#[tokio::test]
async fn test_metrics_endpoint_returns_prometheus_format() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Metrics endpoint should return OK
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();

    // Should be in Prometheus text format (contains # HELP or # TYPE)
    assert!(
        text.contains("# HELP") || text.contains("# TYPE") || text.is_empty(),
        "Metrics should be in Prometheus format or empty"
    );
}

#[tokio::test]
async fn test_health_endpoints_uptime_increases() {
    // First request
    let app1 = create_test_app();
    let response1 = app1
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body1 = axum::body::to_bytes(response1.into_body(), usize::MAX)
        .await
        .unwrap();
    let json1: Value = serde_json::from_slice(&body1).unwrap();
    let uptime1 = json1["uptime_seconds"].as_u64().unwrap();

    // Wait a bit
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Second request
    let app2 = create_test_app();
    let response2 = app2
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body2 = axum::body::to_bytes(response2.into_body(), usize::MAX)
        .await
        .unwrap();
    let json2: Value = serde_json::from_slice(&body2).unwrap();
    let uptime2 = json2["uptime_seconds"].as_u64().unwrap();

    // Uptime should increase (or stay the same due to timing)
    assert!(uptime2 >= uptime1, "Uptime should not decrease");
}

#[tokio::test]
async fn test_ready_endpoint_response_time_tracking() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    let components = json["components"].as_object().unwrap();

    // Each component should track response time
    for (name, component) in components.iter() {
        let response_time = component["response_time_ms"].as_u64();
        assert!(
            response_time.is_some(),
            "{} should have response_time_ms",
            name
        );

        let time = response_time.unwrap();
        assert!(
            time < 1000,
            "{} response time should be reasonable: {}ms",
            name,
            time
        );
    }
}
