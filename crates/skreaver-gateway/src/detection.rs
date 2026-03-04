//! Protocol Detection
//!
//! This module provides protocol detection capabilities for determining
//! whether incoming messages use MCP or A2A protocol format.

use serde_json::Value;

use crate::error::{GatewayError, GatewayResult};

/// Detected protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// Model Context Protocol
    Mcp,
    /// Agent-to-Agent Protocol
    A2a,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Mcp => write!(f, "MCP"),
            Protocol::A2a => write!(f, "A2A"),
        }
    }
}

/// Protocol detector for identifying message formats
#[derive(Debug, Clone, Default)]
pub struct ProtocolDetector {
    /// Enable strict mode (require explicit protocol markers)
    strict_mode: bool,
}

impl ProtocolDetector {
    /// Create a new protocol detector
    pub fn new() -> Self {
        Self { strict_mode: false }
    }

    /// Create a detector with strict mode enabled
    pub fn strict() -> Self {
        Self { strict_mode: true }
    }

    /// Detect protocol from a JSON message
    ///
    /// # Detection Rules
    ///
    /// **MCP Detection:**
    /// - Has "jsonrpc" field with value "2.0"
    /// - Has "method" field (for requests/notifications)
    /// - Has "result" or "error" field (for responses)
    ///
    /// **A2A Detection:**
    /// - Has "taskId" or "task_id" field
    /// - Has "agentCard" or "agent_card" field
    /// - Has "status" field with A2A task status values
    /// - Has "messages" array with "role" and "parts" structure
    pub fn detect(&self, message: &Value) -> GatewayResult<Protocol> {
        // Check for MCP JSON-RPC format
        if self.is_mcp_message(message) {
            return Ok(Protocol::Mcp);
        }

        // Check for A2A format
        if self.is_a2a_message(message) {
            return Ok(Protocol::A2a);
        }

        if self.strict_mode {
            Err(GatewayError::ProtocolDetectionFailed(
                "Could not determine protocol in strict mode".to_string(),
            ))
        } else {
            // Default to MCP in non-strict mode (more common for tool calls)
            Ok(Protocol::Mcp)
        }
    }

    /// Detect protocol from raw JSON string
    pub fn detect_str(&self, json_str: &str) -> GatewayResult<Protocol> {
        let value: Value = serde_json::from_str(json_str)?;
        self.detect(&value)
    }

    /// Check if message is MCP format (JSON-RPC 2.0)
    fn is_mcp_message(&self, message: &Value) -> bool {
        let obj = match message.as_object() {
            Some(o) => o,
            None => return false,
        };

        // MCP uses JSON-RPC 2.0
        if let Some(jsonrpc) = obj.get("jsonrpc")
            && jsonrpc == "2.0"
        {
            // Request or notification (has method)
            if obj.contains_key("method") {
                return true;
            }
            // Response (has result or error, and id)
            if obj.contains_key("id") && (obj.contains_key("result") || obj.contains_key("error")) {
                return true;
            }
        }

        false
    }

    /// Check if message is A2A format
    fn is_a2a_message(&self, message: &Value) -> bool {
        let obj = match message.as_object() {
            Some(o) => o,
            None => return false,
        };

        // A2A task messages have taskId
        if obj.contains_key("taskId") || obj.contains_key("task_id") {
            return true;
        }

        // A2A agent card
        if obj.contains_key("agentCard") || obj.contains_key("agent_card") {
            return true;
        }

        // Check for A2A task structure
        if obj.contains_key("id")
            && obj.contains_key("status")
            && let Some(status) = obj.get("status").and_then(|s| s.as_str())
        {
            let a2a_statuses = [
                "working",
                "completed",
                "failed",
                "cancelled",
                "rejected",
                "input-required",
            ];
            if a2a_statuses.contains(&status) {
                return true;
            }
        }

        // Check for A2A message structure
        if obj.contains_key("messages")
            && let Some(messages) = obj.get("messages").and_then(|m| m.as_array())
            && messages.iter().any(|m| {
                m.get("role").is_some() && (m.get("parts").is_some() || m.get("content").is_some())
            })
        {
            return true;
        }

        // Check for A2A SendMessageRequest
        if obj.contains_key("message")
            && let Some(msg) = obj.get("message").and_then(|m| m.as_object())
            && msg.contains_key("role")
            && msg.contains_key("parts")
        {
            return true;
        }

        false
    }

    /// Detect protocol from HTTP Content-Type header hint
    pub fn detect_from_content_type(&self, content_type: &str) -> Option<Protocol> {
        let ct_lower = content_type.to_lowercase();

        // MCP typically uses application/json with JSON-RPC
        if ct_lower.contains("application/json-rpc") {
            return Some(Protocol::Mcp);
        }

        // A2A may use specific content types
        if ct_lower.contains("application/a2a") {
            return Some(Protocol::A2a);
        }

        // No hint from content type
        None
    }

    /// Detect protocol from URL path hint
    pub fn detect_from_path(&self, path: &str) -> Option<Protocol> {
        let path_lower = path.to_lowercase();

        // A2A endpoints typically have /a2a/ prefix
        if path_lower.contains("/a2a/") {
            return Some(Protocol::A2a);
        }

        // MCP endpoints typically have /mcp/ prefix
        if path_lower.contains("/mcp/") {
            return Some(Protocol::Mcp);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_detect_mcp_request() {
        let detector = ProtocolDetector::new();

        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "calculator",
                "arguments": {"a": 1, "b": 2}
            }
        });

        assert_eq!(detector.detect(&msg).unwrap(), Protocol::Mcp);
    }

    #[test]
    fn test_detect_mcp_response() {
        let detector = ProtocolDetector::new();

        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"content": [{"type": "text", "text": "3"}]}
        });

        assert_eq!(detector.detect(&msg).unwrap(), Protocol::Mcp);
    }

    #[test]
    fn test_detect_mcp_notification() {
        let detector = ProtocolDetector::new();

        let msg = json!({
            "jsonrpc": "2.0",
            "method": "notifications/message",
            "params": {"message": "Hello"}
        });

        assert_eq!(detector.detect(&msg).unwrap(), Protocol::Mcp);
    }

    #[test]
    fn test_detect_a2a_task() {
        let detector = ProtocolDetector::new();

        let msg = json!({
            "id": "task-123",
            "status": "working",
            "messages": [
                {"role": "user", "parts": [{"type": "text", "text": "Hello"}]}
            ]
        });

        assert_eq!(detector.detect(&msg).unwrap(), Protocol::A2a);
    }

    #[test]
    fn test_detect_a2a_task_id() {
        let detector = ProtocolDetector::new();

        let msg = json!({
            "taskId": "task-123",
            "message": {"role": "user", "parts": []}
        });

        assert_eq!(detector.detect(&msg).unwrap(), Protocol::A2a);
    }

    #[test]
    fn test_detect_a2a_send_message_request() {
        let detector = ProtocolDetector::new();

        let msg = json!({
            "message": {
                "role": "user",
                "parts": [{"type": "text", "text": "Summarize this"}]
            }
        });

        assert_eq!(detector.detect(&msg).unwrap(), Protocol::A2a);
    }

    #[test]
    fn test_detect_strict_mode_fails_on_ambiguous() {
        let detector = ProtocolDetector::strict();

        let msg = json!({
            "data": "some random data"
        });

        assert!(detector.detect(&msg).is_err());
    }

    #[test]
    fn test_detect_non_strict_defaults_to_mcp() {
        let detector = ProtocolDetector::new();

        let msg = json!({
            "data": "some random data"
        });

        assert_eq!(detector.detect(&msg).unwrap(), Protocol::Mcp);
    }

    #[test]
    fn test_detect_from_string() {
        let detector = ProtocolDetector::new();

        let mcp_str = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        assert_eq!(detector.detect_str(mcp_str).unwrap(), Protocol::Mcp);

        let a2a_str = r#"{"taskId":"task-1","status":"working"}"#;
        assert_eq!(detector.detect_str(a2a_str).unwrap(), Protocol::A2a);
    }

    #[test]
    fn test_detect_from_path() {
        let detector = ProtocolDetector::new();

        assert_eq!(
            detector.detect_from_path("/api/a2a/tasks"),
            Some(Protocol::A2a)
        );
        assert_eq!(
            detector.detect_from_path("/api/mcp/tools"),
            Some(Protocol::Mcp)
        );
        assert_eq!(detector.detect_from_path("/api/other"), None);
    }

    #[test]
    fn test_detect_from_content_type() {
        let detector = ProtocolDetector::new();

        assert_eq!(
            detector.detect_from_content_type("application/json-rpc"),
            Some(Protocol::Mcp)
        );
        assert_eq!(
            detector.detect_from_content_type("application/a2a+json"),
            Some(Protocol::A2a)
        );
        assert_eq!(detector.detect_from_content_type("application/json"), None);
    }

    #[test]
    fn test_protocol_display() {
        assert_eq!(Protocol::Mcp.to_string(), "MCP");
        assert_eq!(Protocol::A2a.to_string(), "A2A");
    }
}
