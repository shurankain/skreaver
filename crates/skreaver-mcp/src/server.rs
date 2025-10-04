//! MCP Server implementation that exposes Skreaver tools

use crate::adapter::AdaptedToolRegistry;
use crate::error::{McpError, McpResult};
use skreaver_tools::InMemoryToolRegistry;
use tracing::{debug, info};

/// MCP Server that exposes Skreaver tools as MCP resources
pub struct McpServer {
    registry: AdaptedToolRegistry,
    server_info: ServerInfo,
}

/// Server information
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            name: "skreaver-mcp-server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Skreaver MCP Server - Expose Skreaver tools via Model Context Protocol"
                .to_string(),
        }
    }
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(tool_registry: &InMemoryToolRegistry) -> Self {
        // Convert Skreaver tools to MCP format
        let registry = AdaptedToolRegistry::from_registry(tool_registry);

        Self {
            registry,
            server_info: ServerInfo::default(),
        }
    }

    /// Create a new MCP server with custom server info
    pub fn with_info(tool_registry: &InMemoryToolRegistry, server_info: ServerInfo) -> Self {
        let mut server = Self::new(tool_registry);
        server.server_info = server_info;
        server
    }

    /// Create a new empty MCP server
    pub fn new_empty() -> Self {
        Self {
            registry: AdaptedToolRegistry::new(),
            server_info: ServerInfo::default(),
        }
    }

    /// Get mutable access to the tool registry
    pub fn registry_mut(&mut self) -> &mut AdaptedToolRegistry {
        &mut self.registry
    }

    /// Get server information
    pub fn info(&self) -> &ServerInfo {
        &self.server_info
    }

    /// Get the tool registry
    pub fn registry(&self) -> &AdaptedToolRegistry {
        &self.registry
    }

    /// Serve via stdio (stdin/stdout) - standard MCP transport
    ///
    /// This is the primary transport method for MCP servers,
    /// compatible with Claude Desktop and other MCP clients.
    pub async fn serve_stdio(self) -> McpResult<()> {
        info!(
            server = %self.server_info.name,
            version = %self.server_info.version,
            tools = self.registry.tools().len(),
            "Starting MCP server on stdio"
        );

        // TODO: Implement actual rmcp server integration
        // For now, this is a placeholder that demonstrates the API
        // The actual implementation requires:
        // 1. Creating an rmcp service that handles MCP protocol messages
        // 2. Mapping list_tools requests to our registry
        // 3. Mapping call_tool requests to our tool execution
        // 4. Handling MCP protocol lifecycle (initialize, shutdown, etc.)

        debug!("Registered tools:");
        for tool_def in self.registry.list_tools() {
            debug!("  - {}: {}", tool_def.name, tool_def.description);
        }

        // Placeholder: Would use rmcp to serve
        // let transport = (stdin(), stdout());
        // let service = McpService::new(self.registry);
        // service.serve(transport).await?;

        info!("MCP server ready - waiting for client connections");

        // For now, just return Ok to allow compilation
        // Real implementation will come after understanding rmcp API better
        Ok(())
    }

    /// List all available tools
    pub fn list_tools(&self) -> Vec<crate::adapter::McpToolDefinition> {
        self.registry.list_tools()
    }

    /// Call a tool by name
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        debug!(tool = %name, "Calling tool");

        let tool = self
            .registry
            .find(name)
            .ok_or_else(|| McpError::ToolNotFound(name.to_string()))?;

        // Call the tool synchronously in the current thread
        // (MCP tools are expected to be fast and non-blocking)
        let result = tool.call(arguments);

        debug!(tool = %name, success = result.is_ok(), "Tool execution completed");
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::tool::{ExecutionResult, Tool};
    use std::sync::Arc;

    struct TestTool;

    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test_tool"
        }

        fn call(&self, input: String) -> ExecutionResult {
            let parsed: serde_json::Value = serde_json::from_str(&input).unwrap();
            let message = parsed
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("default");

            ExecutionResult::Success {
                output: serde_json::json!({"echo": message}).to_string(),
            }
        }
    }

    #[test]
    fn test_server_creation() {
        let mut server = McpServer::new_empty();
        server.registry_mut().add_tool(Arc::new(TestTool));

        assert_eq!(server.info().name, "skreaver-mcp-server");
        assert_eq!(server.registry().tools().len(), 1);
    }

    #[test]
    fn test_list_tools() {
        let mut server = McpServer::new_empty();
        server.registry_mut().add_tool(Arc::new(TestTool));

        let tools = server.list_tools();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "test_tool");
    }

    #[tokio::test]
    async fn test_call_tool() {
        let mut server = McpServer::new_empty();
        server.registry_mut().add_tool(Arc::new(TestTool));

        let result = server
            .call_tool(
                "test_tool",
                serde_json::json!({
                    "message": "hello"
                }),
            )
            .await
            .unwrap();

        assert_eq!(result["echo"], "hello");
    }

    #[tokio::test]
    async fn test_call_unknown_tool() {
        let server = McpServer::new_empty();

        let result = server
            .call_tool("unknown_tool", serde_json::json!({}))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::ToolNotFound(_)));
    }
}
