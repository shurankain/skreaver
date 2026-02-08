//! Tool core types and traits.
//!
//! This module re-exports all tool-related types from skreaver-core
//! to maintain API compatibility while consolidating implementation.

use serde::de::DeserializeOwned;

// Re-export all core tool types and traits
pub use skreaver_core::tool::{
    ExecutionResult, Tool, ToolCall, ToolCallBuildError, ToolCallBuilder,
};

// Type aliases for backward compatibility
// ToolName has been deprecated in favor of ToolId which provides the same validation
pub use skreaver_core::ToolId as ToolName;
pub use skreaver_core::ValidationError as InvalidToolName;

/// Trait for tool configuration parsing with fallback behavior.
///
/// Tool configurations typically need to handle two input formats:
/// 1. JSON object with all config fields
/// 2. Simple string for the primary field (fallback)
///
/// This trait provides a unified way to parse configurations from tool input.
///
/// # Example
///
/// ```rust,ignore
/// use skreaver_tools::core::ToolConfig;
///
/// #[derive(Debug, Deserialize)]
/// struct MyConfig {
///     path: String,
///     #[serde(default)]
///     recursive: bool,
/// }
///
/// impl ToolConfig for MyConfig {
///     fn from_simple(input: String) -> Self {
///         Self { path: input, recursive: false }
///     }
/// }
///
/// // Now parse with automatic fallback:
/// let config = MyConfig::parse(input_string);
/// ```
pub trait ToolConfig: Sized + DeserializeOwned {
    /// Create a config from a simple string input.
    ///
    /// This is the fallback when JSON parsing fails, typically
    /// treating the input as the primary field value.
    fn from_simple(input: String) -> Self;

    /// Parse input as JSON config, falling back to simple string.
    ///
    /// This method first attempts to parse the input as a JSON object.
    /// If that fails, it falls back to creating a config from the raw
    /// input string using `from_simple`.
    fn parse(input: String) -> Self {
        serde_json::from_str(&input).unwrap_or_else(|_| Self::from_simple(input))
    }

    /// Parse input as JSON config, returning an error if parsing fails
    /// and the input doesn't look like a simple value.
    ///
    /// This variant is stricter - it only uses the fallback if the input
    /// doesn't look like JSON (doesn't start with `{`).
    fn parse_strict(input: String) -> Result<Self, serde_json::Error> {
        if input.trim_start().starts_with('{') {
            serde_json::from_str(&input)
        } else {
            Ok(Self::from_simple(input))
        }
    }
}
