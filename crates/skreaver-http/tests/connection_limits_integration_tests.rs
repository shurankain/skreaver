//! Integration tests for HTTP connection limits

use serial_test::serial;
use skreaver_http::runtime::{
    ConnectionLimitConfig, HttpAgentRuntime, HttpRuntimeConfigBuilder,
    connection_limits::MissingConnectInfoBehavior,
};
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
            mode: skreaver_http::runtime::connection_limits::ConnectionLimitMode::Enabled,
            missing_connect_info_behavior: MissingConnectInfoBehavior::UseFallback(
                "127.0.0.1".parse().unwrap(),
            ),
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
            mode: skreaver_http::runtime::connection_limits::ConnectionLimitMode::Disabled,
            missing_connect_info_behavior: MissingConnectInfoBehavior::UseFallback(
                "127.0.0.1".parse().unwrap(),
            ),
        })
        .build()
        .unwrap();

    let runtime = HttpAgentRuntime::with_config(registry, config);

    // Even when disabled, tracker is created (just doesn't track)
    assert_eq!(runtime.connection_tracker.active_connections(), 0);
}

#[tokio::test]
#[serial]
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
#[serial]
async fn test_missing_connect_info_behavior_env() {
    // Test reject behavior
    unsafe {
        std::env::set_var("SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR", "reject");
    }

    let registry = InMemoryToolRegistry::new();
    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build config");

    let runtime = HttpAgentRuntime::with_config(registry, config);

    // Verify behavior is set correctly
    match runtime
        .connection_tracker
        .config()
        .missing_connect_info_behavior
    {
        MissingConnectInfoBehavior::Reject => {} // Expected
        _ => panic!("Expected Reject behavior"),
    }

    unsafe {
        std::env::remove_var("SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR");
    }
}

#[tokio::test]
#[serial]
async fn test_missing_connect_info_behavior_fallback() {
    // Test fallback behavior with IP
    unsafe {
        std::env::set_var(
            "SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR",
            "fallback:10.0.0.1",
        );
    }

    let registry = InMemoryToolRegistry::new();
    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build config");

    let runtime = HttpAgentRuntime::with_config(registry, config);

    // Verify fallback IP is set correctly
    match &runtime
        .connection_tracker
        .config()
        .missing_connect_info_behavior
    {
        MissingConnectInfoBehavior::UseFallback(ip) => {
            assert_eq!(ip.to_string(), "10.0.0.1");
        }
        other => panic!("Expected UseFallback behavior, got: {:?}", other),
    }

    unsafe {
        std::env::remove_var("SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR");
    }
}

#[tokio::test]
#[serial]
async fn test_missing_connect_info_behavior_disable_per_ip() {
    // Test disable per-IP limits behavior
    unsafe {
        std::env::set_var(
            "SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR",
            "disable_per_ip",
        );
    }

    let registry = InMemoryToolRegistry::new();
    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build config");

    let runtime = HttpAgentRuntime::with_config(registry, config);

    // Verify behavior is set correctly
    match runtime
        .connection_tracker
        .config()
        .missing_connect_info_behavior
    {
        MissingConnectInfoBehavior::DisablePerIpLimits => {} // Expected
        _ => panic!("Expected DisablePerIpLimits behavior"),
    }

    unsafe {
        std::env::remove_var("SKREAVER_CONNECTION_LIMIT_MISSING_BEHAVIOR");
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
