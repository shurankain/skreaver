//! Bridge adapter to use external MCP servers as Skreaver tools

use crate::error::McpResult;
use serde_json::Value;
use skreaver_core::tool::{ExecutionResult, Tool};
use std::sync::Arc;
use tracing::{debug, error};

/// Bridge that connects to an external MCP server and exposes its tools
/// as Skreaver tools
pub struct McpBridge {
    #[allow(dead_code)]
    server_name: String,
    tools: Vec<Arc<BridgedTool>>,
}

impl McpBridge {
    /// Create a new MCP bridge
    pub fn new(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            tools: Vec::new(),
        }
    }

    /// Connect to an external MCP server via stdio (child process)
    pub async fn connect_stdio(server_command: &str) -> McpResult<Self> {
        debug!(command = %server_command, "Connecting to MCP server");

        // TODO: Implement actual MCP client connection using rmcp
        // This would:
        // 1. Spawn the server process
        // 2. Connect via stdio using rmcp client
        // 3. Call list_tools to discover available tools
        // 4. Create BridgedTool wrappers for each

        Ok(Self {
            server_name: server_command.to_string(),
            tools: Vec::new(),
        })
    }

    /// Get all bridged tools
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools
            .iter()
            .map(|t| Arc::clone(t) as Arc<dyn Tool>)
            .collect()
    }
}

/// A tool from an external MCP server, adapted to Skreaver's Tool trait
struct BridgedTool {
    name: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    parameters: Value,
}

impl Tool for BridgedTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn call(&self, _input: String) -> ExecutionResult {
        // TODO: Implement actual MCP call_tool request
        // This would send the input to the external MCP server
        // and return the result
        //
        // For now, this is a placeholder that demonstrates the interface

        debug!(
            tool = %self.name,
            "Executing bridged MCP tool (placeholder)"
        );

        error!("MCP bridge tool execution not yet implemented");

        ExecutionResult::Failure {
            error: "MCP bridge not yet fully implemented".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_creation() {
        let bridge = McpBridge::new("test-server");
        assert_eq!(bridge.server_name, "test-server");
        assert_eq!(bridge.tools().len(), 0);
    }

    #[tokio::test]
    async fn test_bridge_connect_placeholder() {
        // This is a placeholder test
        // Real implementation would test actual MCP server connection
        let result = McpBridge::connect_stdio("echo").await;
        assert!(result.is_ok());
    }
}
