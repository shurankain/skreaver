/// A request to invoke a specific tool with input data.
///
/// `ToolCall` represents an agent's intent to use an external capability.
/// The coordinator will route this call to the appropriate tool implementation
/// based on the name field.
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// The name of the tool to invoke.
    ///
    /// This must match a tool registered in the tool registry.
    pub name: String,

    /// The input data to pass to the tool.
    ///
    /// Tools are responsible for parsing and validating this input.
    pub input: String,
}

/// The result of executing a tool.
///
/// `ExecutionResult` contains both the output data from the tool
/// and a success indicator for error handling.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The output data produced by the tool.
    ///
    /// This can be any string data - plain text, JSON, XML, etc.
    /// The format depends on the specific tool implementation.
    pub output: String,

    /// Whether the tool execution was successful.
    ///
    /// Tools should set this to `false` when they encounter errors,
    /// invalid input, or cannot complete the requested operation.
    pub success: bool,
}

/// Trait defining an external capability that agents can invoke.
///
/// Tools extend agent functionality beyond internal reasoning to include
/// actions like API calls, file operations, calculations, and more.
/// Each tool has a unique name and can process string input to produce
/// structured output.
///
/// # Example
///
/// ```rust
/// use skreaver::tool::{Tool, ExecutionResult};
///
/// struct CalculatorTool;
///
/// impl Tool for CalculatorTool {
///     fn name(&self) -> &str {
///         "calculator"
///     }
///
///     fn call(&self, input: String) -> ExecutionResult {
///         if let Ok(num) = input.parse::<f64>() {
///             ExecutionResult {
///                 output: (num * 2.0).to_string(),
///                 success: true,
///             }
///         } else {
///             ExecutionResult {
///                 output: "Invalid number".to_string(),
///                 success: false,
///             }
///         }
///     }
/// }
/// ```
pub trait Tool {
    /// Returns the unique name identifier for this tool.
    ///
    /// The name is used by the tool registry to route tool calls
    /// to the correct implementation. Names should be unique within
    /// a registry and follow a consistent naming convention.
    ///
    /// # Returns
    ///
    /// A string slice containing the tool's name
    fn name(&self) -> &str;

    /// Execute the tool with the provided input.
    ///
    /// This method performs the tool's core functionality, processing
    /// the input string and returning a structured result. Tools should
    /// handle errors gracefully and set the success flag appropriately.
    ///
    /// # Parameters
    ///
    /// * `input` - The input data for the tool to process
    ///
    /// # Returns
    ///
    /// An `ExecutionResult` containing the output and success status
    fn call(&self, input: String) -> ExecutionResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult {
                output: format!("Echo: {input}"),
                success: true,
            }
        }
    }

    #[test]
    fn tool_can_echo_input() {
        let tool = EchoTool;
        let result = tool.call("Skreaver".into());
        assert_eq!(result.output, "Echo: Skreaver");
        assert!(result.success);
    }

    #[test]
    fn tool_reports_name() {
        let tool = EchoTool;
        assert_eq!(tool.name(), "echo");
    }
}
