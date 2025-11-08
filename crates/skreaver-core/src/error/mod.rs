//! Error Types
//!
//! This module defines custom error types for domain-specific failures
//! throughout the Skreaver framework. These errors provide structured
//! information about what went wrong and enable better error handling
//! and debugging.
//!
//! The error types are organized into focused submodules:
//! - `types`: Core types and validation structures
//! - `tool`: Tool execution and dispatch errors
//! - `memory`: Memory backend and transaction errors
//! - `agent`: Agent and coordinator errors
//! - `conversions`: Error type conversions

// Submodules
mod agent;
mod conversions;
mod memory;
mod tool;
mod types;

// Re-export everything for backward compatibility
pub use agent::{AgentError, AgentResult, CoordinatorError, CoordinatorResult};
pub use conversions::{SkreverError, SkreverResult};
pub use memory::{MemoryError, MemoryResult, TransactionError, TransactionResult};
pub use tool::{ToolError, ToolResult};
pub use types::{
    InputValidationError, MemoryBackend, MemoryErrorKind, MemoryOperation, ValidatedInput,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::StandardTool;

    #[test]
    fn test_tool_error_not_found_with_standard_tool() {
        let tool = crate::tool::ToolDispatch::Standard(StandardTool::HttpGet);
        let error = ToolError::not_found(tool);

        assert_eq!(error.tool_name(), Some("http_get"));
        assert!(error.to_string().contains("http_get"));
        assert!(error.to_string().contains("not found"));
    }

    #[test]
    fn test_tool_error_not_found_by_name() {
        let error = ToolError::not_found_by_name("file_read");
        assert_eq!(error.tool_name(), Some("file_read"));

        // Test invalid name
        let error = ToolError::not_found_by_name("invalid tool name!");
        assert!(matches!(error, ToolError::InvalidToolId { .. }));
        assert_eq!(error.tool_name(), None);
    }

    #[test]
    fn test_tool_error_execution_failed() {
        let tool = crate::tool::ToolDispatch::Standard(StandardTool::JsonParse);
        let error = ToolError::execution_failed(tool, "Invalid JSON format".to_string());

        assert_eq!(error.tool_name(), Some("json_parse"));
        assert!(error.to_string().contains("json_parse"));
        assert!(error.to_string().contains("execution failed"));
        assert!(error.to_string().contains("Invalid JSON format"));
    }

    #[test]
    fn test_tool_error_invalid_input() {
        let tool = crate::tool::ToolDispatch::Standard(StandardTool::HttpPost);
        let error = ToolError::invalid_input(
            tool,
            "test input".to_string(),
            "Missing required field".to_string(),
        );

        assert_eq!(error.tool_name(), Some("http_post"));
        assert!(error.to_string().contains("invalid input"));
        assert!(error.to_string().contains("test input"));
        assert!(error.to_string().contains("Missing required field"));
    }

    #[test]
    fn test_tool_error_timeout() {
        let tool = crate::tool::ToolDispatch::Custom(
            crate::ToolId::parse("custom_tool").expect("Valid tool ID"),
        );
        let error = ToolError::timeout(tool, 5000);

        assert_eq!(error.tool_name(), Some("custom_tool"));
        assert!(error.to_string().contains("custom_tool"));
        assert!(error.to_string().contains("timed out"));
        assert!(error.to_string().contains("5000ms"));
    }

    #[test]
    fn test_validated_input() {
        // Valid input
        let input = ValidatedInput::new("Hello, world!".to_string()).unwrap();
        assert_eq!(input.as_str(), "Hello, world!");
        assert_eq!(input.len(), 13);
        assert!(!input.is_empty());

        // Empty input
        assert!(matches!(
            ValidatedInput::new("".to_string()),
            Err(InputValidationError::Empty)
        ));

        // Too large input
        let large_input = "x".repeat(ValidatedInput::MAX_SIZE + 1);
        assert!(matches!(
            ValidatedInput::new(large_input),
            Err(InputValidationError::TooLarge { .. })
        ));

        // Binary content (lots of null bytes and control characters)
        let binary_input = (0..20u8)
            .cycle()
            .take(100)
            .map(|b| b as char)
            .collect::<String>();
        assert!(matches!(
            ValidatedInput::new(binary_input),
            Err(InputValidationError::BinaryContent)
        ));
    }

    #[test]
    fn test_validated_input_display_truncation() {
        let short_input = ValidatedInput::new_unchecked("short".to_string());
        assert_eq!(short_input.to_string(), "short");

        let long_input = ValidatedInput::new_unchecked("x".repeat(200));
        let display = long_input.to_string();
        assert!(display.len() <= 100);
        assert!(display.ends_with("..."));
    }

    #[test]
    fn test_tool_error_conversions() {
        // Test IdValidationError conversion for tool IDs
        let invalid_id = crate::IdValidationError::Empty;
        let tool_error: ToolError = invalid_id.into();
        assert!(matches!(tool_error, ToolError::InvalidToolId { .. }));

        // Test InputValidationError conversion
        let input_error = InputValidationError::Empty;
        let tool_error: ToolError = input_error.into();
        assert!(matches!(tool_error, ToolError::InvalidInput { .. }));
    }

    #[test]
    fn test_error_hierarchy() {
        let tool_error = ToolError::not_found_by_name("missing_tool");
        let skrever_error: SkreverError = tool_error.into();

        assert!(matches!(skrever_error, SkreverError::Tool(_)));
        assert!(skrever_error.to_string().contains("Tool error"));
    }
}
