//! MCP Server implementation that exposes Skreaver tools
//!
//! This module provides an MCP server that exposes Skreaver tools via the
//! Model Context Protocol, making them accessible to Claude Desktop and
//! other MCP clients.

use crate::adapter::AdaptedToolRegistry;
use crate::error::{McpError, McpResult};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo, TasksCapability},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use skreaver_core::tool::Tool;
use skreaver_tools::InMemoryToolRegistry;
use std::sync::Arc;
use tracing::{debug, error, info};

/// MCP Server that exposes Skreaver tools as MCP resources
#[derive(Clone)]
pub struct McpServer {
    registry: Arc<AdaptedToolRegistry>,
    server_name: String,
    server_version: String,
    tool_router: ToolRouter<Self>,
}

/// Generic tool call request that can handle any tool
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ExecuteToolRequest {
    /// Name of the tool to execute
    #[schemars(description = "Name of the Skreaver tool to execute")]
    pub tool_name: String,

    /// Tool-specific arguments as JSON
    #[schemars(description = "Tool-specific arguments as JSON object")]
    #[serde(default)]
    pub arguments: serde_json::Value,
}

#[tool_router(router = tool_router)]
impl McpServer {
    /// Create a new MCP server
    pub fn new(tool_registry: &InMemoryToolRegistry) -> Self {
        let registry = AdaptedToolRegistry::from_registry(tool_registry);

        Self {
            registry: Arc::new(registry),
            server_name: "skreaver-mcp-server".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            tool_router: Self::tool_router(),
        }
    }

    /// Create a new MCP server with custom name and version
    pub fn with_info(
        tool_registry: &InMemoryToolRegistry,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        let mut server = Self::new(tool_registry);
        server.server_name = name.into();
        server.server_version = version.into();
        server
    }

    /// Create a new empty MCP server
    pub fn new_empty() -> Self {
        Self {
            registry: Arc::new(AdaptedToolRegistry::new()),
            server_name: "skreaver-mcp-server".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            tool_router: Self::tool_router(),
        }
    }

    /// Add a tool to the registry
    pub fn add_tool(&mut self, tool: Arc<dyn Tool>) {
        let mut registry = (*self.registry).clone();
        registry.add_tool(tool);
        self.registry = Arc::new(registry);
    }

    /// Get the tool registry
    pub fn registry(&self) -> &AdaptedToolRegistry {
        &self.registry
    }

    /// List all available tools
    pub fn list_tools(&self) -> Vec<crate::adapter::McpToolDefinition> {
        self.registry.list_tools()
    }

    /// Serve via stdio (stdin/stdout) - standard MCP transport
    ///
    /// This is the primary transport method for MCP servers,
    /// compatible with Claude Desktop and other MCP clients.
    pub async fn serve_stdio(self) -> McpResult<()> {
        info!(
            server = %self.server_name,
            version = %self.server_version,
            tools = self.registry.tools().len(),
            "Starting MCP server on stdio"
        );

        debug!("Registered tools:");
        for tool_def in self.registry.list_tools() {
            debug!("  - {}: {}", tool_def.name, tool_def.description);
        }

        info!("MCP server ready - waiting for client connections");

        let service = self
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| McpError::ServerError(format!("Failed to start server: {}", e)))?;

        service
            .waiting()
            .await
            .map_err(|e| McpError::ServerError(format!("Server error: {}", e)))?;

        info!("MCP server shutdown");
        Ok(())
    }

    /// Dynamic tool dispatcher - routes calls to registered Skreaver tools
    #[tool(
        name = "execute_tool",
        description = "Execute a Skreaver tool by name. Use 'list_skreaver_tools' to see available tools."
    )]
    async fn execute_tool(
        &self,
        request: Parameters<ExecuteToolRequest>,
    ) -> Result<String, String> {
        let tool_name = &request.0.tool_name;

        debug!(tool = %tool_name, "Executing Skreaver tool");

        // Validate tool name
        const MAX_TOOL_NAME_LEN: usize = 256;
        if tool_name.is_empty() {
            return Err("Tool name cannot be empty".to_string());
        }
        if tool_name.len() > MAX_TOOL_NAME_LEN {
            return Err(format!(
                "Tool name too long: {} chars (max {})",
                tool_name.len(),
                MAX_TOOL_NAME_LEN
            ));
        }
        if !tool_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(
                "Tool name contains invalid characters (only alphanumeric, _, -, . allowed)"
                    .to_string(),
            );
        }

        // Find the tool in our registry
        let tool = match self.registry.find(tool_name) {
            Some(t) => t,
            None => {
                return Err(format!("Tool not found: {}", tool_name));
            }
        };

        // Call the tool
        let result = match tool.call(request.0.arguments.clone()) {
            Ok(r) => r,
            Err(e) => {
                error!(tool = %tool_name, error = %e, "Tool execution failed");
                return Err(format!("Tool execution failed: {}", e));
            }
        };

        debug!(tool = %tool_name, "Tool execution completed successfully");

        Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result)))
    }

    /// List all available Skreaver tools
    #[tool(
        name = "list_skreaver_tools",
        description = "List all available Skreaver tools with their descriptions"
    )]
    async fn list_skreaver_tools(&self) -> String {
        let tools = self.registry.list_tools();

        let tool_list: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description
                })
            })
            .collect();

        serde_json::to_string_pretty(&tool_list).unwrap_or_else(|_| "[]".to_string())
    }
}

/// Implement ServerHandler trait for MCP protocol (2025-11-25 spec)
#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability {
                    list_changed: Some(true),
                }),
                tasks: Some(TasksCapability::server_default()),
                ..Default::default()
            },
            server_info: Implementation {
                name: self.server_name.clone(),
                version: self.server_version.clone(),
                title: Some(format!("Skreaver MCP Server ({})", self.server_name)),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "This server exposes Skreaver tools via MCP 2025-11-25. Use 'list_skreaver_tools' to see available tools, then 'execute_tool' to run them.".to_string()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::tool::ExecutionResult;

    struct TestTool;

    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test_tool"
        }

        fn call(&self, input: String) -> ExecutionResult {
            let parsed: serde_json::Value =
                serde_json::from_str(&input).unwrap_or(serde_json::json!({}));
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
        server.add_tool(Arc::new(TestTool));

        assert_eq!(server.server_name, "skreaver-mcp-server");
        assert_eq!(server.registry().tools().len(), 1);
    }

    #[test]
    fn test_list_tools() {
        let mut server = McpServer::new_empty();
        server.add_tool(Arc::new(TestTool));

        let tools = server.list_tools();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "test_tool");
    }

    #[test]
    fn test_server_info() {
        let server = McpServer::new_empty();
        let info = server.get_info();
        assert_eq!(info.server_info.name, "skreaver-mcp-server");
        assert!(!info.server_info.version.is_empty());
    }
}
