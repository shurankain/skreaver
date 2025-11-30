//! Bridge adapter to use external MCP servers as Skreaver tools
//!
//! This module provides integration with external MCP servers, allowing them to be used
//! as Skreaver tools. The bridge handles:
//! - Spawning and connecting to MCP server processes
//! - Tool discovery via the MCP protocol
//! - Request/response translation between Skreaver and MCP formats
//!
//! # Implementation Notes
//!
//! To fully implement MCP client connection:
//!
//! 1. Use `rmcp::client::Client` to establish connection
//! 2. Call `client.list_tools()` to discover available tools
//! 3. For each tool, create a `BridgedTool` that stores a reference to the client
//! 4. In `BridgedTool::call()`, use `client.call_tool()` to execute requests
//!
//! # Example Implementation Sketch
//!
//! ```rust,ignore
//! use rmcp::client::{Client, StdioTransport};
//! use tokio::process::Command;
//!
//! // Spawn server process
//! let mut child = Command::new(server_command)
//!     .stdin(Stdio::piped())
//!     .stdout(Stdio::piped())
//!     .spawn()?;
//!
//! // Create transport from child process stdio
//! let transport = StdioTransport::new(
//!     child.stdin.take().unwrap(),
//!     child.stdout.take().unwrap(),
//! );
//!
//! // Create MCP client
//! let client = Client::new(transport).await?;
//!
//! // Discover tools
//! let tools_response = client.list_tools().await?;
//! for tool_info in tools_response.tools {
//!     let bridged = BridgedTool {
//!         name: tool_info.name,
//!         description: tool_info.description,
//!         parameters: tool_info.input_schema,
//!         client: Arc::clone(&client),
//!     };
//!     tools.push(Arc::new(bridged));
//! }
//! ```

use crate::error::McpResult;
use serde_json::Value;
use skreaver_core::tool::{ExecutionResult, Tool};
use std::sync::Arc;
use tracing::{debug, error};

/// Bridge that connects to an external MCP server and exposes its tools
/// as Skreaver tools
///
/// The bridge maintains a connection to an external MCP server process and
/// translates tool calls between Skreaver's format and the MCP protocol.
pub struct McpBridge {
    #[allow(dead_code)]
    server_name: String,
    tools: Vec<Arc<BridgedTool>>,
    // TODO: Add client field when implementing:
    // client: Arc<rmcp::client::Client>,
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
    ///
    /// This method spawns an MCP server process and establishes a connection
    /// via standard input/output streams.
    ///
    /// # Implementation Steps
    ///
    /// 1. **Spawn Server Process**
    ///    ```rust,ignore
    ///    use tokio::process::{Command, Stdio};
    ///    let mut child = Command::new(server_command)
    ///        .stdin(Stdio::piped())
    ///        .stdout(Stdio::piped())
    ///        .stderr(Stdio::piped())  // Capture errors
    ///        .spawn()?;
    ///    ```
    ///
    /// 2. **Create MCP Client**
    ///    ```rust,ignore
    ///    use rmcp::client::{Client, StdioTransport};
    ///    let transport = StdioTransport::new(
    ///        child.stdin.take().unwrap(),
    ///        child.stdout.take().unwrap(),
    ///    );
    ///    let client = Arc::new(Client::new(transport).await?);
    ///    ```
    ///
    /// 3. **Initialize and Discover Tools**
    ///    ```rust,ignore
    ///    // Initialize connection
    ///    client.initialize().await?;
    ///
    ///    // List available tools
    ///    let tools_response = client.list_tools().await?;
    ///    let tools: Vec<Arc<BridgedTool>> = tools_response.tools
    ///        .into_iter()
    ///        .map(|tool_info| {
    ///            Arc::new(BridgedTool {
    ///                name: tool_info.name,
    ///                description: tool_info.description.unwrap_or_default(),
    ///                parameters: tool_info.input_schema,
    ///                // Store client reference for call execution
    ///            })
    ///        })
    ///        .collect();
    ///    ```
    ///
    /// # Error Handling
    ///
    /// - Process spawn failures
    /// - Connection initialization failures
    /// - Tool discovery errors
    /// - Protocol version mismatches
    ///
    /// # Parameters
    ///
    /// * `server_command` - Command to spawn the MCP server (e.g., "npx @modelcontextprotocol/server-weather")
    ///
    /// # Returns
    ///
    /// A connected `McpBridge` with all discovered tools, or an error
    pub async fn connect_stdio(server_command: &str) -> McpResult<Self> {
        debug!(command = %server_command, "Connecting to MCP server");

        // Placeholder implementation until rmcp client API is stable
        // The rmcp crate is under active development and APIs may change
        //
        // Once ready, implement following the documentation above

        Ok(Self {
            server_name: server_command.to_string(),
            tools: Vec::new(),
            // client: client,
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
///
/// Each `BridgedTool` represents a tool exposed by an external MCP server.
/// When called, it translates the request to MCP format, sends it to the
/// external server, and translates the response back to Skreaver format.
struct BridgedTool {
    name: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    parameters: Value,
    // TODO: Add client reference when implementing:
    // client: Arc<rmcp::client::Client>,
}

impl Tool for BridgedTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn call(&self, _input: String) -> ExecutionResult {
        // Implementation guide for MCP tool execution:
        //
        // 1. Parse input as JSON to match tool's parameter schema
        //    ```rust,ignore
        //    let arguments: Value = serde_json::from_str(&input)
        //        .unwrap_or(json!({ "input": input }));
        //    ```
        //
        // 2. Send call_tool request to MCP server
        //    ```rust,ignore
        //    let response = tokio::runtime::Handle::current()
        //        .block_on(async {
        //            self.client.call_tool(&self.name, arguments).await
        //        })?;
        //    ```
        //
        // 3. Translate MCP response to ExecutionResult
        //    ```rust,ignore
        //    match response {
        //        ToolResponse::Success { content } => {
        //            // Extract text/data from content array
        //            let output = content.iter()
        //                .filter_map(|c| c.as_text())
        //                .collect::<Vec<_>>()
        //                .join("\n");
        //            ExecutionResult::Success { output }
        //        }
        //        ToolResponse::Error { error, .. } => {
        //            ExecutionResult::Failure { error: error.message }
        //        }
        //    }
        //    ```
        //
        // 4. Handle errors gracefully
        //    - Network errors
        //    - Protocol errors
        //    - Tool execution errors
        //
        // Note: The Tool trait is synchronous but MCP calls are async.
        // Use tokio::runtime::Handle::current().block_on() to bridge them,
        // or consider making Tool trait async in the future.

        debug!(
            tool = %self.name,
            "Bridged MCP tool called (placeholder implementation)"
        );

        error!(
            "MCP bridge tool execution not yet implemented. \
            Add rmcp client integration following the implementation guide above."
        );

        ExecutionResult::Failure {
            reason: skreaver_core::FailureReason::InternalError {
                message: format!(
                    "MCP bridge not yet fully implemented for tool '{}'. \
                    See bridge.rs for implementation guide.",
                    self.name
                ),
            },
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
