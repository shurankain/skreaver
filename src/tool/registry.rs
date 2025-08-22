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

    /// Dispatch a tool call with structured error handling.
    ///
    /// This method provides the same functionality as `dispatch` but with
    /// proper error types for better error handling and debugging.
    ///
    /// # Parameters
    ///
    /// * `call` - The tool call containing name and input data
    ///
    /// # Returns
    ///
    /// `Ok(ExecutionResult)` if successful, `Err(ToolError)` if the tool is not found
    fn try_dispatch(&self, call: ToolCall) -> Result<ExecutionResult, crate::error::ToolError> {
        self.dispatch(call.clone())
            .ok_or(crate::error::ToolError::NotFound { name: call.name })
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
/// use skreaver::tool::registry::{InMemoryToolRegistry, ToolRegistry};
/// use skreaver::tool::{Tool, ExecutionResult, ToolCall};
/// use std::sync::Arc;
///
/// struct EchoTool;
///
/// impl Tool for EchoTool {
///     fn name(&self) -> &str { "echo" }
///     fn call(&self, input: String) -> ExecutionResult {
///         ExecutionResult { output: input, success: true }
///     }
/// }
///
/// let registry = InMemoryToolRegistry::new()
///     .with_tool("echo", Arc::new(EchoTool));
///
/// let result = registry.dispatch(ToolCall {
///     name: "echo".to_string(),
///     input: "hello".to_string(),
/// });
/// ```
pub struct InMemoryToolRegistry {
    tools: HashMap<String, Arc<dyn super::Tool>>,
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
            tools: HashMap::new(),
        }
    }

    /// Add a tool to the registry using the builder pattern.
    ///
    /// This is a convenience method for chaining tool registrations
    /// during registry construction.
    ///
    /// # Parameters
    ///
    /// * `name` - The name to register the tool under
    /// * `tool` - The tool implementation wrapped in `Arc` for sharing
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_tool(mut self, name: &str, tool: Arc<dyn super::Tool>) -> Self {
        self.tools.insert(name.to_string(), tool);
        self
    }

    /// Add a tool to the registry with an owned string name.
    ///
    /// Use this when you already have an owned String to avoid cloning.
    ///
    /// # Parameters
    ///
    /// * `name` - The owned name string to register the tool under
    /// * `tool` - The tool implementation wrapped in `Arc` for sharing
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_tool_owned(mut self, name: String, tool: Arc<dyn super::Tool>) -> Self {
        self.tools.insert(name, tool);
        self
    }
}

impl super::registry::ToolRegistry for InMemoryToolRegistry {
    fn dispatch(&self, call: ToolCall) -> Option<ExecutionResult> {
        self.tools.get(&call.name).map(|tool| tool.call(call.input))
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
            ExecutionResult {
                output: input.to_uppercase(),
                success: true,
            }
        }
    }

    struct ReverseTool;

    impl Tool for ReverseTool {
        fn name(&self) -> &str {
            "reverse"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult {
                output: input.chars().rev().collect(),
                success: true,
            }
        }
    }

    #[test]
    fn registry_dispatches_to_correct_tool() {
        let registry = InMemoryToolRegistry::new()
            .with_tool("uppercase", Arc::new(UppercaseTool))
            .with_tool("reverse", Arc::new(ReverseTool));

        let upper = registry.dispatch(ToolCall {
            name: "uppercase".into(),
            input: "skreaver".into(),
        });

        let reversed = registry.dispatch(ToolCall {
            name: "reverse".into(),
            input: "skreaver".into(),
        });

        let missing = registry.dispatch(ToolCall {
            name: "nonexistent".into(),
            input: "skreaver".into(),
        });

        assert_eq!(upper.unwrap().output, "SKREAVER");
        assert_eq!(reversed.unwrap().output, "revaerks");
        assert!(missing.is_none());
    }
}
