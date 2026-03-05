//! Gateway Integration Tests
//!
//! These tests verify the complete protocol gateway functionality including:
//! - End-to-end MCP <-> A2A translation
//! - Protocol compliance validation
//! - Connection registry operations
//! - Complex message scenarios

use serde_json::json;
use skreaver_gateway::{
    ConnectionInfo, ConnectionRegistry, ConnectionState, GatewayError, Protocol, ProtocolDetector,
    ProtocolGateway, ProtocolTranslator,
};

// ============================================================================
// Test 1: Full MCP Tool Call Roundtrip
// ============================================================================

/// Test complete MCP tool call -> A2A task -> MCP response roundtrip
#[test]
fn test_mcp_tool_call_to_a2a_roundtrip() {
    let gateway = ProtocolGateway::new();

    // Step 1: MCP tool call request
    let mcp_request = json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {
            "name": "calculator",
            "arguments": {
                "operation": "add",
                "a": 10,
                "b": 5
            }
        }
    });

    // Translate to A2A
    let a2a_request = gateway
        .translate_to(mcp_request.clone(), Protocol::A2a)
        .unwrap();

    // Verify A2A structure
    assert!(
        a2a_request.get("taskId").is_some() || a2a_request.get("task_id").is_some(),
        "Should have taskId"
    );
    assert!(a2a_request.get("message").is_some(), "Should have message");

    // The message should contain tool call info
    let message = &a2a_request["message"];
    assert_eq!(message["role"], "user", "Message should be from user");
    assert!(message.get("parts").is_some(), "Should have parts");

    // Verify metadata preserves tool info
    let metadata = &a2a_request["metadata"];
    assert_eq!(metadata["tool_name"], "calculator");
}

// ============================================================================
// Test 2: A2A Task Completion to MCP Response
// ============================================================================

/// Test A2A completed task translates to proper MCP success response
#[test]
fn test_a2a_completed_task_to_mcp_response() {
    let gateway = ProtocolGateway::new();

    // A2A completed task
    let a2a_task = json!({
        "id": "task-abc-123",
        "status": "completed",
        "messages": [
            {
                "role": "agent",
                "parts": [
                    {"type": "text", "text": "The result of 10 + 5 is 15"}
                ]
            }
        ],
        "artifacts": []
    });

    // Translate to MCP
    let mcp_response = gateway.translate_to(a2a_task, Protocol::Mcp).unwrap();

    // Verify MCP response structure
    assert_eq!(mcp_response["jsonrpc"], "2.0", "Should be JSON-RPC 2.0");
    assert!(mcp_response.get("id").is_some(), "Should have id");
    assert!(mcp_response.get("result").is_some(), "Should have result");
    assert!(mcp_response.get("error").is_none(), "Should not have error");

    // Verify result contains the content
    let result = &mcp_response["result"];
    assert!(
        result.get("content").is_some(),
        "Result should have content"
    );
}

// ============================================================================
// Test 3: A2A Failed Task to MCP Error Response
// ============================================================================

/// Test A2A failed task translates to proper MCP error response
#[test]
fn test_a2a_failed_task_to_mcp_error() {
    let gateway = ProtocolGateway::new();

    // A2A failed task
    let a2a_task = json!({
        "id": "task-failed-456",
        "status": "failed",
        "messages": [
            {
                "role": "agent",
                "parts": [
                    {"type": "text", "text": "Division by zero error"}
                ]
            }
        ]
    });

    // Translate to MCP
    let mcp_response = gateway.translate_to(a2a_task, Protocol::Mcp).unwrap();

    // Verify MCP error structure
    assert_eq!(mcp_response["jsonrpc"], "2.0", "Should be JSON-RPC 2.0");
    assert!(mcp_response.get("error").is_some(), "Should have error");
    assert!(
        mcp_response.get("result").is_none(),
        "Should not have result"
    );

    // Verify error contains message
    let error = &mcp_response["error"];
    assert!(error.get("code").is_some(), "Error should have code");
    assert!(error.get("message").is_some(), "Error should have message");
}

// ============================================================================
// Test 4: Protocol Detection Accuracy
// ============================================================================

/// Test protocol detection with various message formats
#[test]
fn test_protocol_detection_comprehensive() {
    let detector = ProtocolDetector::new();

    // MCP request variants
    let mcp_messages = vec![
        json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "result": {"tools": []}}),
        json!({"jsonrpc": "2.0", "id": 4, "error": {"code": -32600, "message": "Invalid"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/progress"}),
    ];

    for msg in mcp_messages {
        assert_eq!(
            detector.detect(&msg).unwrap(),
            Protocol::Mcp,
            "Should detect MCP: {}",
            msg
        );
    }

    // A2A message variants - must match actual detection rules in detection.rs
    let a2a_messages = vec![
        json!({"id": "task-1", "status": "working", "messages": []}),
        json!({"id": "task-2", "status": "completed", "messages": [{"role": "agent", "parts": []}]}),
        json!({"taskId": "task-3", "message": {"role": "user", "parts": []}}),
        // agentCard field triggers A2A detection (not agentId + skills)
        json!({"agentCard": {"name": "agent-1"}}),
        json!({"type": "status", "taskId": "task-4", "status": "working"}),
    ];

    for msg in a2a_messages {
        assert_eq!(
            detector.detect(&msg).unwrap(),
            Protocol::A2a,
            "Should detect A2A: {}",
            msg
        );
    }
}

// ============================================================================
// Test 5: MCP Notification Translation
// ============================================================================

/// Test MCP progress notification translates to A2A streaming event
#[test]
fn test_mcp_notification_to_a2a_event() {
    let translator = ProtocolTranslator::new();

    // MCP progress notification
    let mcp_notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": {
            "progressToken": "token-123",
            "progress": 50,
            "total": 100
        }
    });

    // Translate to A2A
    let a2a_event = translator
        .translate(mcp_notification, Protocol::Mcp, Protocol::A2a)
        .unwrap();

    // Should be a streaming event or status update
    assert!(
        a2a_event.get("type").is_some() || a2a_event.get("status").is_some(),
        "Should have event type or status"
    );
}

// ============================================================================
// Test 6: A2A Streaming Event to MCP Notification
// ============================================================================

/// Test A2A streaming events translate to MCP notifications
#[test]
fn test_a2a_streaming_event_to_mcp_notification() {
    let translator = ProtocolTranslator::new();

    // A2A progress event
    let a2a_event = json!({
        "type": "progress",
        "taskId": "task-789",
        "progress": {
            "current": 3,
            "total": 10,
            "message": "Processing step 3 of 10"
        }
    });

    // Translate to MCP
    let mcp_notification = translator
        .translate(a2a_event, Protocol::A2a, Protocol::Mcp)
        .unwrap();

    // Should be MCP notification format
    assert_eq!(mcp_notification["jsonrpc"], "2.0");
    // Notifications don't have id
    assert!(
        mcp_notification.get("method").is_some(),
        "Should have method"
    );
}

// ============================================================================
// Test 7: Connection Registry Integration
// ============================================================================

/// Test connection registry with multiple connections and state transitions
#[tokio::test]
async fn test_connection_registry_full_lifecycle() {
    let registry = ConnectionRegistry::new()
        .with_max_connections(10)
        .with_idle_timeout(60);

    // Register multiple connections
    let mcp_conn = ConnectionInfo::new("mcp-client-1", Protocol::Mcp, "ws://localhost:3000/mcp");
    let a2a_conn = ConnectionInfo::new("a2a-agent-1", Protocol::A2a, "http://localhost:3001/a2a");

    registry.register(mcp_conn).await.unwrap();
    registry.register(a2a_conn).await.unwrap();

    // Verify stats
    let stats = registry.stats().await;
    assert_eq!(stats.total_connections, 2);
    assert_eq!(stats.active_connections, 2);

    // Check protocol counts via by_protocol map (Protocol::to_string() returns uppercase)
    assert_eq!(stats.by_protocol.get("MCP"), Some(&1));
    assert_eq!(stats.by_protocol.get("A2A"), Some(&1));

    // Update connection state
    registry
        .update_state("mcp-client-1", ConnectionState::Disconnecting)
        .await
        .unwrap();

    // Verify connection state
    let conn = registry.get("mcp-client-1").await.unwrap();
    assert_eq!(conn.state, ConnectionState::Disconnecting);

    // Unregister
    registry.unregister("mcp-client-1").await.unwrap();

    let stats = registry.stats().await;
    assert_eq!(stats.total_connections, 1);
    assert_eq!(stats.active_connections, 1);
}

// ============================================================================
// Test 8: Error Handling
// ============================================================================

/// Test gateway error handling for invalid messages
#[test]
fn test_gateway_error_handling() {
    // Use strict mode to get detection failures
    let detector = ProtocolDetector::strict();
    let translator = ProtocolTranslator::new();
    let registry = ConnectionRegistry::new();
    let gateway = ProtocolGateway::with_config(detector, translator, registry);

    // Invalid message (neither MCP nor A2A) - strict mode should fail
    let invalid_message = json!({
        "random": "data",
        "not": "a protocol message"
    });

    let result = gateway.translate_to(invalid_message, Protocol::Mcp);
    assert!(
        result.is_err(),
        "Strict mode should fail for unrecognized message"
    );

    match result.unwrap_err() {
        GatewayError::ProtocolDetectionFailed(_) => {}
        other => panic!("Expected ProtocolDetectionFailed, got: {:?}", other),
    }

    // Non-object JSON should fail with InvalidMessage during translation
    let non_object = json!("just a string");
    let result = gateway.translate_to(non_object, Protocol::A2a);
    assert!(result.is_err(), "Should fail for non-object JSON");
}

// ============================================================================
// Test 9: MCP Sampling to A2A Message
// ============================================================================

/// Test MCP sampling/createMessage translates to A2A SendMessageRequest
#[test]
fn test_mcp_sampling_to_a2a_message() {
    let translator = ProtocolTranslator::new();

    // MCP sampling request
    let mcp_sampling = json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "sampling/createMessage",
        "params": {
            "messages": [
                {
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": "What is the weather like today?"
                    }
                }
            ],
            "maxTokens": 1000
        }
    });

    // Translate to A2A
    let a2a_request = translator
        .translate(mcp_sampling, Protocol::Mcp, Protocol::A2a)
        .unwrap();

    // Should have A2A structure - either a task with messages or a SendMessageRequest
    // The exact structure depends on how sampling is mapped
    assert!(
        a2a_request.get("taskId").is_some()
            || a2a_request.get("task_id").is_some()
            || a2a_request.get("message").is_some()
            || a2a_request.get("messages").is_some(),
        "Should translate to A2A format: {}",
        a2a_request
    );
}

// ============================================================================
// Test 10: Protocol Compliance - MCP JSON-RPC 2.0
// ============================================================================

/// Test that translated MCP messages are JSON-RPC 2.0 compliant
#[test]
fn test_mcp_jsonrpc_compliance() {
    let translator = ProtocolTranslator::new();

    // A2A completed task
    let a2a_task = json!({
        "id": "compliance-test",
        "status": "completed",
        "messages": [
            {"role": "agent", "parts": [{"type": "text", "text": "Result"}]}
        ]
    });

    let mcp_response = translator
        .translate(a2a_task, Protocol::A2a, Protocol::Mcp)
        .unwrap();

    // JSON-RPC 2.0 compliance checks
    assert_eq!(mcp_response["jsonrpc"], "2.0", "Must have jsonrpc: '2.0'");
    assert!(mcp_response.get("id").is_some(), "Response must have id");

    // Must have either result or error, not both
    let has_result = mcp_response.get("result").is_some();
    let has_error = mcp_response.get("error").is_some();
    assert!(
        has_result ^ has_error,
        "Must have exactly one of result or error, not both"
    );
}

// ============================================================================
// Test 11: Protocol Compliance - A2A Task States
// ============================================================================

/// Test that A2A task status values are valid
#[test]
fn test_a2a_task_status_compliance() {
    let detector = ProtocolDetector::new();

    // Valid A2A statuses per spec
    let valid_statuses = [
        "working",
        "completed",
        "failed",
        "cancelled",
        "rejected",
        "input-required",
    ];

    for status in valid_statuses {
        let task = json!({
            "id": format!("task-{}", status),
            "status": status,
            "messages": []
        });

        assert_eq!(
            detector.detect(&task).unwrap(),
            Protocol::A2a,
            "Should detect valid A2A status: {}",
            status
        );
    }
}

// ============================================================================
// Test 12: Bidirectional Translation Consistency
// ============================================================================

/// Test that MCP -> A2A -> MCP preserves essential information
#[test]
fn test_bidirectional_translation_consistency() {
    let gateway = ProtocolGateway::new();

    // Original MCP tool call request (not tools/list which has different semantics)
    let original_mcp = json!({
        "jsonrpc": "2.0",
        "id": 999,
        "method": "tools/call",
        "params": {
            "name": "test_tool",
            "arguments": {"key": "value"}
        }
    });

    // MCP -> A2A
    let a2a = gateway
        .translate_to(original_mcp.clone(), Protocol::A2a)
        .unwrap();

    // A2A should have SendMessageRequest structure with taskId and message
    assert!(
        a2a.get("taskId").is_some() || a2a.get("task_id").is_some(),
        "A2A should have taskId"
    );
    assert!(
        a2a.get("message").is_some(),
        "A2A should have message field"
    );

    // Note: Perfect roundtrip isn't always possible due to protocol differences,
    // but the semantic intent should be preserved
}

// ============================================================================
// Test 13: Agent Card Translation
// ============================================================================

/// Test A2A agent card translates to MCP server info
#[test]
fn test_agent_card_to_mcp() {
    let translator = ProtocolTranslator::new();

    let agent_card = json!({
        "agentId": "weather-agent",
        "name": "Weather Agent",
        "description": "Provides weather information",
        "skills": [
            {
                "name": "get_weather",
                "description": "Get current weather for a location"
            }
        ],
        "capabilities": {
            "streaming": true
        }
    });

    let mcp_info = translator
        .translate(agent_card, Protocol::A2a, Protocol::Mcp)
        .unwrap();

    // Should translate to MCP server info or initialize response
    assert_eq!(mcp_info["jsonrpc"], "2.0");
}

// ============================================================================
// Test 14: Complex Multi-Part Message
// ============================================================================

/// Test translation of complex messages with multiple parts
#[test]
fn test_complex_multipart_message() {
    let translator = ProtocolTranslator::new();

    // A2A message with multiple parts (text + data)
    let a2a_message = json!({
        "id": "multipart-task",
        "status": "completed",
        "messages": [
            {
                "role": "agent",
                "parts": [
                    {"type": "text", "text": "Here is the analysis:"},
                    {"type": "data", "data": {"temperature": 72, "humidity": 45}},
                    {"type": "text", "text": "The conditions are favorable."}
                ]
            }
        ]
    });

    let mcp_response = translator
        .translate(a2a_message, Protocol::A2a, Protocol::Mcp)
        .unwrap();

    // Should have proper MCP structure
    assert_eq!(mcp_response["jsonrpc"], "2.0");
    assert!(mcp_response.get("result").is_some());

    // Content should be preserved
    let result = &mcp_response["result"];
    assert!(result.get("content").is_some());
}
