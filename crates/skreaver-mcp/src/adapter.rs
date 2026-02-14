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

    /// Convert to MCP tool definition (2025-11-25 spec)
    pub fn to_mcp_tool(&self) -> McpToolDefinition {
        // Use tool's own description if available, otherwise generate one
        let description = {
            let desc = self.tool.description();
            if desc.is_empty() {
                format!("Skreaver tool: {}", self.name())
            } else {
                desc.to_string()
            }
        };

        // Use tool's own input schema if available, otherwise use generic string input
        let input_schema = self.tool.input_schema().unwrap_or_else(|| {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Tool input as string"
                    }
                },
                "required": ["input"]
            })
        });

        // Use tool's own output schema if available
        let output_schema = self.tool.output_schema();

        McpToolDefinition {
            name: self.name().to_string(),
            title: None,
            description,
            input_schema,
            output_schema,
            annotations: None,
        }
    }

    /// Convert to MCP tool definition with annotations
    pub fn to_mcp_tool_with_annotations(
        &self,
        annotations: McpToolAnnotations,
    ) -> McpToolDefinition {
        let mut def = self.to_mcp_tool();
        def.annotations = Some(annotations);
        def
    }
}

/// MCP tool definition format (2025-11-25 spec)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolDefinition {
    /// Tool name (1-128 chars, alphanumeric + _ - .)
    pub name: String,
    /// Human-readable display title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema, defaults to 2020-12)
    pub input_schema: Value,
    /// Output schema (JSON Schema) - new in 2025-11-25
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    /// Tool behavior annotations - new in 2025-11-25
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<McpToolAnnotations>,
}

/// Tool behavior annotations (2025-11-25 spec)
///
/// These are hints about tool behavior. Clients should not make
/// tool use decisions based on annotations from untrusted servers.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct McpToolAnnotations {
    /// Whether the tool only reads without modifying its environment (default: false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    /// Whether modifications are destructive vs additive (default: true when not read-only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    /// Whether repeated identical calls produce no additional effects (default: false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    /// Whether the tool interacts with external entities (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
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
            .filter_map(|name| registry.get_tool(name).map(ToolAdapter::new))
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
