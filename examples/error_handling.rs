//! Example demonstrating the new structured error handling in Skreaver.

use skreaver::{
    error::{SkreverError, ToolError, MemoryError, SkreverResult},
    tool::{Tool, ExecutionResult, ToolCall},
    tool::registry::{InMemoryToolRegistry, ToolRegistry},
};
use std::sync::Arc;

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
    let result = registry.dispatch(ToolCall {
        name: "example".to_string(),
        input: "test input".to_string(),
    });
    
    match result {
        Some(exec_result) => {
            match exec_result.into_result() {
                Ok(output) => println!("Success: {}", output),
                Err(error) => println!("Tool error: {}", error),
            }
        }
        None => println!("Tool not found"),
    }

    // Example 2: Tool not found (using new structured error handling)
    println!("\n❌ Example 2: Tool not found");
    let result = registry.try_dispatch(ToolCall {
        name: "nonexistent".to_string(),
        input: "test".to_string(),
    });
    
    match result {
        Ok(exec_result) => println!("Success: {}", exec_result.output),
        Err(ToolError::NotFound { name }) => {
            println!("Tool '{}' not found in registry", name);
        }
        Err(other_error) => println!("Other tool error: {}", other_error),
    }

    // Example 3: Tool execution failure
    println!("\n❌ Example 3: Tool execution failure");
    let result = registry.dispatch(ToolCall {
        name: "failing".to_string(),
        input: "test input".to_string(),
    });
    
    if let Some(exec_result) = result {
        match exec_result.into_result() {
            Ok(output) => println!("Success: {}", output),
            Err(error) => println!("Tool execution failed: {}", error),
        }
    }

    // Example 4: Memory error handling
    println!("\n❌ Example 4: Memory error example");
    let memory_error = MemoryError::StoreFailed {
        key: "test_key".to_string(),
        reason: "Disk full".to_string(),
    };
    
    let skrever_error: SkreverError = memory_error.into();
    println!("Memory error: {}", skrever_error);

    // Example 5: Tool error handling with structured information
    println!("\n❌ Example 5: Structured tool error");
    let tool_error = ToolError::InvalidInput {
        name: "calculator".to_string(),
        input: "invalid_number".to_string(),
        reason: "Not a valid number".to_string(),
    };
    
    println!("Tool error: {}", tool_error);
    
    Ok(())
}