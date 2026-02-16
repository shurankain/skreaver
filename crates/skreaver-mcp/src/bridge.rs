//! Bridge adapter to use external MCP servers as Skreaver tools
//!
//! This module provides integration with external MCP servers, allowing them to be used
//! as Skreaver tools. The bridge handles:
//! - Spawning and connecting to MCP server processes
//! - Tool discovery via the MCP protocol
//! - Request/response translation between Skreaver and MCP formats
//!
//! # Example
//!
//! ```rust,ignore
//! use skreaver_mcp::bridge::McpBridge;
//!
//! // Connect to an MCP server
//! let bridge = McpBridge::connect_stdio("npx @modelcontextprotocol/server-weather").await?;
//!
//! // Get discovered tools
//! let tools = bridge.tools();
//!
//! // Register tools with Skreaver
//! for tool in tools {
//!     registry.register(tool);
//! }
//! ```

use crate::error::{McpError, McpResult};
use rmcp::{
    ClientHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, ClientCapabilities, ClientInfo, Content,
        Implementation, RawContent, Tool as McpToolInfo,
    },
    service::{Peer, RoleClient, RunningService},
    transport::child_process::TokioChildProcess,
};
use serde_json::Value;
use skreaver_core::tool::{ExecutionResult, Tool};
use std::borrow::Cow;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Bridge that connects to an external MCP server and exposes its tools
/// as Skreaver tools
///
/// The bridge maintains a connection to an external MCP server process and
/// translates tool calls between Skreaver's format and the MCP protocol.
pub struct McpBridge {
    server_name: String,
    tools: Vec<Arc<BridgedTool>>,
    service: RunningService<RoleClient, McpClientHandler>,
}

impl std::fmt::Debug for McpBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpBridge")
            .field("server_name", &self.server_name)
            .field("tool_count", &self.tools.len())
            .finish()
    }
}

/// Simple MCP client handler that implements ClientHandler trait
#[derive(Clone, Default)]
struct McpClientHandler {
    client_info: ClientInfo,
}

impl ClientHandler for McpClientHandler {
    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }
}

impl McpBridge {
    /// Connect to an external MCP server via stdio (child process)
    ///
    /// This method spawns an MCP server process and establishes a connection
    /// via standard input/output streams.
    ///
    /// # Parameters
    ///
    /// * `server_command` - Command to spawn the MCP server
    ///   (e.g., "npx @modelcontextprotocol/server-weather")
    ///
    /// # Returns
    ///
    /// A connected `McpBridge` with all discovered tools, or an error
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let bridge = McpBridge::connect_stdio("npx @modelcontextprotocol/server-weather").await?;
    /// println!("Connected! Found {} tools", bridge.tools().len());
    /// ```
    pub async fn connect_stdio(server_command: &str) -> McpResult<Self> {
        info!(command = %server_command, "Connecting to MCP server");

        // Parse command into program and arguments
        let parts: Vec<&str> = server_command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(McpError::InvalidParameters(
                "Empty server command".to_string(),
            ));
        }

        let program = parts[0];
        let args = &parts[1..];

        debug!(program = %program, args = ?args, "Spawning MCP server process");

        // Build tokio Command
        let mut cmd = Command::new(program);
        cmd.args(args);

        // Create child process transport
        let (transport, stderr) = TokioChildProcess::builder(cmd)
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| McpError::ConnectionError(format!("Failed to spawn process: {}", e)))?;

        // Spawn a task to log stderr
        if let Some(mut stderr) = stderr {
            let cmd_name = server_command.to_string();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut reader = tokio::io::BufReader::new(&mut stderr);
                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    debug!(server = %cmd_name, stderr = %line.trim(), "MCP server stderr");
                    line.clear();
                }
            });
        }

        // Create MCP client handler with info
        let handler = McpClientHandler {
            client_info: ClientInfo {
                meta: None,
                protocol_version: Default::default(),
                capabilities: ClientCapabilities {
                    sampling: Some(Default::default()),
                    elicitation: Some(Default::default()),
                    tasks: Some(rmcp::model::TasksCapability::client_default()),
                    ..Default::default()
                },
                client_info: Implementation {
                    name: "skreaver-mcp-bridge".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    ..Default::default()
                },
            },
        };

        // Connect to the MCP server
        let service = handler.serve(transport).await.map_err(|e| {
            McpError::ConnectionError(format!("Failed to initialize MCP client: {}", e))
        })?;

        info!("MCP client connected, discovering tools...");

        // Get the peer to make requests
        let peer = service.peer();

        // List all available tools
        let mcp_tools = peer
            .list_all_tools()
            .await
            .map_err(|e| McpError::ClientError(format!("Failed to list tools: {}", e)))?;

        info!(count = mcp_tools.len(), "Discovered MCP tools");

        // Create bridged tools
        let tools: Vec<Arc<BridgedTool>> = mcp_tools
            .into_iter()
            .map(|tool_info| {
                debug!(
                    name = %tool_info.name,
                    description = ?tool_info.description,
                    "Creating bridged tool"
                );
                Arc::new(BridgedTool::new(tool_info, peer.clone()))
            })
            .collect();

        Ok(Self {
            server_name: server_command.to_string(),
            tools,
            service,
        })
    }

    /// Connect to an external MCP server with custom arguments
    ///
    /// This is a more flexible version that allows specifying the program
    /// and arguments separately.
    ///
    /// # Parameters
    ///
    /// * `program` - The program to run (e.g., "npx", "python")
    /// * `args` - Arguments to pass to the program
    ///
    /// # Returns
    ///
    /// A connected `McpBridge` with all discovered tools, or an error
    pub async fn connect_with_args<I, S>(program: &str, args: I) -> McpResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let args_vec: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
        let full_command = format!("{} {}", program, args_vec.join(" "));
        Self::connect_stdio(&full_command).await
    }

    /// Get all bridged tools
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools
            .iter()
            .map(|t| Arc::clone(t) as Arc<dyn Tool>)
            .collect()
    }

    /// Get the server name/command
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Get the number of available tools
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Find a tool by name
    pub fn find_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| Arc::clone(t) as Arc<dyn Tool>)
    }

    /// Refresh the tool list from the MCP server
    ///
    /// This re-queries the MCP server for available tools and updates
    /// the internal tool cache. Useful when tools may be dynamically
    /// added or removed on the server side.
    ///
    /// # Returns
    ///
    /// The number of tools discovered, or an error
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut bridge = McpBridge::connect_stdio("npx server").await?;
    /// println!("Initial tools: {}", bridge.tool_count());
    ///
    /// // ... server may add/remove tools ...
    ///
    /// let count = bridge.refresh_tools().await?;
    /// println!("Updated tools: {}", count);
    /// ```
    pub async fn refresh_tools(&mut self) -> McpResult<usize> {
        info!(server = %self.server_name, "Refreshing tool list from MCP server");

        // Get the peer to make requests
        let peer = self.service.peer();

        // List all available tools
        let mcp_tools = peer
            .list_all_tools()
            .await
            .map_err(|e| McpError::ClientError(format!("Failed to list tools: {}", e)))?;

        info!(
            server = %self.server_name,
            count = mcp_tools.len(),
            "Tool list refreshed"
        );

        // Create bridged tools
        let tools: Vec<Arc<BridgedTool>> = mcp_tools
            .into_iter()
            .map(|tool_info| {
                debug!(
                    name = %tool_info.name,
                    description = ?tool_info.description,
                    "Creating bridged tool"
                );
                Arc::new(BridgedTool::new(tool_info, peer.clone()))
            })
            .collect();

        let count = tools.len();
        self.tools = tools;

        Ok(count)
    }

    /// Get tool names as a vector of strings
    ///
    /// Useful for displaying available tools without cloning all tool instances.
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name.clone()).collect()
    }

    /// Get tool information including name, description, and schema
    ///
    /// Returns a vector of tuples containing (name, description, input_schema)
    /// for all available tools.
    pub fn tool_info(&self) -> Vec<(String, String, &Value)> {
        self.tools
            .iter()
            .map(|t| (t.name.clone(), t.description.clone(), t.input_schema()))
            .collect()
    }

    /// Check if the connection to the MCP server is closed.
    pub fn is_closed(&self) -> bool {
        self.service.is_closed()
    }

    /// Gracefully close the connection to the MCP server.
    ///
    /// This cancels the service and waits for cleanup to complete, ensuring
    /// the child process is properly terminated.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let mut bridge = McpBridge::connect_stdio("npx server").await?;
    /// // ... use the bridge ...
    /// bridge.close().await?;
    /// ```
    pub async fn close(&mut self) -> McpResult<()> {
        info!(server = %self.server_name, "Closing MCP bridge connection");
        self.service
            .close()
            .await
            .map_err(|e| McpError::ConnectionError(format!("Failed to close connection: {}", e)))?;
        Ok(())
    }

    /// Gracefully close the connection with a timeout.
    ///
    /// Returns `Ok(true)` if shutdown completed within the timeout,
    /// `Ok(false)` if the timeout was reached.
    pub async fn close_with_timeout(&mut self, timeout: std::time::Duration) -> McpResult<bool> {
        info!(server = %self.server_name, ?timeout, "Closing MCP bridge with timeout");
        let result = self
            .service
            .close_with_timeout(timeout)
            .await
            .map_err(|e| McpError::ConnectionError(format!("Failed to close connection: {}", e)))?;
        Ok(result.is_some())
    }
}

/// A tool from an external MCP server, adapted to Skreaver's Tool trait
///
/// Each `BridgedTool` represents a tool exposed by an external MCP server.
/// When called, it translates the request to MCP format, sends it to the
/// external server, and translates the response back to Skreaver format.
pub struct BridgedTool {
    name: String,
    description: String,
    input_schema: Value,
    peer: Peer<RoleClient>,
}

impl BridgedTool {
    /// Create a new bridged tool from MCP tool info
    fn new(info: McpToolInfo, peer: Peer<RoleClient>) -> Self {
        Self {
            name: info.name.to_string(),
            description: info.description.map(|s| s.to_string()).unwrap_or_default(),
            input_schema: Value::Object((*info.input_schema).clone()),
            peer,
        }
    }

    /// Get the tool's input schema
    pub fn input_schema(&self) -> &Value {
        &self.input_schema
    }

    /// Get the tool's description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Call the tool asynchronously
    pub async fn call_async(&self, input: Value) -> McpResult<Value> {
        debug!(tool = %self.name, "Calling MCP tool");

        // Build the call request (2025-11-25 spec: includes meta and task fields)
        let params = CallToolRequestParams {
            meta: None,
            name: Cow::Owned(self.name.clone()),
            arguments: Some(input.as_object().cloned().unwrap_or_default()),
            task: None,
        };

        // Call the tool via MCP
        let result: CallToolResult = self
            .peer
            .call_tool(params)
            .await
            .map_err(|e| McpError::ToolExecutionFailed(format!("MCP call failed: {}", e)))?;

        // Check for tool error
        if result.is_error.unwrap_or(false) {
            let error_msg = extract_text_from_contents(&result.content);
            return Err(McpError::ToolExecutionFailed(error_msg));
        }

        // Convert result to JSON
        let output = contents_to_json(&result.content);
        Ok(output)
    }
}

impl Tool for BridgedTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Option<Value> {
        Some(self.input_schema.clone())
    }

    fn call(&self, input: String) -> ExecutionResult {
        debug!(tool = %self.name, "Bridged MCP tool called");

        // Parse input as JSON
        let input_value: Value = match serde_json::from_str(&input) {
            Ok(v) => v,
            Err(e) => {
                // Try wrapping as object with "input" key
                warn!(
                    tool = %self.name,
                    error = %e,
                    "Failed to parse input as JSON, wrapping in object"
                );
                serde_json::json!({ "input": input })
            }
        };

        // The Tool trait is synchronous but MCP calls are async.
        // Use tokio::runtime::Handle to bridge them.
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(e) => {
                error!(error = %e, "No tokio runtime available");
                return ExecutionResult::Failure {
                    reason: skreaver_core::FailureReason::InternalError {
                        message: "No async runtime available for MCP call".to_string(),
                    },
                };
            }
        };

        // Clone what we need for the async block
        let name = self.name.clone();
        let peer = self.peer.clone();

        // Execute the async call (2025-11-25 spec: CallToolRequestParams with meta/task)
        let result = handle.block_on(async move {
            let params = CallToolRequestParams {
                meta: None,
                name: Cow::Owned(name),
                arguments: Some(input_value.as_object().cloned().unwrap_or_default()),
                task: None,
            };

            peer.call_tool(params).await
        });

        match result {
            Ok(call_result) => {
                if call_result.is_error.unwrap_or(false) {
                    let error_msg = extract_text_from_contents(&call_result.content);
                    ExecutionResult::Failure {
                        reason: skreaver_core::FailureReason::Custom {
                            category: "mcp_tool_error".to_string(),
                            message: error_msg,
                        },
                    }
                } else {
                    let output = contents_to_json(&call_result.content);
                    ExecutionResult::Success {
                        output: serde_json::to_string(&output)
                            .unwrap_or_else(|_| output.to_string()),
                    }
                }
            }
            Err(e) => {
                error!(tool = %self.name, error = %e, "MCP tool call failed");
                ExecutionResult::Failure {
                    reason: skreaver_core::FailureReason::NetworkError {
                        message: format!("MCP call failed: {}", e),
                    },
                }
            }
        }
    }
}

/// Extract text content from MCP Content array
fn extract_text_from_contents(contents: &[Content]) -> String {
    contents
        .iter()
        .filter_map(|c| match &c.raw {
            RawContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert MCP Content array to JSON value
fn contents_to_json(contents: &[Content]) -> Value {
    if contents.is_empty() {
        return Value::Null;
    }

    if contents.len() == 1 {
        return content_to_json(&contents[0]);
    }

    Value::Array(contents.iter().map(content_to_json).collect())
}

/// Convert single MCP Content to JSON value
fn content_to_json(content: &Content) -> Value {
    match &content.raw {
        RawContent::Text(text) => {
            // Try to parse as JSON, otherwise return as string
            serde_json::from_str(&text.text).unwrap_or_else(|_| Value::String(text.text.clone()))
        }
        RawContent::Image(image) => {
            serde_json::json!({
                "type": "image",
                "data": image.data,
                "mime_type": image.mime_type
            })
        }
        RawContent::Audio(audio) => {
            serde_json::json!({
                "type": "audio",
                "data": audio.data,
                "mime_type": audio.mime_type
            })
        }
        RawContent::Resource(resource) => {
            serde_json::json!({
                "type": "resource",
                "resource": resource.resource
            })
        }
        RawContent::ResourceLink(link) => {
            serde_json::json!({
                "type": "resource_link",
                "uri": link.uri,
                "name": link.name,
                "mime_type": link.mime_type
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_content(text: &str) -> Content {
        Content::text(text)
    }

    #[test]
    fn test_extract_text_from_contents() {
        let contents = vec![make_text_content("Hello"), make_text_content("World")];

        let result = extract_text_from_contents(&contents);
        assert_eq!(result, "Hello\nWorld");
    }

    #[test]
    fn test_contents_to_json_single_text() {
        let contents = vec![make_text_content("{\"key\": \"value\"}")];

        let result = contents_to_json(&contents);
        assert_eq!(result, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_contents_to_json_plain_text() {
        let contents = vec![make_text_content("plain text")];

        let result = contents_to_json(&contents);
        assert_eq!(result, Value::String("plain text".to_string()));
    }

    #[test]
    fn test_contents_to_json_empty() {
        let contents: Vec<Content> = vec![];
        let result = contents_to_json(&contents);
        assert_eq!(result, Value::Null);
    }
}
