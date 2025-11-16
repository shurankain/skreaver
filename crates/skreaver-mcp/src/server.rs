//! MCP Server implementation that exposes Skreaver tools

use crate::adapter::AdaptedToolRegistry;
use crate::error::{McpError, McpResult};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{CallToolResult, Content, ErrorCode, ErrorData as RmcpError},
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use skreaver_tools::InMemoryToolRegistry;
use std::borrow::Cow;
use tracing::{debug, error, info};

/// MCP Server that exposes Skreaver tools as MCP resources
#[derive(Clone)]
pub struct McpServer {
    registry: AdaptedToolRegistry,
    server_info: ServerInfo,
    tool_router: ToolRouter<McpServer>,
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

/// Generic tool call request that can handle any tool
#[derive(Debug, Deserialize, Serialize, rmcp::schemars::JsonSchema)]
pub struct GenericToolRequest {
    #[schemars(description = "Tool-specific parameters as JSON")]
    pub params: serde_json::Value,
}

#[tool_router]
impl McpServer {
    /// Create a new MCP server
    pub fn new(tool_registry: &InMemoryToolRegistry) -> Self {
        // Convert Skreaver tools to MCP format
        let registry = AdaptedToolRegistry::from_registry(tool_registry);

        Self {
            registry,
            server_info: ServerInfo::default(),
            tool_router: Self::tool_router(),
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
            tool_router: Self::tool_router(),
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

        debug!("Registered tools:");
        for tool_def in self.registry.list_tools() {
            debug!("  - {}: {}", tool_def.name, tool_def.description);
        }

        info!("MCP server ready - waiting for client connections");

        // Use rmcp stdio transport
        let service = self
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| McpError::ServerError(format!("Failed to start server: {}", e)))?;

        // Wait for the service to complete
        service
            .waiting()
            .await
            .map_err(|e| McpError::ServerError(format!("Server error: {}", e)))?;

        info!("MCP server shutdown");
        Ok(())
    }

    /// List all available tools
    pub fn list_tools(&self) -> Vec<crate::adapter::McpToolDefinition> {
        self.registry.list_tools()
    }

    /// Dynamic tool dispatcher - routes calls to registered Skreaver tools
    ///
    /// This is a generic handler that works with any tool in the registry.
    /// The rmcp framework requires us to define tools at compile time with the #[tool]
    /// attribute, but we want to support dynamically registered tools.
    ///
    /// As a workaround, we define one generic tool handler that dispatches to
    /// the appropriate tool at runtime based on the tool name in the request.
    #[tool(description = "Execute a Skreaver tool")]
    async fn execute_tool(
        &self,
        Parameters(request): Parameters<GenericToolRequest>,
    ) -> Result<CallToolResult, RmcpError> {
        debug!("Executing tool with params: {:?}", request.params);

        // Extract tool name from params
        let tool_name = request
            .params
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RmcpError {
                code: ErrorCode(-32602),
                message: Cow::from("Missing 'tool_name' in params"),
                data: None,
            })?;

        // Get tool arguments
        let tool_args = request
            .params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        debug!(tool = %tool_name, "Calling tool");

        // Find the tool in our registry
        let tool = self.registry.find(tool_name).ok_or_else(|| RmcpError {
            code: ErrorCode(-32601),
            message: Cow::from(format!("Tool not found: {}", tool_name)),
            data: None,
        })?;

        // Call the tool
        let result = tool.call(tool_args).map_err(|e| {
            error!(tool = %tool_name, error = %e, "Tool execution failed");
            RmcpError {
                code: ErrorCode(-32603),
                message: Cow::from(format!("Tool execution failed: {}", e)),
                data: None,
            }
        })?;

        debug!(tool = %tool_name, "Tool execution completed successfully");

        // Convert result to MCP format
        let content = Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result)),
        );

        Ok(CallToolResult::success(vec![content]))
    }
}

/// Implement the ServerHandler trait for MCP protocol support
#[tool_handler]
impl ServerHandler for McpServer {
    // The tool_handler macro automatically implements the required methods
    // based on the #[tool] annotations above
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

    #[test]
    fn test_server_has_tool_router() {
        let server = McpServer::new_empty();
        // Just verify the server was created successfully with tool_router
        assert_eq!(server.info().name, "skreaver-mcp-server");
    }
}
