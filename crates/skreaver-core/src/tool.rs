// ToolName has been deprecated in favor of ToolId directly.
// All validation is now handled by ToolId which provides the same security guarantees.
// This reduces unnecessary type wrapping and allocation overhead.

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
    Custom(crate::ToolId),
}

impl ToolDispatch {
    /// Create a dispatch method from a tool name string.
    #[allow(deprecated)]
    pub fn from_name(name: &str) -> Result<Self, crate::IdValidationError> {
        if let Some(standard_tool) = StandardTool::from_name(name) {
            Ok(ToolDispatch::Standard(standard_tool))
        } else {
            Ok(ToolDispatch::Custom(crate::ToolId::parse(name)?))
        }
    }

    /// Get the tool name as a string.
    pub fn name(&self) -> &str {
        match self {
            ToolDispatch::Standard(tool) => tool.name(),
            ToolDispatch::Custom(tool_id) => tool_id.as_str(),
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
    /// `Ok(ToolCall)` if the name is valid, `Err(IdValidationError)` otherwise
    #[allow(deprecated)]
    pub fn new(name: &str, input: &str) -> Result<Self, crate::IdValidationError> {
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
    pub fn from_validated(name: crate::ToolId, input: String) -> Self {
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
    /// `Ok(ToolCall)` if the name is valid, `Err(IdValidationError)` otherwise
    #[allow(deprecated)]
    pub fn from_owned(name: String, input: String) -> Result<Self, crate::IdValidationError> {
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
#[allow(deprecated)]
pub enum ToolCallBuildError {
    /// No tool name was provided.
    MissingName,
    /// The provided tool ID is invalid.
    InvalidName(crate::IdValidationError),
}

#[allow(deprecated)]
impl std::fmt::Display for ToolCallBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolCallBuildError::MissingName => write!(f, "Tool name is required"),
            ToolCallBuildError::InvalidName(err) => write!(f, "Invalid tool ID: {}", err),
        }
    }
}

#[allow(deprecated)]
impl std::error::Error for ToolCallBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ToolCallBuildError::MissingName => None,
            ToolCallBuildError::InvalidName(err) => Some(err),
        }
    }
}

/// Categorized failure reasons for tool execution.
///
/// This enum provides structured error information instead of plain strings,
/// making it easier to handle different failure types programmatically.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FailureReason {
    /// Invalid or malformed input provided to the tool
    InvalidInput {
        /// Description of what was invalid
        message: String,
    },
    /// Required resource not found (file, URL, etc.)
    NotFound {
        /// What was not found
        resource: String,
    },
    /// Permission denied or unauthorized access
    PermissionDenied {
        /// What operation was denied
        message: String,
    },
    /// Network-related failure
    NetworkError {
        /// Description of the network issue
        message: String,
    },
    /// I/O operation failed
    IoError {
        /// Description of the I/O failure
        message: String,
    },
    /// Timeout exceeded
    Timeout {
        /// What operation timed out
        operation: String,
    },
    /// Internal tool error or unexpected state
    InternalError {
        /// Description of the internal error
        message: String,
    },
    /// Custom error for tool-specific failures
    Custom {
        /// Error category or code
        category: String,
        /// Error message
        message: String,
    },
}

impl FailureReason {
    /// Get a human-readable error message
    pub fn message(&self) -> String {
        match self {
            FailureReason::InvalidInput { message } => format!("Invalid input: {}", message),
            FailureReason::NotFound { resource } => format!("Not found: {}", resource),
            FailureReason::PermissionDenied { message } => {
                format!("Permission denied: {}", message)
            }
            FailureReason::NetworkError { message } => format!("Network error: {}", message),
            FailureReason::IoError { message } => format!("I/O error: {}", message),
            FailureReason::Timeout { operation } => format!("Timeout: {}", operation),
            FailureReason::InternalError { message } => format!("Internal error: {}", message),
            FailureReason::Custom { category, message } => format!("{}: {}", category, message),
        }
    }
}

impl std::fmt::Display for FailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// The result of executing a tool.
///
/// `ExecutionResult` represents either successful execution with output
/// or failed execution with a structured failure reason. This design makes it
/// impossible to have inconsistent success/failure states at compile time.
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// Tool executed successfully with the given output.
    ///
    /// The output can be any string data - plain text, JSON, XML, etc.
    /// The format depends on the specific tool implementation.
    Success { output: String },

    /// Tool execution failed with a structured reason.
    ///
    /// This indicates that the tool encountered an error, received
    /// invalid input, or could not complete the requested operation.
    Failure { reason: FailureReason },
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

    /// Create a failed execution result with a structured reason.
    ///
    /// # Parameters
    ///
    /// * `reason` - The structured failure reason
    ///
    /// # Returns
    ///
    /// An `ExecutionResult::Failure` variant
    pub fn failed(reason: FailureReason) -> Self {
        ExecutionResult::Failure { reason }
    }

    /// Create a failed execution result from a plain error message.
    ///
    /// This is a convenience method for backward compatibility.
    /// The message is wrapped in `FailureReason::InternalError`.
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
            reason: FailureReason::InternalError {
                message: error_message,
            },
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
    /// The output string or error message
    pub fn output(&self) -> String {
        match self {
            ExecutionResult::Success { output } => output.clone(),
            ExecutionResult::Failure { reason } => reason.message(),
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

    /// Get the failure reason if available.
    ///
    /// # Returns
    ///
    /// `Some(reason)` if failed, `None` if successful
    pub fn failure_reason(&self) -> Option<&FailureReason> {
        match self {
            ExecutionResult::Success { .. } => None,
            ExecutionResult::Failure { reason } => Some(reason),
        }
    }

    /// Get the error message if available.
    ///
    /// # Returns
    ///
    /// `Some(error)` if failed, `None` if successful
    pub fn error_message(&self) -> Option<String> {
        match self {
            ExecutionResult::Success { .. } => None,
            ExecutionResult::Failure { reason } => Some(reason.message()),
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
            ExecutionResult::Failure { reason } => Err(reason.message()),
        }
    }
}

/// Validated tool input.
///
/// This newtype wraps a String and ensures it has been validated for
/// security concerns (secrets, injection attacks, etc.) before being
/// passed to tool implementations.
///
/// # Type Safety
///
/// By using a newtype, we ensure at compile time that only validated
/// input can be passed to tools. This prevents accidentally passing
/// unvalidated user input directly to sensitive operations.
///
/// # Example
///
/// ```rust
/// use skreaver_core::tool::ToolInput;
///
/// // Can only be created through validation
/// let input = ToolInput::new_unchecked("safe input".to_string());
/// let value: &str = input.as_str();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ToolInput(String);

impl ToolInput {
    /// Create a new ToolInput without validation.
    ///
    /// # Safety
    ///
    /// This bypasses validation and should only be used when:
    /// - The input is known to be safe (e.g., from trusted sources)
    /// - The input has already been validated
    /// - The tool will perform its own validation
    ///
    /// For user-provided input, use `validate()` instead.
    pub fn new_unchecked(input: String) -> Self {
        Self(input)
    }

    /// Create a validated ToolInput from user-provided string.
    ///
    /// This method performs security validation including:
    /// - Secret detection (API keys, passwords, tokens)
    /// - Injection attack patterns (SQL, command injection)
    /// - Length limits
    ///
    /// # Parameters
    ///
    /// * `input` - The raw input string to validate
    /// * `policy` - Security policy to validate against
    ///
    /// # Returns
    ///
    /// `Ok(ToolInput)` if validation passes, `Err(SecurityError)` otherwise
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_core::tool::ToolInput;
    /// use skreaver_core::security::{SecurityPolicy, policy::*};
    ///
    /// let policy = SecurityPolicy {
    ///     fs_policy: FileSystemPolicy::default(),
    ///     http_policy: HttpPolicy::default(),
    ///     network_policy: NetworkPolicy::default(),
    /// };
    /// let input = ToolInput::validate("safe input".to_string(), &policy);
    /// ```
    #[cfg(feature = "security-basic")]
    pub fn validate(
        input: String,
        policy: &crate::security::SecurityPolicy,
    ) -> Result<Self, crate::security::SecurityError> {
        use crate::security::validation::InputValidator;

        let validator = InputValidator::new(policy);
        validator.validate(&input)?;
        Ok(Self(input))
    }

    /// Get the input as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into the inner String.
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Get the length of the input in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl AsRef<str> for ToolInput {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for ToolInput {
    fn from(s: String) -> Self {
        Self::new_unchecked(s)
    }
}

impl From<ToolInput> for String {
    fn from(input: ToolInput) -> Self {
        input.0
    }
}

impl std::fmt::Display for ToolInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
/// use skreaver_core::tool::{Tool, ExecutionResult, FailureReason};
///
/// struct CalculatorTool;
///
/// impl Tool for CalculatorTool {
///     fn name(&self) -> &str {
///         "calculator"
///     }
///
///     fn description(&self) -> &str {
///         "Multiplies a number by 2"
///     }
///
///     fn call(&self, input: String) -> ExecutionResult {
///         if let Ok(num) = input.parse::<f64>() {
///             ExecutionResult::Success { output: (num * 2.0).to_string() }
///         } else {
///             ExecutionResult::Failure {
///                 reason: FailureReason::InvalidInput {
///                     message: "Invalid number".to_string()
///                 }
///             }
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

    /// Returns a human-readable description of the tool.
    ///
    /// This description is used in tool listings and help text.
    /// Override this to provide a meaningful description.
    ///
    /// # Returns
    ///
    /// A string slice containing the tool's description
    fn description(&self) -> &str {
        ""
    }

    /// Returns the JSON Schema for the tool's input.
    ///
    /// Override this to provide a specific input schema for the tool.
    /// The default implementation returns `None`, indicating a generic
    /// string input schema should be used.
    ///
    /// # Returns
    ///
    /// An optional JSON Value containing the input schema
    fn input_schema(&self) -> Option<serde_json::Value> {
        None
    }

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

/// Extension trait for tools that provide structured results with metadata.
///
/// This trait allows tools to opt-in to providing rich execution metadata
/// alongside their results. Tools implementing this trait can track timing,
/// add tags, and preserve diagnostic context throughout the execution pipeline.
///
/// # Example
///
/// ```rust
/// use skreaver_core::tool::{Tool, ExecutionResult, StructuredTool};
/// use skreaver_core::{StructuredToolResult, ToolResultBuilder};
/// use chrono::Utc;
///
/// struct TimedCalculator;
///
/// impl Tool for TimedCalculator {
///     fn name(&self) -> &str { "calculator" }
///     fn call(&self, input: String) -> ExecutionResult {
///         // Simple implementation for backwards compatibility
///         ExecutionResult::success("42".to_string())
///     }
/// }
///
/// impl StructuredTool for TimedCalculator {
///     fn call_structured(&self, input: String) -> StructuredToolResult {
///         let start = Utc::now();
///
///         // Perform calculation
///         let result = input.parse::<f64>()
///             .map(|n| n * 2.0);
///
///         match result {
///             Ok(value) => {
///                 ToolResultBuilder::new(self.name())
///                     .started_at(start)
///                     .tag("math")
///                     .metadata("operation", "multiply")
///                     .success(value.to_string())
///             }
///             Err(_) => {
///                 ToolResultBuilder::new(self.name())
///                     .started_at(start)
///                     .tag("math")
///                     .failure("Invalid number", true)
///             }
///         }
///     }
/// }
/// ```
pub trait StructuredTool: Tool {
    /// Execute the tool with structured result tracking.
    ///
    /// This method should perform the same functionality as `call()` but
    /// return a `StructuredToolResult` that preserves execution metadata,
    /// timing information, and structured context.
    ///
    /// # Parameters
    ///
    /// * `input` - The input data for the tool to process
    ///
    /// # Returns
    ///
    /// A `StructuredToolResult` with rich metadata
    fn call_structured(&self, input: String) -> crate::StructuredToolResult;
}

/// Adapter that implements StructuredTool for any Tool by wrapping results.
///
/// This allows existing tools to automatically gain structured result support
/// without modification. The adapter creates instant metadata since the original
/// tool doesn't track timing.
pub struct StructuredToolAdapter<T: Tool> {
    inner: T,
}

impl<T: Tool> StructuredToolAdapter<T> {
    /// Wrap a Tool to add structured result support.
    pub fn new(tool: T) -> Self {
        Self { inner: tool }
    }

    /// Unwrap to get the inner tool.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Get a reference to the inner tool.
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T: Tool> Tool for StructuredToolAdapter<T> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn call(&self, input: String) -> ExecutionResult {
        self.inner.call(input)
    }
}

impl<T: Tool> StructuredTool for StructuredToolAdapter<T> {
    fn call_structured(&self, input: String) -> crate::StructuredToolResult {
        let result = self.inner.call(input);
        crate::StructuredToolResult::from_execution_result(result, self.inner.name())
    }
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
    fn test_tool_id_validation_for_dispatch() {
        use crate::ToolId;

        // Valid tool IDs (same validation as ToolName had)
        assert!(ToolId::parse("calculator").is_ok());
        assert!(ToolId::parse("tool_name").is_ok());
        assert!(ToolId::parse("tool-name").is_ok());
        assert!(ToolId::parse("Tool123").is_ok());

        // Invalid IDs
        assert!(ToolId::parse("").is_err());
        assert!(ToolId::parse("   ").is_err());
        assert!(ToolId::parse("tool with spaces").is_err());
        assert!(ToolId::parse("tool@special").is_err());

        // Too long (max is 128)
        let long_name = "a".repeat(129);
        assert!(ToolId::parse(&long_name).is_err());

        // Path traversal blocked
        assert!(ToolId::parse("../etc/passwd").is_err());
        assert!(ToolId::parse("./secret").is_err());
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

    #[test]
    fn test_structured_tool_adapter() {
        use crate::StructuredTool;

        let tool = EchoTool;
        let adapter = StructuredToolAdapter::new(tool);

        // Test that Tool trait still works
        let result = adapter.call("test input".to_string());
        assert!(result.is_success());
        assert_eq!(result.output(), "Echo: test input");

        // Test structured call
        let structured = adapter.call_structured("test input".to_string());
        assert!(structured.is_success());
        assert_eq!(structured.success_output(), Some("Echo: test input"));
        assert_eq!(structured.tool_name(), "echo");
        // Metadata is preserved
        assert!(!structured.metadata().tool_name.is_empty());
    }

    #[test]
    fn test_structured_tool_trait() {
        use crate::{StructuredTool, ToolResultBuilder};

        struct TimedTool;

        impl Tool for TimedTool {
            fn name(&self) -> &str {
                "timed"
            }

            fn call(&self, _input: String) -> ExecutionResult {
                ExecutionResult::success("result".to_string())
            }
        }

        impl StructuredTool for TimedTool {
            fn call_structured(&self, input: String) -> crate::StructuredToolResult {
                let start = chrono::Utc::now();
                std::thread::sleep(std::time::Duration::from_millis(1));

                ToolResultBuilder::new(self.name())
                    .started_at(start)
                    .tag("test")
                    .metadata("input_length", input.len().to_string())
                    .success("result")
            }
        }

        let tool = TimedTool;
        let result = tool.call_structured("hello".to_string());

        assert!(result.is_success());
        assert_eq!(result.tool_name(), "timed");
        assert!(result.duration_ms() >= 1);
        assert!(result.metadata().tags.contains(&"test".to_string()));
        assert_eq!(
            result.metadata().custom_metadata.get("input_length"),
            Some(&"5".to_string())
        );
    }

    #[test]
    fn test_tool_input_creation() {
        let input = ToolInput::new_unchecked("test input".to_string());
        assert_eq!(input.as_str(), "test input");
        assert_eq!(input.len(), 10);
        assert!(!input.is_empty());
    }

    #[test]
    fn test_tool_input_conversions() {
        let input = ToolInput::from("test".to_string());
        assert_eq!(input.as_str(), "test");

        let s: String = input.clone().into();
        assert_eq!(s, "test");

        let r: &str = input.as_ref();
        assert_eq!(r, "test");
    }

    #[test]
    fn test_tool_input_display() {
        let input = ToolInput::new_unchecked("hello world".to_string());
        assert_eq!(format!("{}", input), "hello world");
    }

    #[test]
    #[cfg(feature = "security-basic")]
    fn test_tool_input_validation() {
        use crate::security::{
            SecurityPolicy,
            policy::{FileSystemPolicy, HttpPolicy, NetworkPolicy},
        };

        let policy = SecurityPolicy {
            fs_policy: FileSystemPolicy::default(),
            http_policy: HttpPolicy::default(),
            network_policy: NetworkPolicy::default(),
        };

        // Valid input should pass
        let result = ToolInput::validate("normal input".to_string(), &policy);
        assert!(result.is_ok());

        // Input with secrets should fail
        let result = ToolInput::validate("api_key=sk_test123456789abcdef".to_string(), &policy);
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_input_serialization() {
        let input = ToolInput::new_unchecked("test data".to_string());

        // Test JSON serialization
        let json = serde_json::to_string(&input).unwrap();
        assert_eq!(json, "\"test data\"");

        // Test JSON deserialization
        let deserialized: ToolInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, input);
    }
}
