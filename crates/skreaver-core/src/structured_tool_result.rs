//! Structured tool execution results with rich metadata preservation.
//!
//! This module provides type-safe tool result representations that preserve
//! execution context, timing information, and metadata throughout the agent
//! execution pipeline. By embedding this information directly in the type
//! system, we prevent accidental loss of important diagnostic data.

use chrono::{DateTime, Utc};
use std::time::Duration;

/// Rich metadata attached to tool execution results.
///
/// This type ensures important diagnostic information is preserved
/// throughout the agent execution lifecycle.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolExecutionMetadata {
    /// The name of the tool that was executed.
    pub tool_name: String,

    /// When the tool execution started.
    pub started_at: DateTime<Utc>,

    /// When the tool execution completed.
    pub completed_at: DateTime<Utc>,

    /// How long the tool took to execute.
    pub duration: Duration,

    /// Optional tags for categorization and filtering.
    pub tags: Vec<String>,

    /// Optional custom metadata as key-value pairs.
    pub custom_metadata: std::collections::HashMap<String, String>,
}

impl ToolExecutionMetadata {
    /// Create new metadata for a tool execution.
    ///
    /// # Parameters
    ///
    /// * `tool_name` - The name of the tool being executed
    /// * `started_at` - When execution began
    /// * `completed_at` - When execution finished
    ///
    /// # Returns
    ///
    /// A new `ToolExecutionMetadata` instance with calculated duration
    pub fn new(
        tool_name: impl Into<String>,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
    ) -> Self {
        let duration = completed_at
            .signed_duration_since(started_at)
            .to_std()
            .unwrap_or(Duration::ZERO);

        Self {
            tool_name: tool_name.into(),
            started_at,
            completed_at,
            duration,
            tags: Vec::new(),
            custom_metadata: std::collections::HashMap::new(),
        }
    }

    /// Create metadata with the current time as both start and end.
    ///
    /// Useful for tools that complete instantly or when precise timing isn't needed.
    pub fn instant(tool_name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self::new(tool_name, now, now)
    }

    /// Add a tag to this metadata.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add custom metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_metadata.insert(key.into(), value.into());
        self
    }

    /// Get the duration as milliseconds for easy display.
    pub fn duration_ms(&self) -> u64 {
        self.duration.as_millis() as u64
    }
}

/// Structured result from tool execution with preserved context.
///
/// Unlike the simple `ExecutionResult` which only contains output/error strings,
/// `StructuredToolResult` preserves execution metadata, timing information,
/// and structured context. This prevents the common anti-pattern of losing
/// diagnostic information when passing results through the system.
///
/// # Type Safety
///
/// This type uses tagged unions to make invalid states unrepresentable:
/// - Success results always have output and cannot have errors
/// - Failure results always have errors and cannot have output
/// - Both variants preserve execution metadata
#[derive(Debug, Clone, PartialEq)]
pub enum StructuredToolResult {
    /// Tool executed successfully.
    Success {
        /// The successful output from the tool.
        output: String,

        /// Execution metadata including timing and tool name.
        metadata: ToolExecutionMetadata,
    },

    /// Tool execution failed.
    Failure {
        /// The error message describing what went wrong.
        error: String,

        /// Execution metadata including timing and tool name.
        metadata: ToolExecutionMetadata,

        /// Optional error code for programmatic error handling.
        error_code: Option<String>,

        /// Whether the error is recoverable (agent can retry).
        recoverable: bool,
    },
}

impl StructuredToolResult {
    /// Create a successful result with metadata.
    ///
    /// # Parameters
    ///
    /// * `output` - The successful output
    /// * `metadata` - Execution metadata
    pub fn success(output: impl Into<String>, metadata: ToolExecutionMetadata) -> Self {
        Self::Success {
            output: output.into(),
            metadata,
        }
    }

    /// Create a failed result with metadata.
    ///
    /// # Parameters
    ///
    /// * `error` - Error message
    /// * `metadata` - Execution metadata
    /// * `recoverable` - Whether the agent can retry
    pub fn failure(
        error: impl Into<String>,
        metadata: ToolExecutionMetadata,
        recoverable: bool,
    ) -> Self {
        Self::Failure {
            error: error.into(),
            metadata,
            error_code: None,
            recoverable,
        }
    }

    /// Create a failed result with an error code.
    pub fn failure_with_code(
        error: impl Into<String>,
        metadata: ToolExecutionMetadata,
        error_code: impl Into<String>,
        recoverable: bool,
    ) -> Self {
        Self::Failure {
            error: error.into(),
            metadata,
            error_code: Some(error_code.into()),
            recoverable,
        }
    }

    /// Check if this result represents success.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Check if this result represents failure.
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure { .. })
    }

    /// Get the metadata regardless of success/failure.
    pub fn metadata(&self) -> &ToolExecutionMetadata {
        match self {
            Self::Success { metadata, .. } => metadata,
            Self::Failure { metadata, .. } => metadata,
        }
    }

    /// Get the tool name from metadata.
    pub fn tool_name(&self) -> &str {
        &self.metadata().tool_name
    }

    /// Get the execution duration.
    pub fn duration(&self) -> Duration {
        self.metadata().duration
    }

    /// Get the execution duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.metadata().duration_ms()
    }

    /// Get the output or error message.
    pub fn output_or_error(&self) -> &str {
        match self {
            Self::Success { output, .. } => output,
            Self::Failure { error, .. } => error,
        }
    }

    /// Get the success output if available.
    pub fn success_output(&self) -> Option<&str> {
        match self {
            Self::Success { output, .. } => Some(output),
            Self::Failure { .. } => None,
        }
    }

    /// Get the error message if available.
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Success { .. } => None,
            Self::Failure { error, .. } => Some(error),
        }
    }

    /// Get the error code if this is a failure with a code.
    pub fn error_code(&self) -> Option<&str> {
        match self {
            Self::Success { .. } => None,
            Self::Failure { error_code, .. } => error_code.as_deref(),
        }
    }

    /// Check if this failure is recoverable.
    ///
    /// Returns `None` if this is a success (no error to recover from).
    pub fn is_recoverable(&self) -> Option<bool> {
        match self {
            Self::Success { .. } => None,
            Self::Failure { recoverable, .. } => Some(*recoverable),
        }
    }

    /// Convert to a Result type, losing metadata.
    ///
    /// Use this only when you need to interop with code that expects
    /// simple Result types. Prefer keeping the structured result when possible.
    pub fn into_result(self) -> Result<String, String> {
        match self {
            Self::Success { output, .. } => Ok(output),
            Self::Failure { error, .. } => Err(error),
        }
    }

    /// Convert to the simple ExecutionResult, losing metadata.
    ///
    /// Use this for backwards compatibility with code expecting ExecutionResult.
    /// Prefer StructuredToolResult in new code to preserve metadata.
    pub fn into_execution_result(self) -> super::tool::ExecutionResult {
        match self {
            Self::Success { output, .. } => super::tool::ExecutionResult::success(output),
            Self::Failure { error, .. } => super::tool::ExecutionResult::failure(error),
        }
    }

    /// Create from a simple ExecutionResult with minimal metadata.
    ///
    /// Since ExecutionResult doesn't have metadata, this creates
    /// instant metadata with the current time.
    pub fn from_execution_result(
        result: super::tool::ExecutionResult,
        tool_name: impl Into<String>,
    ) -> Self {
        let metadata = ToolExecutionMetadata::instant(tool_name);
        match result {
            super::tool::ExecutionResult::Success { output } => Self::Success { output, metadata },
            super::tool::ExecutionResult::Failure { reason } => Self::Failure {
                error: reason.message(),
                metadata,
                error_code: None,
                recoverable: true, // Default to recoverable when we don't have info
            },
        }
    }
}

/// Builder for creating StructuredToolResult with fluent API.
///
/// This builder ensures all required fields are provided while making
/// optional fields easy to add.
pub struct ToolResultBuilder {
    tool_name: String,
    started_at: Option<DateTime<Utc>>,
    tags: Vec<String>,
    custom_metadata: std::collections::HashMap<String, String>,
}

impl ToolResultBuilder {
    /// Create a new builder for the given tool name.
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            started_at: None,
            tags: Vec::new(),
            custom_metadata: std::collections::HashMap::new(),
        }
    }

    /// Set the start time (defaults to current time if not set).
    pub fn started_at(mut self, time: DateTime<Utc>) -> Self {
        self.started_at = Some(time);
        self
    }

    /// Add a tag.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add custom metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_metadata.insert(key.into(), value.into());
        self
    }

    /// Build a successful result.
    pub fn success(self, output: impl Into<String>) -> StructuredToolResult {
        let completed_at = Utc::now();
        let started_at = self.started_at.unwrap_or(completed_at);

        let mut metadata = ToolExecutionMetadata::new(self.tool_name, started_at, completed_at);
        metadata.tags = self.tags;
        metadata.custom_metadata = self.custom_metadata;

        StructuredToolResult::Success {
            output: output.into(),
            metadata,
        }
    }

    /// Build a failed result.
    pub fn failure(self, error: impl Into<String>, recoverable: bool) -> StructuredToolResult {
        let completed_at = Utc::now();
        let started_at = self.started_at.unwrap_or(completed_at);

        let mut metadata = ToolExecutionMetadata::new(self.tool_name, started_at, completed_at);
        metadata.tags = self.tags;
        metadata.custom_metadata = self.custom_metadata;

        StructuredToolResult::Failure {
            error: error.into(),
            metadata,
            error_code: None,
            recoverable,
        }
    }

    /// Build a failed result with an error code.
    pub fn failure_with_code(
        self,
        error: impl Into<String>,
        error_code: impl Into<String>,
        recoverable: bool,
    ) -> StructuredToolResult {
        let completed_at = Utc::now();
        let started_at = self.started_at.unwrap_or(completed_at);

        let mut metadata = ToolExecutionMetadata::new(self.tool_name, started_at, completed_at);
        metadata.tags = self.tags;
        metadata.custom_metadata = self.custom_metadata;

        StructuredToolResult::Failure {
            error: error.into(),
            metadata,
            error_code: Some(error_code.into()),
            recoverable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_creation() {
        let start = Utc::now();
        let end = start + chrono::Duration::milliseconds(100);

        let metadata = ToolExecutionMetadata::new("test_tool", start, end);

        assert_eq!(metadata.tool_name, "test_tool");
        assert_eq!(metadata.started_at, start);
        assert_eq!(metadata.completed_at, end);
        assert!(metadata.duration.as_millis() >= 100);
        assert!(metadata.tags.is_empty());
        assert!(metadata.custom_metadata.is_empty());
    }

    #[test]
    fn test_metadata_with_tags() {
        let metadata = ToolExecutionMetadata::instant("test")
            .with_tag("http")
            .with_tag("external");

        assert_eq!(metadata.tags, vec!["http", "external"]);
    }

    #[test]
    fn test_metadata_with_custom() {
        let metadata = ToolExecutionMetadata::instant("test")
            .with_metadata("request_id", "abc123")
            .with_metadata("user_id", "user_42");

        assert_eq!(
            metadata.custom_metadata.get("request_id"),
            Some(&"abc123".to_string())
        );
        assert_eq!(
            metadata.custom_metadata.get("user_id"),
            Some(&"user_42".to_string())
        );
    }

    #[test]
    fn test_success_result() {
        let metadata = ToolExecutionMetadata::instant("calculator");
        let result = StructuredToolResult::success("42", metadata.clone());

        assert!(result.is_success());
        assert!(!result.is_failure());
        assert_eq!(result.success_output(), Some("42"));
        assert_eq!(result.error_message(), None);
        assert_eq!(result.tool_name(), "calculator");
        assert_eq!(result.metadata(), &metadata);
    }

    #[test]
    fn test_failure_result() {
        let metadata = ToolExecutionMetadata::instant("http_get");
        let result = StructuredToolResult::failure("Connection timeout", metadata.clone(), true);

        assert!(!result.is_success());
        assert!(result.is_failure());
        assert_eq!(result.success_output(), None);
        assert_eq!(result.error_message(), Some("Connection timeout"));
        assert_eq!(result.is_recoverable(), Some(true));
        assert_eq!(result.error_code(), None);
    }

    #[test]
    fn test_failure_with_error_code() {
        let metadata = ToolExecutionMetadata::instant("database_query");
        let result = StructuredToolResult::failure_with_code(
            "Unique constraint violation",
            metadata,
            "E_DUPLICATE_KEY",
            false,
        );

        assert!(result.is_failure());
        assert_eq!(result.error_code(), Some("E_DUPLICATE_KEY"));
        assert_eq!(result.is_recoverable(), Some(false));
    }

    #[test]
    fn test_builder_success() {
        let result = ToolResultBuilder::new("test_tool")
            .tag("integration")
            .metadata("version", "1.0")
            .success("output data");

        assert!(result.is_success());
        assert_eq!(result.success_output(), Some("output data"));
        assert!(result.metadata().tags.contains(&"integration".to_string()));
        assert_eq!(
            result.metadata().custom_metadata.get("version"),
            Some(&"1.0".to_string())
        );
    }

    #[test]
    fn test_builder_failure() {
        let result = ToolResultBuilder::new("test_tool")
            .tag("external")
            .failure("Network error", true);

        assert!(result.is_failure());
        assert_eq!(result.error_message(), Some("Network error"));
        assert_eq!(result.is_recoverable(), Some(true));
    }

    #[test]
    fn test_conversion_to_execution_result() {
        let metadata = ToolExecutionMetadata::instant("test");
        let structured = StructuredToolResult::success("output", metadata);
        let simple = structured.into_execution_result();

        assert!(simple.is_success());
        assert_eq!(simple.output(), "output");
    }

    #[test]
    fn test_conversion_from_execution_result() {
        use crate::tool::ExecutionResult;

        let simple = ExecutionResult::success("result".to_string());
        let structured = StructuredToolResult::from_execution_result(simple, "test_tool");

        assert!(structured.is_success());
        assert_eq!(structured.success_output(), Some("result"));
        assert_eq!(structured.tool_name(), "test_tool");
    }
}
