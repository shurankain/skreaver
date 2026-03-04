//! # Skreaver Gateway - Protocol Translation Layer
//!
//! This crate provides a protocol gateway for bidirectional translation between
//! MCP (Model Context Protocol) and A2A (Agent-to-Agent) protocols.
//!
//! ## Features
//!
//! - **Protocol Detection**: Automatically detect whether messages use MCP or A2A format
//! - **Bidirectional Translation**: Convert messages between MCP and A2A protocols
//! - **Connection Registry**: Track and manage active protocol connections
//!
//! ## Protocol Overview
//!
//! ### MCP (Model Context Protocol)
//! - JSON-RPC 2.0 based protocol
//! - Tool calls, resources, and sampling
//! - Request/response with notifications
//!
//! ### A2A (Agent-to-Agent Protocol)
//! - Task-based communication
//! - Agent cards for capability discovery
//! - Streaming events and artifacts
//!
//! ## Example: Protocol Detection
//!
//! ```rust
//! use skreaver_gateway::{ProtocolDetector, Protocol};
//! use serde_json::json;
//!
//! let detector = ProtocolDetector::new();
//!
//! // Detect MCP message
//! let mcp_msg = json!({
//!     "jsonrpc": "2.0",
//!     "id": 1,
//!     "method": "tools/call",
//!     "params": {"name": "calculator"}
//! });
//! assert_eq!(detector.detect(&mcp_msg).unwrap(), Protocol::Mcp);
//!
//! // Detect A2A message
//! let a2a_msg = json!({
//!     "taskId": "task-123",
//!     "status": "working"
//! });
//! assert_eq!(detector.detect(&a2a_msg).unwrap(), Protocol::A2a);
//! ```
//!
//! ## Example: Protocol Translation
//!
//! ```rust
//! use skreaver_gateway::{ProtocolTranslator, Protocol};
//! use serde_json::json;
//!
//! let translator = ProtocolTranslator::new();
//!
//! // Translate MCP tool call to A2A
//! let mcp_request = json!({
//!     "jsonrpc": "2.0",
//!     "id": 1,
//!     "method": "tools/call",
//!     "params": {
//!         "name": "calculator",
//!         "arguments": {"a": 5, "b": 3}
//!     }
//! });
//!
//! let a2a_result = translator.translate(mcp_request, Protocol::Mcp, Protocol::A2a).unwrap();
//! assert!(a2a_result.get("taskId").is_some());
//! ```
//!
//! ## Example: Connection Registry
//!
//! ```rust
//! use skreaver_gateway::{ConnectionRegistry, ConnectionInfo, Protocol};
//!
//! #[tokio::main]
//! async fn main() {
//!     let registry = ConnectionRegistry::new()
//!         .with_max_connections(100)
//!         .with_idle_timeout(300);
//!
//!     // Register a connection
//!     let info = ConnectionInfo::new("conn-1", Protocol::Mcp, "http://localhost:3000");
//!     registry.register(info).await.unwrap();
//!
//!     // List active connections
//!     let active = registry.list_active().await;
//!     println!("Active connections: {}", active.len());
//! }
//! ```

pub mod detection;
pub mod error;
pub mod registry;
pub mod translation;

// Re-export core types
pub use detection::{Protocol, ProtocolDetector};
pub use error::{GatewayError, GatewayResult};
pub use registry::{ConnectionInfo, ConnectionRegistry, ConnectionState, RegistryStats};
pub use translation::{A2aToMcpTranslator, McpToA2aTranslator, ProtocolTranslator};

/// Gateway for protocol translation and routing
///
/// The `ProtocolGateway` combines protocol detection, translation, and
/// connection management into a unified interface.
#[derive(Debug, Clone)]
pub struct ProtocolGateway {
    /// Protocol detector
    pub detector: ProtocolDetector,
    /// Protocol translator
    pub translator: ProtocolTranslator,
    /// Connection registry
    pub registry: ConnectionRegistry,
}

impl ProtocolGateway {
    /// Create a new protocol gateway
    pub fn new() -> Self {
        Self {
            detector: ProtocolDetector::new(),
            translator: ProtocolTranslator::new(),
            registry: ConnectionRegistry::new(),
        }
    }

    /// Create a gateway with custom configuration
    pub fn with_config(
        detector: ProtocolDetector,
        translator: ProtocolTranslator,
        registry: ConnectionRegistry,
    ) -> Self {
        Self {
            detector,
            translator,
            registry,
        }
    }

    /// Detect and translate a message to the target protocol
    pub fn translate_to(
        &self,
        message: serde_json::Value,
        target: Protocol,
    ) -> GatewayResult<serde_json::Value> {
        let source = self.detector.detect(&message)?;
        self.translator.translate(message, source, target)
    }

    /// Detect protocol and translate to the opposite protocol
    pub fn translate_opposite(
        &self,
        message: serde_json::Value,
    ) -> GatewayResult<serde_json::Value> {
        let source = self.detector.detect(&message)?;
        let target = match source {
            Protocol::Mcp => Protocol::A2a,
            Protocol::A2a => Protocol::Mcp,
        };
        self.translator.translate(message, source, target)
    }
}

impl Default for ProtocolGateway {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_gateway_translate_mcp_to_a2a() {
        let gateway = ProtocolGateway::new();

        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "test",
                "arguments": {}
            }
        });

        let result = gateway.translate_to(mcp_request, Protocol::A2a).unwrap();
        assert!(result.get("taskId").is_some() || result.get("task_id").is_some());
    }

    #[test]
    fn test_gateway_translate_a2a_to_mcp() {
        let gateway = ProtocolGateway::new();

        let a2a_task = json!({
            "id": "task-123",
            "status": "completed",
            "messages": [
                {
                    "role": "agent",
                    "parts": [{"type": "text", "text": "Done"}]
                }
            ]
        });

        let result = gateway.translate_to(a2a_task, Protocol::Mcp).unwrap();
        assert_eq!(result["jsonrpc"], "2.0");
    }

    #[test]
    fn test_gateway_translate_opposite() {
        let gateway = ProtocolGateway::new();

        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "ping"
        });

        let result = gateway.translate_opposite(mcp_request).unwrap();
        // MCP -> A2A
        assert!(result.get("jsonrpc").is_none());
    }

    #[test]
    fn test_gateway_same_protocol_passthrough() {
        let gateway = ProtocolGateway::new();

        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "ping"
        });

        let result = gateway
            .translate_to(mcp_request.clone(), Protocol::Mcp)
            .unwrap();
        assert_eq!(result, mcp_request);
    }

    #[tokio::test]
    async fn test_gateway_with_registry() {
        let gateway = ProtocolGateway::new();

        // Register a connection
        let info = ConnectionInfo::new("conn-1", Protocol::Mcp, "http://localhost:3000");
        gateway.registry.register(info).await.unwrap();

        // Check stats
        let stats = gateway.registry.stats().await;
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.active_connections, 1);
    }
}
