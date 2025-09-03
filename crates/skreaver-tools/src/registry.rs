use super::{ExecutionResult, ToolCall};
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for managing and dispatching tool calls.
///
/// Tool registries maintain collections of available tools and route
/// incoming tool calls to the appropriate implementations. Different
/// registry types can provide varying capabilities like local storage,
/// distributed dispatch, or dynamic tool loading.
pub trait ToolRegistry {
    /// Dispatch a tool call to the appropriate tool implementation.
    ///
    /// Looks up the tool by name and executes it with the provided input.
    /// Returns `None` if the requested tool is not found in the registry.
    ///
    /// # Parameters
    ///
    /// * `call` - The tool call containing name and input data
    ///
    /// # Returns
    ///
    /// `Some(ExecutionResult)` if the tool exists, `None` otherwise
    fn dispatch(&self, call: ToolCall) -> Option<ExecutionResult>;

    /// Dispatch a tool call using a reference to avoid cloning.
    ///
    /// Zero-copy dispatch method that looks up and executes tools without
    /// taking ownership of the ToolCall. This eliminates cloning in hot paths.
    ///
    /// # Parameters
    ///
    /// * `call` - Reference to the tool call containing name and input data
    ///
    /// # Returns
    ///
    /// `Some(ExecutionResult)` if the tool exists, `None` otherwise
    fn dispatch_ref(&self, call: &ToolCall) -> Option<ExecutionResult> {
        // Default implementation for backward compatibility - clones the call
        self.dispatch(call.clone())
    }

    /// Dispatch a tool call with structured error handling.
    ///
    /// This method provides the same functionality as `dispatch_ref` but with
    /// proper error types for better error handling and debugging.
    ///
    /// # Parameters
    ///
    /// * `call` - Reference to the tool call containing name and input data
    ///
    /// # Returns
    ///
    /// `Ok(ExecutionResult)` if successful, `Err(ToolError)` if the tool is not found
    fn try_dispatch(&self, call: &ToolCall) -> Result<ExecutionResult, String> {
        self.dispatch_ref(call)
            .ok_or(format!("Tool not found: {}", call.name()))
    }
}

/// In-memory tool registry for local tool storage and dispatch.
///
/// `InMemoryToolRegistry` provides a simple, fast registry implementation
/// suitable for single-process agent systems. Tools are stored in a HashMap
/// and accessed by name for O(1) lookup performance.
///
/// # Example
///
/// ```rust
/// use skreaver_tools::{InMemoryToolRegistry, ToolRegistry};
/// use skreaver_core::{Tool, ExecutionResult, ToolCall};
/// use skreaver_tools::ToolName;
/// use std::sync::Arc;
///
/// struct EchoTool;
///
/// impl Tool for EchoTool {
///     fn name(&self) -> &str { "echo" }
///     fn call(&self, input: String) -> ExecutionResult {
///         ExecutionResult::Success { output: input }
///     }
/// }
///
/// let registry = InMemoryToolRegistry::new()
///     .with_tool("echo", Arc::new(EchoTool));
///
/// let result = registry.dispatch(ToolCall::new("echo", "hello").expect("Valid tool name"));
/// ```
#[derive(Clone)]
pub struct InMemoryToolRegistry {
    standard_tools: HashMap<super::StandardTool, Arc<dyn super::Tool>>,
    custom_tools: HashMap<super::ToolName, Arc<dyn super::Tool>>,
}

impl Default for InMemoryToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryToolRegistry {
    /// Create a new empty tool registry.
    ///
    /// # Returns
    ///
    /// A new `InMemoryToolRegistry` with no tools registered
    pub fn new() -> Self {
        Self {
            standard_tools: HashMap::new(),
            custom_tools: HashMap::new(),
        }
    }

    /// Add a tool to the registry using the builder pattern.
    ///
    /// This is a convenience method for chaining tool registrations
    /// during registry construction. The name will be validated.
    ///
    /// # Parameters
    ///
    /// * `name` - The name to register the tool under (will be validated)
    /// * `tool` - The tool implementation wrapped in `Arc` for sharing
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Panics
    ///
    /// Panics if the tool name is invalid. Use `try_with_tool` for error handling.
    pub fn with_tool(mut self, name: &str, tool: Arc<dyn super::Tool>) -> Self {
        if let Some(standard_tool) = super::StandardTool::from_name(name) {
            self.standard_tools.insert(standard_tool, tool);
        } else {
            let tool_name = super::ToolName::new(name).expect("Valid tool name");
            self.custom_tools.insert(tool_name, tool);
        }
        self
    }

    /// Try to add a tool to the registry using the builder pattern.
    ///
    /// This method validates the tool name and returns an error if invalid.
    ///
    /// # Parameters
    ///
    /// * `name` - The name to register the tool under (will be validated)
    /// * `tool` - The tool implementation wrapped in `Arc` for sharing
    ///
    /// # Returns
    ///
    /// `Ok(Self)` for method chaining, or `Err(InvalidToolName)` if name is invalid
    pub fn try_with_tool(
        mut self,
        name: &str,
        tool: Arc<dyn super::Tool>,
    ) -> Result<Self, super::InvalidToolName> {
        if let Some(standard_tool) = super::StandardTool::from_name(name) {
            self.standard_tools.insert(standard_tool, tool);
        } else {
            let tool_name = super::ToolName::new(name)?;
            self.custom_tools.insert(tool_name, tool);
        }
        Ok(self)
    }

    /// Add a standard tool to the registry.
    ///
    /// This provides compile-time validation for standard tools.
    ///
    /// # Parameters
    ///
    /// * `standard_tool` - The standard tool type
    /// * `tool` - The tool implementation wrapped in `Arc` for sharing
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_standard_tool(
        mut self,
        standard_tool: super::StandardTool,
        tool: Arc<dyn super::Tool>,
    ) -> Self {
        self.standard_tools.insert(standard_tool, tool);
        self
    }

    /// Add a tool to the registry with a validated ToolName.
    ///
    /// Use this when you already have a validated ToolName to avoid re-validation.
    ///
    /// # Parameters
    ///
    /// * `name` - The validated tool name
    /// * `tool` - The tool implementation wrapped in `Arc` for sharing
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_tool_validated(
        mut self,
        name: super::ToolName,
        tool: Arc<dyn super::Tool>,
    ) -> Self {
        self.custom_tools.insert(name, tool);
        self
    }
}

impl super::registry::ToolRegistry for InMemoryToolRegistry {
    fn dispatch(&self, call: ToolCall) -> Option<ExecutionResult> {
        match &call.dispatch {
            super::ToolDispatch::Standard(standard_tool) => self
                .standard_tools
                .get(standard_tool)
                .map(|tool| tool.call(call.input)),
            super::ToolDispatch::Custom(tool_name) => self
                .custom_tools
                .get(tool_name)
                .map(|tool| tool.call(call.input)),
        }
    }

    fn dispatch_ref(&self, call: &ToolCall) -> Option<ExecutionResult> {
        // Zero-copy implementation: only clone the input string, not the entire ToolCall
        match &call.dispatch {
            super::ToolDispatch::Standard(standard_tool) => self
                .standard_tools
                .get(standard_tool)
                .map(|tool| tool.call(call.input.clone())),
            super::ToolDispatch::Custom(tool_name) => self
                .custom_tools
                .get(tool_name)
                .map(|tool| tool.call(call.input.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tool;
    use std::sync::Arc;

    struct UppercaseTool;

    impl Tool for UppercaseTool {
        fn name(&self) -> &str {
            "uppercase"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult::Success {
                output: input.to_uppercase(),
            }
        }
    }

    struct ReverseTool;

    impl Tool for ReverseTool {
        fn name(&self) -> &str {
            "reverse"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult::Success {
                output: input.chars().rev().collect(),
            }
        }
    }

    #[test]
    fn registry_dispatches_to_correct_tool() {
        let registry = InMemoryToolRegistry::new()
            .with_tool("uppercase", Arc::new(UppercaseTool))
            .with_tool("reverse", Arc::new(ReverseTool));

        let upper =
            registry.dispatch(ToolCall::new("uppercase", "skreaver").expect("Valid tool name"));

        let reversed =
            registry.dispatch(ToolCall::new("reverse", "skreaver").expect("Valid tool name"));

        let missing =
            registry.dispatch(ToolCall::new("nonexistent", "skreaver").expect("Valid tool name"));

        assert_eq!(upper.unwrap().output(), "SKREAVER");
        assert_eq!(reversed.unwrap().output(), "revaerks");
        assert!(missing.is_none());
    }
}
