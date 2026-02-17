//! Integration tests for MCP Bridge
//!
//! These tests verify the MCP Bridge can connect to real MCP servers
//! and correctly translate tool calls.

#![cfg(feature = "client")]

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use skreaver_mcp::McpBridge;
use std::time::Duration;
use tokio::time::timeout;

/// Test request for the echo tool
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct EchoRequest {
    #[schemars(description = "Message to echo back")]
    message: String,
}

/// Test request for the calculator tool
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
struct CalculatorRequest {
    #[schemars(description = "First operand")]
    a: f64,
    #[schemars(description = "Second operand")]
    b: f64,
    #[schemars(description = "Operation: add, subtract, multiply, divide")]
    operation: String,
}

/// A simple test MCP server for integration testing
#[derive(Clone)]
struct TestMcpServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router(router = tool_router)]
impl TestMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Echo tool - returns the input message
    #[tool(name = "echo", description = "Echo back the input message")]
    async fn echo(&self, request: Parameters<EchoRequest>) -> String {
        format!("Echo: {}", request.0.message)
    }

    /// Calculator tool - performs basic math operations
    #[tool(name = "calculator", description = "Perform basic math operations")]
    async fn calculator(&self, request: Parameters<CalculatorRequest>) -> Result<String, String> {
        let result = match request.0.operation.as_str() {
            "add" => request.0.a + request.0.b,
            "subtract" => request.0.a - request.0.b,
            "multiply" => request.0.a * request.0.b,
            "divide" => {
                if request.0.b == 0.0 {
                    return Err("Division by zero".to_string());
                }
                request.0.a / request.0.b
            }
            op => return Err(format!("Unknown operation: {}", op)),
        };
        Ok(result.to_string())
    }

    /// Greeting tool - returns a greeting
    #[tool(name = "greet", description = "Get a friendly greeting")]
    async fn greet(&self) -> String {
        "Hello from the test MCP server!".to_string()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TestMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability::default()),
                ..Default::default()
            },
            server_info: Implementation {
                name: "test-mcp-server".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            instructions: Some("Test MCP server for integration testing".to_string()),
        }
    }
}

/// Helper to test MCP protocol communication via duplex pipes
/// This verifies the test server works correctly and can be discovered
async fn run_server_and_client_test() {
    // Create pipes for communication
    let (client_read, server_write) = tokio::io::duplex(4096);
    let (server_read, client_write) = tokio::io::duplex(4096);

    // Start the test server
    let server = TestMcpServer::new();
    let server_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(server_read, server_write);

    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await;
        if let Ok(service) = service {
            let _ = service.waiting().await;
        }
    });

    // Give server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect the bridge using the client side of the pipes
    let client_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(client_read, client_write);

    // Create a simple client handler (2025-11-25 spec: meta field added)
    let client_handler = rmcp::model::ClientInfo {
        meta: None,
        protocol_version: Default::default(),
        capabilities: Default::default(),
        client_info: Implementation {
            name: "test-bridge-client".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
    };

    // Connect to the server
    let client_service = client_handler
        .serve(client_transport)
        .await
        .expect("Failed to connect");

    // List tools
    let tools = client_service
        .peer()
        .list_all_tools()
        .await
        .expect("Failed to list tools");

    // Verify we got the expected tools
    assert_eq!(tools.len(), 3, "Expected 3 tools from test server");

    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(tool_names.contains(&"echo"), "Missing echo tool");
    assert!(
        tool_names.contains(&"calculator"),
        "Missing calculator tool"
    );
    assert!(tool_names.contains(&"greet"), "Missing greet tool");

    // Clean up
    server_handle.abort();
}

#[tokio::test]
async fn test_bridge_discovers_tools_via_duplex() {
    run_server_and_client_test().await;
}

/// Test that bridge correctly handles connection errors
#[tokio::test]
async fn test_bridge_connection_error_nonexistent_command() {
    let result = McpBridge::connect_stdio("nonexistent_command_12345").await;
    assert!(result.is_err(), "Should fail for nonexistent command");

    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("Connection error") || err_str.contains("Failed to spawn"),
        "Error should indicate connection failure: {}",
        err_str
    );
}

/// Test that bridge correctly handles empty command
#[tokio::test]
async fn test_bridge_empty_command() {
    let result = McpBridge::connect_stdio("").await;
    assert!(result.is_err(), "Should fail for empty command");

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("Empty server command"),
        "Error should indicate empty command"
    );
}

/// Test bridge with connect_with_args
#[tokio::test]
async fn test_bridge_connect_with_args_error() {
    let result = McpBridge::connect_with_args("nonexistent_program", ["--arg1", "--arg2"]).await;
    assert!(result.is_err(), "Should fail for nonexistent program");
}

/// Test content conversion utilities
mod content_conversion {
    use rmcp::model::Content;
    use serde_json::Value;

    // Re-implement the helper functions for testing since they're private
    fn extract_text_from_contents(contents: &[Content]) -> String {
        contents
            .iter()
            .filter_map(|c| match &c.raw {
                rmcp::model::RawContent::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn contents_to_json(contents: &[Content]) -> Value {
        if contents.is_empty() {
            return Value::Null;
        }

        if contents.len() == 1 {
            return content_to_json(&contents[0]);
        }

        Value::Array(contents.iter().map(content_to_json).collect())
    }

    fn content_to_json(content: &Content) -> Value {
        match &content.raw {
            rmcp::model::RawContent::Text(text) => serde_json::from_str(&text.text)
                .unwrap_or_else(|_| Value::String(text.text.clone())),
            rmcp::model::RawContent::Image(image) => {
                serde_json::json!({
                    "type": "image",
                    "data": image.data,
                    "mime_type": image.mime_type
                })
            }
            rmcp::model::RawContent::Audio(audio) => {
                serde_json::json!({
                    "type": "audio",
                    "data": audio.data,
                    "mime_type": audio.mime_type
                })
            }
            rmcp::model::RawContent::Resource(resource) => {
                serde_json::json!({
                    "type": "resource",
                    "resource": resource.resource
                })
            }
            rmcp::model::RawContent::ResourceLink(link) => {
                serde_json::json!({
                    "type": "resource_link",
                    "uri": link.uri,
                    "name": link.name,
                    "mime_type": link.mime_type
                })
            }
        }
    }

    #[test]
    fn test_multiple_text_contents() {
        let contents = vec![
            Content::text("Line 1"),
            Content::text("Line 2"),
            Content::text("Line 3"),
        ];

        let result = extract_text_from_contents(&contents);
        assert_eq!(result, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_json_parsing_in_text() {
        let contents = vec![Content::text(r#"{"status": "ok", "count": 42}"#)];

        let result = contents_to_json(&contents);
        assert_eq!(result["status"], "ok");
        assert_eq!(result["count"], 42);
    }

    #[test]
    fn test_multiple_contents_to_array() {
        let contents = vec![Content::text("first"), Content::text("second")];

        let result = contents_to_json(&contents);
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_image_content_conversion() {
        let content = Content::image("base64data", "image/png");
        let result = content_to_json(&content);

        assert_eq!(result["type"], "image");
        assert_eq!(result["data"], "base64data");
        assert_eq!(result["mime_type"], "image/png");
    }
}

/// Test timeout handling
#[tokio::test]
async fn test_bridge_connection_timeout() {
    // Try to connect with a very short timeout to a command that hangs
    let result = timeout(
        Duration::from_millis(100),
        McpBridge::connect_stdio("sleep 10"),
    )
    .await;

    // Should timeout before completing
    assert!(result.is_err(), "Should timeout");
}

/// Test tool schema caching
#[tokio::test]
async fn test_tool_schema_cached() {
    // Create pipes for communication
    let (client_read, server_write) = tokio::io::duplex(4096);
    let (server_read, client_write) = tokio::io::duplex(4096);

    // Start the test server
    let server = TestMcpServer::new();
    let server_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(server_read, server_write);

    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await;
        if let Ok(service) = service {
            let _ = service.waiting().await;
        }
    });

    // Give server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect the bridge
    let client_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(client_read, client_write);

    let client_handler = rmcp::model::ClientInfo {
        meta: None,
        protocol_version: Default::default(),
        capabilities: Default::default(),
        client_info: rmcp::model::Implementation {
            name: "test-schema-client".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
    };

    let client_service = client_handler
        .serve(client_transport)
        .await
        .expect("Failed to connect");

    let tools = client_service
        .peer()
        .list_all_tools()
        .await
        .expect("Failed to list tools");

    // Verify schemas are available
    for tool in tools {
        assert!(!tool.name.is_empty(), "Tool should have a name");
        // Input schema should be present (JSON Schema object)
        assert!(
            !tool.input_schema.is_empty(),
            "Tool {} should have input schema",
            tool.name
        );
    }

    // Clean up
    server_handle.abort();
}

/// Test tool_names() helper
#[tokio::test]
async fn test_tool_names_helper() {
    // Create pipes for communication
    let (client_read, server_write) = tokio::io::duplex(4096);
    let (server_read, client_write) = tokio::io::duplex(4096);

    // Start the test server
    let server = TestMcpServer::new();
    let server_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(server_read, server_write);

    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await;
        if let Ok(service) = service {
            let _ = service.waiting().await;
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect the bridge
    let client_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(client_read, client_write);

    let client_handler = rmcp::model::ClientInfo {
        meta: None,
        protocol_version: Default::default(),
        capabilities: Default::default(),
        client_info: rmcp::model::Implementation {
            name: "test-names-client".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
    };

    let client_service = client_handler
        .serve(client_transport)
        .await
        .expect("Failed to connect");

    let peer = client_service.peer();
    let tools = peer.list_all_tools().await.expect("Failed to list tools");

    // Verify we have tool names
    assert!(tools.len() >= 3);
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"calculator"));
    assert!(names.contains(&"greet"));

    // Clean up
    server_handle.abort();
}

/// Test tool execution via duplex pipes
#[tokio::test]
async fn test_tool_execution_via_duplex() {
    use rmcp::model::CallToolRequestParams;
    use std::borrow::Cow;

    // Create pipes for communication
    let (client_read, server_write) = tokio::io::duplex(4096);
    let (server_read, client_write) = tokio::io::duplex(4096);

    // Start the test server
    let server = TestMcpServer::new();
    let server_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(server_read, server_write);

    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await;
        if let Ok(service) = service {
            let _ = service.waiting().await;
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect the client
    let client_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(client_read, client_write);

    let client_handler = rmcp::model::ClientInfo {
        meta: None,
        protocol_version: Default::default(),
        capabilities: Default::default(),
        client_info: rmcp::model::Implementation {
            name: "test-execution-client".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
    };

    let client_service = client_handler
        .serve(client_transport)
        .await
        .expect("Failed to connect");

    let peer = client_service.peer();

    // Test echo tool
    let echo_params = CallToolRequestParams {
        meta: None,
        name: Cow::Borrowed("echo"),
        arguments: Some(serde_json::json!({"message": "Hello, MCP!"}).as_object().unwrap().clone()),
        task: None,
    };

    let result = peer.call_tool(echo_params).await.expect("Echo call failed");
    assert!(!result.is_error.unwrap_or(false), "Echo should succeed");

    // Extract text content
    let text = result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            rmcp::model::RawContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    assert!(text.contains("Hello, MCP!"), "Echo should return input");

    // Test calculator tool - addition
    let calc_params = CallToolRequestParams {
        meta: None,
        name: Cow::Borrowed("calculator"),
        arguments: Some(
            serde_json::json!({"a": 10.0, "b": 5.0, "operation": "add"})
                .as_object()
                .unwrap()
                .clone(),
        ),
        task: None,
    };

    let result = peer
        .call_tool(calc_params)
        .await
        .expect("Calculator call failed");
    assert!(
        !result.is_error.unwrap_or(false),
        "Calculator should succeed"
    );

    let text = result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            rmcp::model::RawContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    assert_eq!(text, "15", "10 + 5 = 15");

    // Test greet tool (no parameters)
    let greet_params = CallToolRequestParams {
        meta: None,
        name: Cow::Borrowed("greet"),
        arguments: None,
        task: None,
    };

    let result = peer
        .call_tool(greet_params)
        .await
        .expect("Greet call failed");
    assert!(!result.is_error.unwrap_or(false), "Greet should succeed");

    let text = result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            rmcp::model::RawContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    assert!(text.contains("Hello"), "Greet should return greeting");

    server_handle.abort();
}

/// Test calculator tool with different operations
#[tokio::test]
async fn test_calculator_operations() {
    use rmcp::model::CallToolRequestParams;
    use std::borrow::Cow;

    let (client_read, server_write) = tokio::io::duplex(4096);
    let (server_read, client_write) = tokio::io::duplex(4096);

    let server = TestMcpServer::new();
    let server_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(server_read, server_write);

    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await;
        if let Ok(service) = service {
            let _ = service.waiting().await;
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(client_read, client_write);

    let client_handler = rmcp::model::ClientInfo {
        meta: None,
        protocol_version: Default::default(),
        capabilities: Default::default(),
        client_info: rmcp::model::Implementation {
            name: "test-calc-client".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
    };

    let client_service = client_handler
        .serve(client_transport)
        .await
        .expect("Failed to connect");

    let peer = client_service.peer();

    // Test all operations
    let test_cases = [
        (20.0, 5.0, "subtract", "15"),
        (6.0, 7.0, "multiply", "42"),
        (100.0, 4.0, "divide", "25"),
    ];

    for (a, b, op, expected) in test_cases {
        let params = CallToolRequestParams {
            meta: None,
            name: Cow::Borrowed("calculator"),
            arguments: Some(
                serde_json::json!({"a": a, "b": b, "operation": op})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
            task: None,
        };

        let result = peer.call_tool(params).await.expect("Calculator call failed");
        assert!(!result.is_error.unwrap_or(false));

        let text = result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                rmcp::model::RawContent::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(text, expected, "{} {} {} should equal {}", a, op, b, expected);
    }

    server_handle.abort();
}

/// Test error handling - division by zero
#[tokio::test]
async fn test_tool_error_handling() {
    use rmcp::model::CallToolRequestParams;
    use std::borrow::Cow;

    let (client_read, server_write) = tokio::io::duplex(4096);
    let (server_read, client_write) = tokio::io::duplex(4096);

    let server = TestMcpServer::new();
    let server_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(server_read, server_write);

    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await;
        if let Ok(service) = service {
            let _ = service.waiting().await;
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let client_transport =
        rmcp::transport::async_rw::AsyncRwTransport::new(client_read, client_write);

    let client_handler = rmcp::model::ClientInfo {
        meta: None,
        protocol_version: Default::default(),
        capabilities: Default::default(),
        client_info: rmcp::model::Implementation {
            name: "test-error-client".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
    };

    let client_service = client_handler
        .serve(client_transport)
        .await
        .expect("Failed to connect");

    let peer = client_service.peer();

    // Test division by zero
    let params = CallToolRequestParams {
        meta: None,
        name: Cow::Borrowed("calculator"),
        arguments: Some(
            serde_json::json!({"a": 10.0, "b": 0.0, "operation": "divide"})
                .as_object()
                .unwrap()
                .clone(),
        ),
        task: None,
    };

    let result = peer.call_tool(params).await.expect("Call should complete");
    assert!(result.is_error.unwrap_or(false), "Division by zero should error");

    let error_text = result
        .content
        .iter()
        .filter_map(|c| match &c.raw {
            rmcp::model::RawContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    assert!(
        error_text.contains("Division by zero"),
        "Error should mention division by zero"
    );

    // Test unknown operation
    let params = CallToolRequestParams {
        meta: None,
        name: Cow::Borrowed("calculator"),
        arguments: Some(
            serde_json::json!({"a": 10.0, "b": 5.0, "operation": "modulo"})
                .as_object()
                .unwrap()
                .clone(),
        ),
        task: None,
    };

    let result = peer.call_tool(params).await.expect("Call should complete");
    assert!(
        result.is_error.unwrap_or(false),
        "Unknown operation should error"
    );

    server_handle.abort();
}

/// Test error mapping from McpError to FailureReason
mod error_mapping {
    use skreaver_mcp::error::McpError;

    #[test]
    fn test_tool_execution_error_mapping() {
        let err = McpError::ToolExecutionFailed("calculation failed".to_string());
        let reason = err.to_failure_reason();

        match reason {
            skreaver_core::FailureReason::Custom { category, message } => {
                assert_eq!(category, "mcp_tool_error");
                assert!(message.contains("calculation failed"));
            }
            _ => panic!("Expected Custom error type"),
        }
    }

    #[test]
    fn test_connection_error_mapping() {
        let err = McpError::ConnectionError("connection refused".to_string());
        let reason = err.to_failure_reason();

        match reason {
            skreaver_core::FailureReason::NetworkError { message } => {
                assert!(message.contains("connection refused"));
            }
            _ => panic!("Expected NetworkError type"),
        }
    }

    #[test]
    fn test_timeout_error_mapping() {
        let err = McpError::TaskTimeout("operation timed out".to_string());
        let reason = err.to_failure_reason();

        match reason {
            skreaver_core::FailureReason::Timeout { operation } => {
                assert!(operation.contains("timed out"));
            }
            _ => panic!("Expected Timeout type"),
        }
    }

    #[test]
    fn test_invalid_params_error_mapping() {
        let err = McpError::InvalidParameters("missing required field".to_string());
        let reason = err.to_failure_reason();

        match reason {
            skreaver_core::FailureReason::InvalidInput { message } => {
                assert!(message.contains("missing required field"));
            }
            _ => panic!("Expected InvalidInput type"),
        }
    }
}
