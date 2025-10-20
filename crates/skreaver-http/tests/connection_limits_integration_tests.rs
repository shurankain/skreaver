//! Integration tests for HTTP connection limits

use skreaver_http::runtime::{ConnectionLimitConfig, HttpAgentRuntime, HttpRuntimeConfigBuilder};
use skreaver_tools::InMemoryToolRegistry;

// Test helper - no agent needed for connection limit tests

#[tokio::test]
async fn test_connection_limits_enabled_by_default() {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);

    // Check that connection tracker is created with defaults
    assert_eq!(runtime.connection_tracker.active_connections(), 0);

    let stats = runtime.connection_tracker.stats().await;
    assert_eq!(stats.max_connections, 10_000);
    assert_eq!(stats.max_connections_per_ip, 100);
}

#[tokio::test]
async fn test_connection_limits_custom_config() {
    let registry = InMemoryToolRegistry::new();

    let config = HttpRuntimeConfigBuilder::new()
        .connection_limits(ConnectionLimitConfig {
            max_connections: 50,
            max_connections_per_ip: 5,
            enabled: true,
        })
        .build()
        .unwrap();

    let runtime = HttpAgentRuntime::with_config(registry, config);

    let stats = runtime.connection_tracker.stats().await;
    assert_eq!(stats.max_connections, 50);
    assert_eq!(stats.max_connections_per_ip, 5);
}

#[tokio::test]
async fn test_connection_limits_can_be_disabled() {
    let registry = InMemoryToolRegistry::new();

    let config = HttpRuntimeConfigBuilder::new()
        .connection_limits(ConnectionLimitConfig {
            max_connections: 10,
            max_connections_per_ip: 1,
            enabled: false,  // Disabled
        })
        .build()
        .unwrap();

    let runtime = HttpAgentRuntime::with_config(registry, config);

    // Even when disabled, tracker is created (just doesn't track)
    assert_eq!(runtime.connection_tracker.active_connections(), 0);
}

#[tokio::test]
async fn test_connection_limits_from_env() {
    // Set environment variables
    unsafe {
        std::env::set_var("SKREAVER_CONNECTION_LIMIT_MAX", "500");
        std::env::set_var("SKREAVER_CONNECTION_LIMIT_PER_IP", "10");
        std::env::set_var("SKREAVER_CONNECTION_LIMIT_ENABLED", "true");
    }

    let registry = InMemoryToolRegistry::new();
    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build config");

    let runtime = HttpAgentRuntime::with_config(registry, config);

    let stats = runtime.connection_tracker.stats().await;
    assert_eq!(stats.max_connections, 500);
    assert_eq!(stats.max_connections_per_ip, 10);

    // Clean up
    unsafe {
        std::env::remove_var("SKREAVER_CONNECTION_LIMIT_MAX");
        std::env::remove_var("SKREAVER_CONNECTION_LIMIT_PER_IP");
        std::env::remove_var("SKREAVER_CONNECTION_LIMIT_ENABLED");
    }
}

#[tokio::test]
async fn test_router_compiles_with_connection_limits() {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);

    // Just verify the router can be created (compilation test)
    let _router = runtime.router();

    // If we get here, the middleware integration works successfully
}
