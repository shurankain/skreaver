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
    /// use skreaver_core::tool::ToolName;
    ///
    /// let name = ToolName::new("calculator").unwrap();
    /// assert_eq!(name.as_str(), "calculator");
    /// ```
    pub fn new(name: &str) -> Result<Self, InvalidToolName> {
        use crate::validation::IdentifierRules;

        let validated = IdentifierRules::TOOL_NAME
            .validate(name)
            .map_err(|e| match e {
                crate::validation::ValidationError::Empty => InvalidToolName::Empty,
                crate::validation::ValidationError::TooLong { length, .. } => {
                    InvalidToolName::TooLong(length)
                }
                crate::validation::ValidationError::InvalidChar { input, .. } => {
                    InvalidToolName::InvalidChars(input)
                }
            })?;

        Ok(ToolName(validated))
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

/// Standard tool types for strongly-typed dispatch.
///
/// This enum provides compile-time tool validation and eliminates
/// string-based lookup overhead in the hot path. Each variant
/// corresponds to a specific tool implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardTool {
    // Network tools
    HttpGet,
    HttpPost,
    HttpPut,
    HttpDelete,

    // File I/O tools
    FileRead,
    FileWrite,
    DirectoryList,
    DirectoryCreate,

    // Data processing tools
    JsonParse,
    JsonTransform,
    XmlParse,
    TextAnalyze,
    TextReverse,
    TextSearch,
    TextSplit,
    TextUppercase,
}

impl StandardTool {
    /// Get the tool name as a string for backwards compatibility.
    pub fn name(&self) -> &'static str {
        match self {
            StandardTool::HttpGet => "http_get",
            StandardTool::HttpPost => "http_post",
            StandardTool::HttpPut => "http_put",
            StandardTool::HttpDelete => "http_delete",
            StandardTool::FileRead => "file_read",
            StandardTool::FileWrite => "file_write",
            StandardTool::DirectoryList => "directory_list",
            StandardTool::DirectoryCreate => "directory_create",
            StandardTool::JsonParse => "json_parse",
            StandardTool::JsonTransform => "json_transform",
            StandardTool::XmlParse => "xml_parse",
            StandardTool::TextAnalyze => "text_analyze",
            StandardTool::TextReverse => "text_reverse",
            StandardTool::TextSearch => "text_search",
            StandardTool::TextSplit => "text_split",
            StandardTool::TextUppercase => "text_uppercase",
        }
    }

    /// Try to parse a tool name string into a StandardTool.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "http_get" => Some(StandardTool::HttpGet),
            "http_post" => Some(StandardTool::HttpPost),
            "http_put" => Some(StandardTool::HttpPut),
            "http_delete" => Some(StandardTool::HttpDelete),
            "file_read" => Some(StandardTool::FileRead),
            "file_write" => Some(StandardTool::FileWrite),
            "directory_list" => Some(StandardTool::DirectoryList),
            "directory_create" => Some(StandardTool::DirectoryCreate),
            "json_parse" => Some(StandardTool::JsonParse),
            "json_transform" => Some(StandardTool::JsonTransform),
            "xml_parse" => Some(StandardTool::XmlParse),
            "text_analyze" => Some(StandardTool::TextAnalyze),
            "text_reverse" => Some(StandardTool::TextReverse),
            "text_search" => Some(StandardTool::TextSearch),
            "text_split" => Some(StandardTool::TextSplit),
            "text_uppercase" => Some(StandardTool::TextUppercase),
            _ => None,
        }
    }

    /// Get all standard tools as a slice.
    pub fn all() -> &'static [StandardTool] {
        &[
            StandardTool::HttpGet,
            StandardTool::HttpPost,
            StandardTool::HttpPut,
            StandardTool::HttpDelete,
            StandardTool::FileRead,
            StandardTool::FileWrite,
            StandardTool::DirectoryList,
            StandardTool::DirectoryCreate,
            StandardTool::JsonParse,
            StandardTool::JsonTransform,
            StandardTool::XmlParse,
            StandardTool::TextAnalyze,
            StandardTool::TextReverse,
            StandardTool::TextSearch,
            StandardTool::TextSplit,
            StandardTool::TextUppercase,
        ]
    }
}

impl std::fmt::Display for StandardTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Tool dispatch method for improved type safety and performance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolDispatch {
    /// Dispatch to a standard tool using compile-time validation.
    Standard(StandardTool),
    /// Dispatch to a custom tool using runtime validation.
    Custom(ToolName),
}

impl ToolDispatch {
    /// Create a dispatch method from a tool name string.
    pub fn from_name(name: &str) -> Result<Self, InvalidToolName> {
        if let Some(standard_tool) = StandardTool::from_name(name) {
            Ok(ToolDispatch::Standard(standard_tool))
        } else {
            Ok(ToolDispatch::Custom(ToolName::new(name)?))
        }
    }

    /// Get the tool name as a string.
    pub fn name(&self) -> &str {
        match self {
            ToolDispatch::Standard(tool) => tool.name(),
            ToolDispatch::Custom(name) => name.as_str(),
        }
    }
}

/// A request to invoke a specific tool with input data.
///
/// `ToolCall` represents an agent's intent to use an external capability.
/// The coordinator will route this call to the appropriate tool implementation
/// based on the dispatch field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// The tool dispatch method (strongly-typed or custom).
    ///
    /// Using `ToolDispatch` enables compile-time validation for standard tools
    /// while still supporting custom tool extensions.
    pub dispatch: ToolDispatch,

    /// The input data to pass to the tool.
    ///
    /// Tools are responsible for parsing and validating this input.
    pub input: String,
}

impl ToolCall {
    /// Backwards compatibility: get the name field.
    pub fn name(&self) -> &str {
        self.dispatch.name()
    }

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
            dispatch: ToolDispatch::from_name(name)?,
            input: input.to_string(),
        })
    }

    /// Create a new ToolCall from a standard tool type.
    ///
    /// This provides compile-time validation for standard tools.
    ///
    /// # Parameters
    ///
    /// * `tool` - The standard tool type
    /// * `input` - The input data
    ///
    /// # Returns
    ///
    /// A new `ToolCall` instance
    pub fn from_standard(tool: StandardTool, input: String) -> Self {
        Self {
            dispatch: ToolDispatch::Standard(tool),
            input,
        }
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
        Self {
            dispatch: ToolDispatch::Custom(name),
            input,
        }
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
            dispatch: ToolDispatch::from_name(&name)?,
            input,
        })
    }

    /// Create a new builder for configuring ToolCall instances.
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_core::tool::ToolCall;
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
        let dispatch = ToolDispatch::from_name(&name).map_err(ToolCallBuildError::InvalidName)?;

        Ok(ToolCall {
            dispatch,
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
    Success { output: String },

    /// Tool execution failed with the given error.
    ///
    /// This indicates that the tool encountered an error, received
    /// invalid input, or could not complete the requested operation.
    Failure { error: String },
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
        ExecutionResult::Success { output }
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
        ExecutionResult::Failure {
            error: error_message,
        }
    }

    /// Check if the execution was successful.
    ///
    /// # Returns
    ///
    /// `true` if this is a Success variant, `false` otherwise
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionResult::Success { .. })
    }

    /// Check if the execution failed.
    ///
    /// # Returns
    ///
    /// `true` if this is a Failure variant, `false` otherwise
    pub fn is_failure(&self) -> bool {
        matches!(self, ExecutionResult::Failure { .. })
    }

    /// Get the output string (for success) or error message (for failure).
    ///
    /// # Returns
    ///
    /// A reference to the output string or error message
    pub fn output(&self) -> &str {
        match self {
            ExecutionResult::Success { output } => output,
            ExecutionResult::Failure { error } => error,
        }
    }

    /// Get the success output if available.
    ///
    /// # Returns
    ///
    /// `Some(output)` if successful, `None` if failed
    pub fn success_output(&self) -> Option<&str> {
        match self {
            ExecutionResult::Success { output } => Some(output),
            ExecutionResult::Failure { .. } => None,
        }
    }

    /// Get the error message if available.
    ///
    /// # Returns
    ///
    /// `Some(error)` if failed, `None` if successful
    pub fn error_message(&self) -> Option<&str> {
        match self {
            ExecutionResult::Success { .. } => None,
            ExecutionResult::Failure { error } => Some(error),
        }
    }

    /// Convert to a Result type for easier error handling.
    ///
    /// # Returns
    ///
    /// `Ok(output)` if successful, `Err(error_message)` if failed
    pub fn into_result(self) -> Result<String, String> {
        match self {
            ExecutionResult::Success { output } => Ok(output),
            ExecutionResult::Failure { error } => Err(error),
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
/// use skreaver_core::tool::{Tool, ExecutionResult};
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
///             ExecutionResult::Success { output: (num * 2.0).to_string() }
///         } else {
///             ExecutionResult::Failure { error: "Invalid number".to_string() }
///         }
///     }
/// }
/// ```
pub trait Tool: Send + Sync {
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

/// Non-empty queue of tool calls that prevents invalid states.
///
/// This type ensures that when an agent requests tool execution, there is always
/// at least one tool to execute. It prevents the common error where agents
/// expect tool results but provide an empty tool queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyToolQueue {
    first: ToolCall,
    rest: Vec<ToolCall>,
}

impl NonEmptyToolQueue {
    /// Create a new non-empty tool queue with a single tool call.
    pub fn new(first: ToolCall) -> Self {
        Self {
            first,
            rest: Vec::new(),
        }
    }

    /// Create a non-empty tool queue from a vector of tool calls.
    ///
    /// Returns `None` if the vector is empty.
    pub fn from_vec(mut tools: Vec<ToolCall>) -> Option<Self> {
        if tools.is_empty() {
            None
        } else {
            let first = tools.remove(0);
            Some(Self { first, rest: tools })
        }
    }

    /// Create a non-empty tool queue from a vector, returning an error if empty.
    pub fn try_from_vec(tools: Vec<ToolCall>) -> Result<Self, EmptyToolQueueError> {
        Self::from_vec(tools).ok_or(EmptyToolQueueError)
    }

    /// Add a tool call to the queue.
    pub fn push(&mut self, tool: ToolCall) {
        self.rest.push(tool);
    }

    /// Get the first tool call in the queue.
    pub fn first(&self) -> &ToolCall {
        &self.first
    }

    /// Get all tool calls as a slice.
    pub fn as_slice(&self) -> Vec<&ToolCall> {
        std::iter::once(&self.first)
            .chain(self.rest.iter())
            .collect()
    }

    /// Get the number of tool calls in the queue.
    pub fn len(&self) -> usize {
        1 + self.rest.len()
    }

    /// NonEmptyToolQueue is never empty by design.
    ///
    /// This method always returns `false` since a NonEmptyToolQueue
    /// is guaranteed to contain at least one tool call.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Check if the queue contains only one tool call.
    pub fn is_single(&self) -> bool {
        self.rest.is_empty()
    }

    /// Iterate over all tool calls in the queue.
    pub fn iter(&self) -> impl Iterator<Item = &ToolCall> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }

    /// Convert to a vector of tool calls.
    pub fn into_vec(self) -> Vec<ToolCall> {
        std::iter::once(self.first).chain(self.rest).collect()
    }

    /// Get a mutable reference to the first tool call.
    pub fn first_mut(&mut self) -> &mut ToolCall {
        &mut self.first
    }

    /// Get mutable references to all tool calls.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ToolCall> {
        std::iter::once(&mut self.first).chain(self.rest.iter_mut())
    }
}

impl IntoIterator for NonEmptyToolQueue {
    type Item = ToolCall;
    type IntoIter = std::iter::Chain<std::iter::Once<ToolCall>, std::vec::IntoIter<ToolCall>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.first).chain(self.rest)
    }
}

impl<'a> IntoIterator for &'a NonEmptyToolQueue {
    type Item = &'a ToolCall;
    type IntoIter = std::iter::Chain<std::iter::Once<&'a ToolCall>, std::slice::Iter<'a, ToolCall>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.first).chain(self.rest.iter())
    }
}

impl From<ToolCall> for NonEmptyToolQueue {
    fn from(tool: ToolCall) -> Self {
        Self::new(tool)
    }
}

impl TryFrom<Vec<ToolCall>> for NonEmptyToolQueue {
    type Error = EmptyToolQueueError;

    fn try_from(tools: Vec<ToolCall>) -> Result<Self, Self::Error> {
        Self::try_from_vec(tools)
    }
}

/// Error returned when trying to create a NonEmptyToolQueue from an empty vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyToolQueueError;

impl std::fmt::Display for EmptyToolQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cannot create NonEmptyToolQueue from empty vector")
    }
}

impl std::error::Error for EmptyToolQueueError {}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn call(&self, input: String) -> ExecutionResult {
            ExecutionResult::Success {
                output: format!("Echo: {input}"),
            }
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
            .name("http_get")
            .input("https://example.com")
            .build()
            .expect("Valid tool name");

        assert_eq!(call.name(), "http_get");
        assert_eq!(call.input, "https://example.com");
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
        let call = ToolCall::new("file_read", "test.txt").expect("Valid tool name");

        assert_eq!(call.name(), "file_read");
        assert_eq!(call.input, "test.txt");
    }

    #[test]
    fn test_tool_call_from_owned() {
        let name = String::from("json_parse");
        let input = String::from("{\"key\": \"value\"}");
        let call = ToolCall::from_owned(name, input).expect("Valid tool name");

        assert_eq!(call.name(), "json_parse");
        assert_eq!(call.input, "{\"key\": \"value\"}");
    }

    #[test]
    fn test_tool_call_from_standard() {
        let call = ToolCall::from_standard(StandardTool::HttpPost, "POST data".to_string());

        assert_eq!(call.name(), "http_post");
        assert_eq!(call.input, "POST data");
    }

    #[test]
    fn test_standard_tool_dispatch() {
        // Test standard tool recognition
        let dispatch = ToolDispatch::from_name("http_get").expect("Valid tool name");
        match dispatch {
            ToolDispatch::Standard(StandardTool::HttpGet) => {} // Expected
            _ => panic!("Expected StandardTool::HttpGet"),
        }

        // Test custom tool fallback
        let dispatch = ToolDispatch::from_name("custom_tool").expect("Valid tool name");
        match dispatch {
            ToolDispatch::Custom(name) => assert_eq!(name.as_str(), "custom_tool"),
            _ => panic!("Expected Custom dispatch"),
        }
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

    #[test]
    fn test_non_empty_tool_queue_creation() {
        let tool1 = ToolCall::new("tool1", "input1").unwrap();
        let queue = NonEmptyToolQueue::new(tool1.clone());

        assert_eq!(queue.len(), 1);
        assert!(queue.is_single());
        assert!(!queue.is_empty()); // NonEmptyToolQueue is never empty
        assert_eq!(queue.first(), &tool1);
    }

    #[test]
    fn test_non_empty_tool_queue_from_vec() {
        let tool1 = ToolCall::new("tool1", "input1").unwrap();
        let tool2 = ToolCall::new("tool2", "input2").unwrap();
        let tools = vec![tool1.clone(), tool2.clone()];

        let queue = NonEmptyToolQueue::from_vec(tools).unwrap();
        assert_eq!(queue.len(), 2);
        assert!(!queue.is_single());
        assert_eq!(queue.first(), &tool1);

        let all_tools: Vec<_> = queue.iter().collect();
        assert_eq!(all_tools.len(), 2);
        assert_eq!(all_tools[0], &tool1);
        assert_eq!(all_tools[1], &tool2);
    }

    #[test]
    fn test_non_empty_tool_queue_from_empty_vec() {
        let empty_tools: Vec<ToolCall> = vec![];
        let result = NonEmptyToolQueue::from_vec(empty_tools);
        assert!(result.is_none());

        let try_result = NonEmptyToolQueue::try_from_vec(vec![]);
        assert!(try_result.is_err());
        assert!(matches!(try_result.unwrap_err(), EmptyToolQueueError));
    }

    #[test]
    fn test_non_empty_tool_queue_push() {
        let tool1 = ToolCall::new("tool1", "input1").unwrap();
        let tool2 = ToolCall::new("tool2", "input2").unwrap();

        let mut queue = NonEmptyToolQueue::new(tool1.clone());
        assert_eq!(queue.len(), 1);

        queue.push(tool2.clone());
        assert_eq!(queue.len(), 2);
        assert!(!queue.is_single());

        let tools: Vec<_> = queue.iter().cloned().collect();
        assert_eq!(tools, vec![tool1, tool2]);
    }

    #[test]
    fn test_non_empty_tool_queue_iteration() {
        let tool1 = ToolCall::new("tool1", "input1").unwrap();
        let tool2 = ToolCall::new("tool2", "input2").unwrap();
        let tool3 = ToolCall::new("tool3", "input3").unwrap();

        let queue =
            NonEmptyToolQueue::from_vec(vec![tool1.clone(), tool2.clone(), tool3.clone()]).unwrap();

        // Test iterator
        let collected: Vec<_> = queue.iter().cloned().collect();
        assert_eq!(collected, vec![tool1.clone(), tool2.clone(), tool3.clone()]);

        // Test into_iter
        let collected: Vec<_> = queue.clone().into_iter().collect();
        assert_eq!(collected, vec![tool1.clone(), tool2.clone(), tool3.clone()]);

        // Test into_vec
        let vec_result = queue.into_vec();
        assert_eq!(vec_result, vec![tool1, tool2, tool3]);
    }

    #[test]
    fn test_non_empty_tool_queue_conversions() {
        let tool = ToolCall::new("test_tool", "test_input").unwrap();

        // Test From<ToolCall>
        let queue: NonEmptyToolQueue = tool.clone().into();
        assert_eq!(queue.first(), &tool);
        assert!(queue.is_single());

        // Test TryFrom<Vec<ToolCall>>
        let tools = vec![tool.clone()];
        let queue: NonEmptyToolQueue = tools.try_into().unwrap();
        assert_eq!(queue.first(), &tool);

        // Test TryFrom with empty vec fails
        let empty_tools: Vec<ToolCall> = vec![];
        let result: Result<NonEmptyToolQueue, _> = empty_tools.try_into();
        assert!(result.is_err());
    }
}
