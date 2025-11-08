//! Critical Path Tests for Tool Operations
//!
//! These tests focus on tool execution, validation, and error handling
//! that are critical for production workloads.

use skreaver_core::{ExecutionResult, Tool, ToolCall};
use skreaver_testing::mock_tools::MockTool;
use skreaver_tools::ToolName;

/// Test basic tool execution with success response
#[test]
fn test_tool_execution_success() {
    let mock_tool = MockTool::new("test_tool").with_response("test_input", "success_response");

    let result = mock_tool.call("test_input".to_string());
    assert!(result.is_success());
    assert_eq!(result.output(), "success_response");
}

/// Test tool execution with failure response
#[test]
fn test_tool_execution_failure() {
    let mock_tool = MockTool::new("error_tool").with_failure("error_input", "Simulated error");

    let result = mock_tool.call("error_input".to_string());
    assert!(result.is_failure());
    assert!(result.output().contains("Simulated error"));
}

/// Test tool with default response for unmatched inputs
#[test]
fn test_tool_default_response() {
    let mock_tool = MockTool::new("default_tool")
        .with_response("specific_input", "specific_response")
        .with_default_response("default_response");

    // Test specific input
    let specific_result = mock_tool.call("specific_input".to_string());
    assert!(specific_result.is_success());
    assert_eq!(specific_result.output(), "specific_response");

    // Test unmatched input (should use default)
    let default_result = mock_tool.call("unknown_input".to_string());
    assert!(default_result.is_success());
    assert_eq!(default_result.output(), "default_response");
}

/// Test tool name validation
#[test]
fn test_tool_name_validation() {
    // Valid tool names should work
    assert!(ToolName::parse("valid_tool").is_ok());
    assert!(ToolName::parse("valid-tool-123").is_ok());
    assert!(ToolName::parse("tool_with_underscores").is_ok());

    // Invalid tool names should fail
    assert!(ToolName::parse("").is_err()); // Empty string
    assert!(ToolName::parse("tool with spaces").is_err()); // Contains spaces
    assert!(ToolName::parse("tool@symbol").is_err()); // Invalid characters
}

/// Test tool call creation and properties
#[test]
fn test_tool_call_creation() {
    let input = "test_input";

    let tool_call = ToolCall::new("test_tool", input).expect("Valid tool call");

    assert_eq!(tool_call.name(), "test_tool");
    assert_eq!(tool_call.input, input);
}

/// Test execution result creation and properties
#[test]
fn test_execution_result_success() {
    let result = ExecutionResult::success("success_output".to_string());

    assert!(result.is_success());
    assert!(!result.is_failure());
    assert_eq!(result.output(), "success_output");
}

/// Test execution result failure
#[test]
fn test_execution_result_failure() {
    let result = ExecutionResult::failure("error_message".to_string());

    assert!(!result.is_success());
    assert!(result.is_failure());
    assert_eq!(result.output(), "error_message");
}

/// Test mock tool call tracking
#[test]
fn test_mock_tool_call_tracking() {
    let mock_tool = MockTool::new("tracking_tool")
        .with_response("input1", "output1")
        .with_response("input2", "output2");

    // Make multiple calls
    mock_tool.call("input1".to_string());
    mock_tool.call("input2".to_string());

    // Verify call tracking
    assert_eq!(mock_tool.call_count(), 2);
    let history = mock_tool.call_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0], "input1");
    assert_eq!(history[1], "input2");
}

/// Test tool with empty input
#[test]
fn test_tool_empty_input() {
    let mock_tool = MockTool::new("empty_input_tool").with_response("", "handled_empty_input");

    let result = mock_tool.call("".to_string());
    assert!(result.is_success());
    assert_eq!(result.output(), "handled_empty_input");
}

/// Test tool with large input
#[test]
fn test_tool_large_input() {
    let large_input = "x".repeat(10000); // 10KB input
    let mock_tool =
        MockTool::new("large_input_tool").with_response(&large_input, "handled_large_input");

    let result = mock_tool.call(large_input);
    assert!(result.is_success());
    assert_eq!(result.output(), "handled_large_input");
}

/// Test tool with special characters in input
#[test]
fn test_tool_special_characters() {
    let special_input = "Test with special chars: 🦀 \n\t\r中文 العربية";
    let mock_tool =
        MockTool::new("special_char_tool").with_response(special_input, "handled_special_chars");

    let result = mock_tool.call(special_input.to_string());
    assert!(result.is_success());
    assert_eq!(result.output(), "handled_special_chars");
}

/// Test multiple tools with same name (should be independent)
#[test]
fn test_multiple_tools_independence() {
    let tool1 = MockTool::new("shared_name").with_response("input", "output1");
    let tool2 = MockTool::new("shared_name").with_response("input", "output2");

    let result1 = tool1.call("input".to_string());
    let result2 = tool2.call("input".to_string());

    // Each tool should produce its own configured output
    assert_eq!(result1.output(), "output1");
    assert_eq!(result2.output(), "output2");
}

/// Test tool performance characteristics
#[test]
fn test_tool_performance() {
    use std::time::Instant;

    let mock_tool = MockTool::new("perf_tool").with_default_response("fast_response");

    let start = Instant::now();

    // Make 1000 tool calls
    for i in 0..1000 {
        mock_tool.call(format!("input_{}", i));
    }

    let duration = start.elapsed();

    // Should complete 1000 calls in reasonable time (< 50ms on modern hardware)
    assert!(
        duration.as_millis() < 50,
        "Tool calls too slow: {:?}",
        duration
    );
}
