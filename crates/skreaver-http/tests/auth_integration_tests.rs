//! Integration tests for HTTP authentication middleware
//!
//! These tests verify that:
//! - Protected endpoints require authentication
//! - Public endpoints work without authentication
//! - JWT and API Key authentication both work
//! - Invalid credentials are rejected

use axum::{
    body::Body,
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use skreaver_http::runtime::{HttpAgentRuntime, HttpRuntimeConfig};
use skreaver_tools::InMemoryToolRegistry;
use tower::ServiceExt; // for `oneshot` method

/// Helper to create test app
fn create_test_app() -> axum::Router {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);
    runtime.router_with_config(HttpRuntimeConfig::default())
}

/// Helper to create a valid JWT token for testing
fn create_test_jwt() -> String {
    use skreaver_http::runtime::auth::create_jwt_token;

    create_jwt_token(
        "test-user".to_string(),
        vec!["read".to_string(), "write".to_string()],
    )
    .expect("Failed to create test JWT")
}

#[tokio::test]
async fn test_public_endpoints_work_without_auth() {
    let app = create_test_app();

    // Test /health endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/health should be accessible without auth"
    );

    // Test /ready endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/ready should be accessible without auth"
    );

    // Test /metrics endpoint
    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/metrics should be accessible without auth"
    );
}

#[tokio::test]
async fn test_protected_endpoints_reject_unauthenticated_requests() {
    let app = create_test_app();

    // Test /agents endpoint (GET)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "/agents should require authentication"
    );

    // Test /agents endpoint (POST)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agents")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"test","type":"echo"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "POST /agents should require authentication"
    );

    // Test /queue/metrics endpoint
    let response = app
        .oneshot(
            Request::builder()
                .uri("/queue/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "/queue/metrics should require authentication"
    );
}

#[tokio::test]
async fn test_protected_endpoints_accept_valid_jwt() {
    let app = create_test_app();
    let token = create_test_jwt();

    // Test /agents endpoint with valid JWT
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                .header(AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/agents should accept valid JWT token"
    );

    // Test /queue/metrics with valid JWT
    let response = app
        .oneshot(
            Request::builder()
                .uri("/queue/metrics")
                .header(AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/queue/metrics should accept valid JWT token"
    );
}

#[tokio::test]
async fn test_protected_endpoints_accept_valid_api_key() {
    let app = create_test_app();

    // Use the default test API key from auth.rs
    let api_key = "sk-test-key-123";

    // Test /agents endpoint with valid API key via Authorization header
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                .header(AUTHORIZATION, format!("Bearer {}", api_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/agents should accept valid API key via Authorization header"
    );

    // Test /agents endpoint with valid API key via X-API-Key header
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                .header("X-API-Key", api_key)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "/agents should accept valid API key via X-API-Key header"
    );
}

#[tokio::test]
async fn test_protected_endpoints_reject_invalid_jwt() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                .header(AUTHORIZATION, "Bearer invalid-token-12345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "/agents should reject invalid JWT token"
    );
}

#[tokio::test]
async fn test_protected_endpoints_reject_invalid_api_key() {
    let app = create_test_app();

    // Test with invalid API key via Authorization header
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                .header(AUTHORIZATION, "Bearer sk-invalid-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "/agents should reject invalid API key via Authorization header"
    );

    // Test with invalid API key via X-API-Key header
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                .header("X-API-Key", "sk-invalid-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "/agents should reject invalid API key via X-API-Key header"
    );
}

#[tokio::test]
async fn test_auth_token_endpoint_is_public() {
    let app = create_test_app();

    // The /auth/token endpoint should be public (no auth required)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/token")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"user_id":"test","permissions":["read"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should not be UNAUTHORIZED (it's public)
    assert_ne!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "/auth/token should be accessible without authentication"
    );
}

#[tokio::test]
async fn test_missing_authorization_header_returns_401() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                // No Authorization header
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Missing Authorization header should return 401"
    );
}

#[tokio::test]
async fn test_malformed_authorization_header_returns_401() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/agents")
                .header(AUTHORIZATION, "InvalidFormat token123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Malformed Authorization header should return 401"
    );
}
