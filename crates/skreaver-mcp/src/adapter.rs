//! Adapter to convert Skreaver tools to MCP format

use serde_json::Value;
use skreaver_core::tool::{ExecutionResult, Tool};
use std::sync::Arc;

use crate::error::{McpError, McpResult};

/// Adapter that wraps a Skreaver tool for MCP compatibility
#[derive(Clone)]
pub struct ToolAdapter {
    tool: Arc<dyn Tool>,
}

impl ToolAdapter {
    /// Create a new tool adapter
    pub fn new(tool: Arc<dyn Tool>) -> Self {
        Self { tool }
    }

    /// Get the tool name
    pub fn name(&self) -> &str {
        self.tool.name()
    }

    /// Execute the tool with JSON input (synchronously, will be wrapped in async by server)
    pub fn call(&self, input: Value) -> McpResult<Value> {
        // Convert JSON input to string for Skreaver's Tool trait
        let input_str = serde_json::to_string(&input)
            .map_err(|e| McpError::InvalidParameters(e.to_string()))?;

        // Call the tool (synchronous)
        let result = self.tool.call(input_str);

        // Convert ExecutionResult to JSON
        self.execution_result_to_json(result)
    }

    /// Convert ExecutionResult to JSON Value
    fn execution_result_to_json(&self, result: ExecutionResult) -> McpResult<Value> {
        match result {
            ExecutionResult::Success { output, .. } => {
                // Try to parse output as JSON, fallback to string
                serde_json::from_str(&output).or_else(|_| {
                    Ok(serde_json::json!({
                        "output": output
                    }))
                })
            }
            ExecutionResult::Failure { reason, .. } => {
                Err(McpError::ToolExecutionFailed(reason.to_string()))
            }
        }
    }

    /// Convert to MCP tool definition
    pub fn to_mcp_tool(&self) -> McpToolDefinition {
        McpToolDefinition {
            name: self.name().to_string(),
            description: format!("Skreaver tool: {}", self.name()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Tool input as string"
                    }
                },
                "required": ["input"]
            }),
        }
    }
}

/// MCP tool definition format
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: Value,
}

/// Registry of adapted tools
#[derive(Clone)]
pub struct AdaptedToolRegistry {
    tools: Vec<ToolAdapter>,
}

impl AdaptedToolRegistry {
    /// Create a new adapted tool registry from a Skreaver InMemoryToolRegistry
    pub fn from_registry(registry: &skreaver_tools::InMemoryToolRegistry) -> Self {
        // Iterate over all tools in the registry and adapt them
        let tools = registry
            .tool_names()
            .iter()
            .filter_map(|name| registry.get_tool(name).map(|tool| ToolAdapter::new(tool)))
            .collect();

        Self { tools }
    }

    /// Create empty registry
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Add a tool to the registry
    pub fn add_tool(&mut self, tool: Arc<dyn Tool>) {
        self.tools.push(ToolAdapter::new(tool));
    }

    /// Get all tools
    pub fn tools(&self) -> &[ToolAdapter] {
        &self.tools
    }

    /// Find a tool by name
    pub fn find(&self, name: &str) -> Option<&ToolAdapter> {
        self.tools.iter().find(|t| t.name() == name)
    }

    /// Get all tool definitions for MCP
    pub fn list_tools(&self) -> Vec<McpToolDefinition> {
        self.tools.iter().map(|t| t.to_mcp_tool()).collect()
    }
}

impl Default for AdaptedToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::tool::ExecutionResult;

    struct MockTool;

    impl Tool for MockTool {
        fn name(&self) -> &str {
            "mock_tool"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult::Success {
                output: format!("Echo: {}", input),
            }
        }
    }

    #[test]
    fn test_tool_adapter() {
        let tool = Arc::new(MockTool);
        let adapter = ToolAdapter::new(tool);

        assert_eq!(adapter.name(), "mock_tool");

        let def = adapter.to_mcp_tool();
        assert_eq!(def.name, "mock_tool");
        assert!(def.input_schema.is_object());
    }

    #[test]
    fn test_tool_execution() {
        let tool = Arc::new(MockTool);
        let adapter = ToolAdapter::new(tool);

        let result = adapter.call(serde_json::json!({"test": "data"}));
        assert!(result.is_ok());
    }
}
