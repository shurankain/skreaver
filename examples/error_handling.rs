//! Example demonstrating the new structured error handling in Skreaver.

use skreaver::{
    ExecutionResult, InMemoryToolRegistry, MemoryKey, Tool, ToolCall, ToolRegistry,
};
use skreaver_core::error::{MemoryError, ToolError, ValidatedInput, MemoryBackend, MemoryErrorKind};
use skreaver_core::tool::{ToolDispatch, ToolName};
use std::sync::Arc;

// Define result type for this example
type SkreverResult<T> = Result<T, Box<dyn std::error::Error>>;
type SkreverError = Box<dyn std::error::Error>;

/// Example tool that can fail with different error types
struct ExampleTool {
    should_fail: bool,
}

impl Tool for ExampleTool {
    fn name(&self) -> &str {
        "example"
    }

    fn call(&self, input: String) -> ExecutionResult {
        if self.should_fail {
            // Example of tool failure
            ExecutionResult::failure(format!("Tool failed to process: {}", input))
        } else if input.is_empty() {
            // Example of invalid input
            ExecutionResult::failure("Input cannot be empty".to_string())
        } else {
            // Success case
            ExecutionResult::success(format!("Processed: {}", input))
        }
    }
}

fn main() -> SkreverResult<()> {
    println!("🔧 Skreaver Error Handling Example");

    // Create a tool registry
    let registry = InMemoryToolRegistry::new()
        .with_tool("example", Arc::new(ExampleTool { should_fail: false }))
        .with_tool("failing", Arc::new(ExampleTool { should_fail: true }));

    // Example 1: Successful tool call
    println!("\n✅ Example 1: Successful tool call");
    let result =
        registry.dispatch(ToolCall::new("example", "test input").expect("Valid tool name"));

    match result {
        Some(exec_result) => match exec_result.into_result() {
            Ok(output) => println!("Success: {}", output),
            Err(error) => println!("Tool error: {}", error),
        },
        None => println!("Tool not found"),
    }

    // Example 2: Tool not found (using new structured error handling)
    println!("\n❌ Example 2: Tool not found");
    let tool_call = ToolCall::new("nonexistent", "test").expect("Valid tool name");
    let result = registry.try_dispatch(&tool_call);

    match result {
        Ok(exec_result) => println!("Success: {}", exec_result.output()),
        Err(error_msg) => {
            println!("Tool error: {}", error_msg);
        }
    }

    // Example 3: Tool execution failure
    println!("\n❌ Example 3: Tool execution failure");
    let result =
        registry.dispatch(ToolCall::new("failing", "test input").expect("Valid tool name"));

    if let Some(exec_result) = result {
        match exec_result.into_result() {
            Ok(output) => println!("Success: {}", output),
            Err(error) => println!("Tool execution failed: {}", error),
        }
    }

    // Example 4: Memory error handling
    println!("\n❌ Example 4: Memory error example");
    let memory_error = MemoryError::StoreFailed {
        key: MemoryKey::new("test_key").unwrap(),
        backend: MemoryBackend::File,
        kind: MemoryErrorKind::ResourceExhausted {
            resource: "disk space".to_string(),
            limit: "100GB".to_string(),
        },
    };

    let skrever_error: SkreverError = Box::new(memory_error);
    println!("Memory error: {}", skrever_error);

    // Example 5: Tool error handling with structured information
    println!("\n❌ Example 5: Structured tool error");
    let tool_name = ToolName::new("calculator").expect("Valid tool name");
    let tool_dispatch = ToolDispatch::Custom(tool_name);
    let validated_input = ValidatedInput::new("invalid_number".to_string()).expect("Valid input");
    let tool_error = ToolError::InvalidInput {
        tool: tool_dispatch,
        input: validated_input,
        reason: "Not a valid number".to_string(),
    };

    println!("Tool error: {}", tool_error);

    Ok(())
}
