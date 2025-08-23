/// Validated tool name that prevents typos and ensures consistent naming.
///
/// `ToolName` is a newtype wrapper around `String` that provides compile-time
/// validation and prevents common errors like typos in tool names. It enforces
/// naming conventions and length limits to ensure tool names are valid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolName(String);

/// Errors that can occur when creating a `ToolName`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidToolName {
    /// Tool name is empty or contains only whitespace.
    Empty,
    /// Tool name exceeds the maximum allowed length.
    TooLong(usize),
    /// Tool name contains invalid characters.
    InvalidChars(String),
}

impl std::fmt::Display for InvalidToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidToolName::Empty => write!(f, "Tool name cannot be empty"),
            InvalidToolName::TooLong(len) => {
                write!(f, "Tool name too long: {} characters (max 64)", len)
            }
            InvalidToolName::InvalidChars(name) => {
                write!(f, "Tool name contains invalid characters: '{}'", name)
            }
        }
    }
}

impl std::error::Error for InvalidToolName {}

impl ToolName {
    /// Maximum allowed length for tool names.
    pub const MAX_LENGTH: usize = 64;

    /// Create a new validated tool name.
    ///
    /// # Parameters
    ///
    /// * `name` - The tool name string to validate
    ///
    /// # Returns
    ///
    /// `Ok(ToolName)` if valid, `Err(InvalidToolName)` if validation fails
    ///
    /// # Validation Rules
    ///
    /// - Must not be empty or only whitespace
    /// - Must not exceed 64 characters
    /// - Must contain only alphanumeric characters, underscores, and hyphens
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver::tool::ToolName;
    ///
    /// let name = ToolName::new("calculator").unwrap();
    /// assert_eq!(name.as_str(), "calculator");
    /// ```
    pub fn new(name: &str) -> Result<Self, InvalidToolName> {
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(InvalidToolName::Empty);
        }

        if trimmed.len() > Self::MAX_LENGTH {
            return Err(InvalidToolName::TooLong(trimmed.len()));
        }

        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(InvalidToolName::InvalidChars(trimmed.to_string()));
        }

        Ok(ToolName(trimmed.to_string()))
    }

    /// Get the tool name as a string slice.
    ///
    /// # Returns
    ///
    /// The validated tool name as a `&str`
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the length of the tool name in bytes.
    ///
    /// # Returns
    ///
    /// The length of the tool name
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the tool name is empty.
    ///
    /// # Returns
    ///
    /// `true` if the tool name is empty (this should never happen for validated names)
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert into the underlying string.
    ///
    /// # Returns
    ///
    /// The validated tool name as an owned `String`
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ToolName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for ToolName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for ToolName {
    type Error = InvalidToolName;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        ToolName::new(name)
    }
}

impl TryFrom<String> for ToolName {
    type Error = InvalidToolName;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        ToolName::new(&name)
    }
}

/// A request to invoke a specific tool with input data.
///
/// `ToolCall` represents an agent's intent to use an external capability.
/// The coordinator will route this call to the appropriate tool implementation
/// based on the name field.
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// The validated name of the tool to invoke.
    ///
    /// This must match a tool registered in the tool registry.
    /// Using `ToolName` prevents typos and ensures valid tool names.
    pub name: ToolName,

    /// The input data to pass to the tool.
    ///
    /// Tools are responsible for parsing and validating this input.
    pub input: String,
}

impl ToolCall {
    /// Create a new ToolCall from string references.
    ///
    /// This validates the tool name and creates a new ToolCall instance.
    ///
    /// # Parameters
    ///
    /// * `name` - The tool name (will be validated)
    /// * `input` - The input data
    ///
    /// # Returns
    ///
    /// `Ok(ToolCall)` if the name is valid, `Err(InvalidToolName)` otherwise
    pub fn new(name: &str, input: &str) -> Result<Self, InvalidToolName> {
        Ok(Self {
            name: ToolName::new(name)?,
            input: input.to_string(),
        })
    }

    /// Create a new ToolCall from a validated ToolName and input string.
    ///
    /// Use this when you already have a validated ToolName to avoid re-validation.
    ///
    /// # Parameters
    ///
    /// * `name` - The validated tool name
    /// * `input` - The input data
    ///
    /// # Returns
    ///
    /// A new `ToolCall` instance
    pub fn from_validated(name: ToolName, input: String) -> Self {
        Self { name, input }
    }

    /// Create a new ToolCall from owned strings with validation.
    ///
    /// # Parameters
    ///
    /// * `name` - The tool name string (will be validated)
    /// * `input` - The owned input string
    ///
    /// # Returns
    ///
    /// `Ok(ToolCall)` if the name is valid, `Err(InvalidToolName)` otherwise
    pub fn from_owned(name: String, input: String) -> Result<Self, InvalidToolName> {
        Ok(Self {
            name: ToolName::new(&name)?,
            input,
        })
    }

    /// Create a new builder for configuring ToolCall instances.
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver::tool::ToolCall;
    ///
    /// let call = ToolCall::builder()
    ///     .name("calculator")
    ///     .input("2 + 2")
    ///     .build()
    ///     .expect("Valid tool name");
    /// ```
    pub fn builder() -> ToolCallBuilder {
        ToolCallBuilder::default()
    }
}

/// Builder for configuring ToolCall instances with validation.
#[derive(Debug, Default)]
pub struct ToolCallBuilder {
    name: Option<String>,
    input: String,
}

impl ToolCallBuilder {
    /// Set the name of the tool to invoke.
    ///
    /// The name will be validated when `build()` is called.
    ///
    /// # Parameters
    ///
    /// * `name` - The tool name (will be validated)
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Set the input data for the tool.
    ///
    /// The input format depends on the specific tool implementation.
    ///
    /// # Parameters
    ///
    /// * `input` - The input data
    pub fn input(mut self, input: &str) -> Self {
        self.input = input.to_string();
        self
    }

    /// Build the configured ToolCall with validation.
    ///
    /// # Returns
    ///
    /// `Ok(ToolCall)` if the name is valid, `Err(BuildError)` otherwise
    pub fn build(self) -> Result<ToolCall, ToolCallBuildError> {
        let name = self.name.ok_or(ToolCallBuildError::MissingName)?;
        let tool_name = ToolName::new(&name).map_err(ToolCallBuildError::InvalidName)?;

        Ok(ToolCall {
            name: tool_name,
            input: self.input,
        })
    }
}

/// Errors that can occur when building a ToolCall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallBuildError {
    /// No tool name was provided.
    MissingName,
    /// The provided tool name is invalid.
    InvalidName(InvalidToolName),
}

impl std::fmt::Display for ToolCallBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolCallBuildError::MissingName => write!(f, "Tool name is required"),
            ToolCallBuildError::InvalidName(err) => write!(f, "Invalid tool name: {}", err),
        }
    }
}

impl std::error::Error for ToolCallBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToolCallBuildError::MissingName => None,
            ToolCallBuildError::InvalidName(err) => Some(err),
        }
    }
}

/// The result of executing a tool.
///
/// `ExecutionResult` represents either successful execution with output
/// or failed execution with an error message. This design makes it impossible
/// to have inconsistent success/failure states at compile time.
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// Tool executed successfully with the given output.
    ///
    /// The output can be any string data - plain text, JSON, XML, etc.
    /// The format depends on the specific tool implementation.
    Success(String),

    /// Tool execution failed with the given error message.
    ///
    /// This indicates that the tool encountered an error, received
    /// invalid input, or could not complete the requested operation.
    Failure(String),
}

impl ExecutionResult {
    /// Create a successful execution result.
    ///
    /// # Parameters
    ///
    /// * `output` - The successful output from the tool
    ///
    /// # Returns
    ///
    /// An `ExecutionResult::Success` variant
    pub fn success(output: String) -> Self {
        ExecutionResult::Success(output)
    }

    /// Create a failed execution result.
    ///
    /// # Parameters
    ///
    /// * `error_message` - Description of what went wrong
    ///
    /// # Returns
    ///
    /// An `ExecutionResult::Failure` variant
    pub fn failure(error_message: String) -> Self {
        ExecutionResult::Failure(error_message)
    }

    /// Check if the execution was successful.
    ///
    /// # Returns
    ///
    /// `true` if this is a Success variant, `false` otherwise
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionResult::Success(_))
    }

    /// Check if the execution failed.
    ///
    /// # Returns
    ///
    /// `true` if this is a Failure variant, `false` otherwise
    pub fn is_failure(&self) -> bool {
        matches!(self, ExecutionResult::Failure(_))
    }

    /// Get the output regardless of success/failure status.
    ///
    /// # Returns
    ///
    /// A reference to the output or error message
    pub fn output(&self) -> &str {
        match self {
            ExecutionResult::Success(output) | ExecutionResult::Failure(output) => output,
        }
    }

    /// Convert to a Result type for easier error handling.
    ///
    /// # Returns
    ///
    /// `Ok(output)` if successful, `Err(error_message)` if failed
    pub fn into_result(self) -> Result<String, String> {
        match self {
            ExecutionResult::Success(output) => Ok(output),
            ExecutionResult::Failure(error) => Err(error),
        }
    }
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
///             ExecutionResult::Success((num * 2.0).to_string())
///         } else {
///             ExecutionResult::Failure("Invalid number".to_string())
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
            ExecutionResult::Success(format!("Echo: {input}"))
        }
    }

    #[test]
    fn tool_can_echo_input() {
        let tool = EchoTool;
        let result = tool.call("Skreaver".into());
        assert_eq!(result.output(), "Echo: Skreaver");
        assert!(result.is_success());
    }

    #[test]
    fn tool_reports_name() {
        let tool = EchoTool;
        assert_eq!(tool.name(), "echo");
    }

    #[test]
    fn test_tool_call_builder() {
        let call = ToolCall::builder()
            .name("calculator")
            .input("2 + 2")
            .build()
            .expect("Valid tool name");

        assert_eq!(call.name.as_str(), "calculator");
        assert_eq!(call.input, "2 + 2");
    }

    #[test]
    fn test_tool_call_builder_defaults() {
        let result = ToolCall::builder().build();
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolCallBuildError::MissingName => {}
            _ => panic!("Expected MissingName error"),
        }
    }

    #[test]
    fn test_tool_call_new() {
        let call = ToolCall::new("test_tool", "test input").expect("Valid tool name");

        assert_eq!(call.name.as_str(), "test_tool");
        assert_eq!(call.input, "test input");
    }

    #[test]
    fn test_tool_call_from_owned() {
        let name = String::from("owned_tool");
        let input = String::from("owned input");
        let call = ToolCall::from_owned(name, input).expect("Valid tool name");

        assert_eq!(call.name.as_str(), "owned_tool");
        assert_eq!(call.input, "owned input");
    }

    #[test]
    fn test_tool_name_validation() {
        // Valid names
        assert!(ToolName::new("calculator").is_ok());
        assert!(ToolName::new("tool_name").is_ok());
        assert!(ToolName::new("tool-name").is_ok());
        assert!(ToolName::new("Tool123").is_ok());

        // Invalid names
        assert!(ToolName::new("").is_err());
        assert!(ToolName::new("   ").is_err());
        assert!(ToolName::new("tool with spaces").is_err());
        assert!(ToolName::new("tool@special").is_err());

        // Too long name
        let long_name = "a".repeat(65);
        assert!(ToolName::new(&long_name).is_err());
    }
}
