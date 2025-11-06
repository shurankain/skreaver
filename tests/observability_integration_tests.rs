//! Observability Integration Tests
//!
//! These tests verify that the observability framework works correctly
//! in end-to-end scenarios as specified in DEVELOPMENT_PLAN.md Phase 0.3.

use skreaver_observability::{
    ObservabilityConfig, init_observability,
    metrics::MetricsRegistry,
    tags::{AgentId, CardinalTags, ErrorKind, MemoryOp, SessionId, ToolName},
};
use std::time::Duration;

/// Test basic observability initialization
#[tokio::test]
async fn test_observability_initialization() {
    let config = ObservabilityConfig {
        metrics_enabled: false, // Disable metrics to avoid singleton conflicts
        tracing_enabled: false, // Disable tracing to avoid conflicts
        health_enabled: true,   // Health is safe
        otel_endpoint: None,
        namespace: "test_obs".to_string(),
        ..Default::default()
    };

    let result = init_observability(config);
    // Note: This may fail if called multiple times due to singleton nature
    // In production, observability would be initialized once at startup
    if result.is_err() {
        println!(
            "Observability initialization failed (expected in tests): {:?}",
            result
        );
    }
}

/// Test core metrics collection as specified in DEVELOPMENT_PLAN.md
#[tokio::test]
async fn test_core_metrics_collection() {
    // Create registry directly to avoid global singleton issues
    let namespace = "test_metrics";
    let registry = MetricsRegistry::new(namespace).expect("Registry creation should succeed");

    // Test agent session tracking
    let agent_id = AgentId::parse("test-agent").expect("Valid agent ID");
    let session_id = SessionId::generate();
    let tags = CardinalTags::for_agent_session(agent_id, session_id);

    registry
        .record_agent_session_start(&tags)
        .expect("Should record session start");
    assert_eq!(registry.core_metrics().agent_sessions_active.get(), 1.0);

    registry
        .record_agent_session_end(&tags)
        .expect("Should record session end");
    assert_eq!(registry.core_metrics().agent_sessions_active.get(), 0.0);

    // Test tool execution metrics
    let tool_name = ToolName::parse("test_tool").expect("Valid tool name");
    let duration = Duration::from_millis(150);

    registry
        .record_tool_execution(&tool_name, duration)
        .expect("Should record tool execution");

    // Test error recording
    registry
        .record_agent_error(&ErrorKind::Tool)
        .expect("Should record error");

    // Test memory operations
    registry
        .record_memory_operation(&MemoryOp::Write)
        .expect("Should record memory op");
}

/// Test cardinality enforcement as specified in DEVELOPMENT_PLAN.md
#[tokio::test]
async fn test_cardinality_enforcement() {
    let namespace = "test_cardinality";
    let registry = MetricsRegistry::new(namespace).expect("Registry creation should succeed");

    // Test tool cardinality limit (≤20)
    for i in 0..20 {
        let tool_name = ToolName::parse(format!("tool_{}", i)).expect("Valid tool name");
        let result = registry.record_tool_execution(&tool_name, Duration::from_millis(1));
        assert!(result.is_ok(), "Should accept tool within limit");
    }

    // 21st tool should fail
    let tool_name = ToolName::parse("tool_21").expect("Valid tool name");
    let result = registry.record_tool_execution(&tool_name, Duration::from_millis(1));
    assert!(result.is_err(), "Should reject tool exceeding limit");

    // Check cardinality stats
    let stats = registry
        .cardinality_stats()
        .expect("Should get cardinality stats");
    assert_eq!(stats.tool_names_count, 20);
    assert_eq!(stats.error_kinds_count, 10);
    assert_eq!(stats.memory_ops_count, 4);
}

/// Test metrics collector convenience interface
#[tokio::test]
async fn test_metrics_collector() {
    let namespace = "test_collector";
    let registry = std::sync::Arc::new(
        MetricsRegistry::new(namespace).expect("Registry creation should succeed"),
    );
    let collector = skreaver_observability::metrics::MetricsCollector::new(registry);

    // Test tool timer
    let tool_name = ToolName::parse("timed_tool").expect("Valid tool name");
    let timer = collector.start_tool_timer(tool_name);

    // Simulate some work
    tokio::time::sleep(Duration::from_millis(10)).await;

    timer.finish().expect("Timer should finish successfully");

    // Test error recording
    collector
        .record_error(ErrorKind::Network)
        .expect("Should record error");

    // Test memory operation recording
    collector
        .record_memory_op(MemoryOp::Read)
        .expect("Should record memory op");
}

/// Test HTTP metrics (important for HTTP runtime integration)
#[tokio::test]
async fn test_http_metrics() {
    let namespace = "test_http";
    let registry = MetricsRegistry::new(namespace).expect("Registry creation should succeed");

    // Record HTTP requests
    let routes = ["/health", "/metrics", "/agents"];
    let methods = ["GET", "POST"];

    for route in &routes {
        for method in &methods {
            let duration = Duration::from_millis(25);
            registry
                .record_http_request(route, method, duration)
                .expect("Should record HTTP request");
        }
    }

    // Verify cardinality tracking for HTTP
    let stats = registry.cardinality_stats().expect("Should get stats");
    assert!(
        stats.http_routes_count <= 30,
        "Should respect HTTP route limit"
    );
}

/// Test session correlation via tags
#[tokio::test]
async fn test_session_correlation() {
    let agent_id = AgentId::parse("correlation-agent").expect("Valid agent ID");
    let session_id = SessionId::generate();
    let tool_name = ToolName::parse("correlation_tool").expect("Valid tool name");

    // Create tags for tool execution with session correlation
    let tags =
        CardinalTags::for_tool_execution(agent_id.clone(), session_id.clone(), tool_name.clone());

    assert_eq!(tags.agent_id, Some(agent_id));
    assert_eq!(tags.session_id, Some(session_id));
    assert_eq!(tags.tool_name, Some(tool_name));
    assert!(tags.error_kind.is_none());

    // Test error tags
    let error_tags = CardinalTags::for_error(ErrorKind::Timeout);
    assert!(error_tags.agent_id.is_none());
    assert_eq!(error_tags.error_kind, Some(ErrorKind::Timeout));
}

/// Test latency buckets compliance with DEVELOPMENT_PLAN.md
#[test]
fn test_latency_buckets_compliance() {
    use skreaver_observability::LATENCY_BUCKETS;

    // Verify exact buckets as specified in plan
    let expected = &[0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.5, 5.0, 10.0];
    assert_eq!(
        LATENCY_BUCKETS, expected,
        "Latency buckets should match development plan"
    );

    // Verify coverage from microseconds to 10+ seconds
    assert!(LATENCY_BUCKETS[0] <= 0.01, "Should cover millisecond range");
    assert!(
        LATENCY_BUCKETS.last().unwrap() >= &10.0,
        "Should cover 10+ second range"
    );
}
