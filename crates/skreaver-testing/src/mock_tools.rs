//! # Mock Tools for Testing
//!
//! This module provides mock tool implementations that return predictable responses,
//! allowing for reliable and controlled agent testing scenarios.

use skreaver_core::{ExecutionResult, Tool, ToolCall};
use skreaver_tools::{ToolName, ToolRegistry};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A mock tool that returns predefined responses based on input patterns
#[derive(Debug, Clone)]
pub struct MockTool {
    name: String,
    responses: HashMap<String, ExecutionResult>,
    default_response: Option<ExecutionResult>,
    call_count: Arc<Mutex<usize>>,
    call_history: Arc<Mutex<Vec<String>>>,
}

impl MockTool {
    /// Create a new mock tool with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            responses: HashMap::new(),
            default_response: None,
            call_count: Arc::new(Mutex::new(0)),
            call_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a response for a specific input
    pub fn with_response(mut self, input: impl Into<String>, response: impl Into<String>) -> Self {
        self.responses
            .insert(input.into(), ExecutionResult::success(response.into()));
        self
    }

    /// Add a failure response for a specific input
    pub fn with_failure(mut self, input: impl Into<String>, error: impl Into<String>) -> Self {
        self.responses
            .insert(input.into(), ExecutionResult::failure(error.into()));
        self
    }

    /// Set a default response for any unmatched input
    pub fn with_default_response(mut self, response: impl Into<String>) -> Self {
        self.default_response = Some(ExecutionResult::success(response.into()));
        self
    }

    /// Set a default failure response for any unmatched input
    pub fn with_default_failure(mut self, error: impl Into<String>) -> Self {
        self.default_response = Some(ExecutionResult::failure(error.into()));
        self
    }

    /// Get the number of times this tool has been called
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }

    /// Get the history of inputs passed to this tool
    pub fn call_history(&self) -> Vec<String> {
        self.call_history.lock().unwrap().clone()
    }

    /// Reset call count and history
    pub fn reset(&self) {
        *self.call_count.lock().unwrap() = 0;
        self.call_history.lock().unwrap().clear();
    }

    /// Check if the tool was called with a specific input
    pub fn was_called_with(&self, input: &str) -> bool {
        self.call_history
            .lock()
            .unwrap()
            .contains(&input.to_string())
    }
}

impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn call(&self, input: String) -> ExecutionResult {
        // Update call tracking
        *self.call_count.lock().unwrap() += 1;
        self.call_history.lock().unwrap().push(input.clone());

        // Return response based on input
        if let Some(response) = self.responses.get(&input) {
            response.clone()
        } else if let Some(default) = &self.default_response {
            default.clone()
        } else {
            ExecutionResult::success(format!("Mock response for: {}", input))
        }
    }
}

/// A registry of mock tools for testing scenarios
#[derive(Clone)]
pub struct MockToolRegistry {
    tools: HashMap<ToolName, Arc<MockTool>>,
}

impl MockToolRegistry {
    /// Create a new empty mock tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Add a mock tool to the registry
    pub fn with_tool(mut self, tool: MockTool) -> Self {
        let tool_name = ToolName::new(&tool.name).expect("Valid tool name");
        self.tools.insert(tool_name, Arc::new(tool));
        self
    }

    /// Create a simple echo mock tool
    pub fn with_echo_tool(self) -> Self {
        let echo_tool = MockTool::new("echo").with_default_response("echo response");
        self.with_tool(echo_tool)
    }

    /// Create a mock tool that always succeeds
    pub fn with_success_tool(self, name: impl Into<String>) -> Self {
        let success_tool = MockTool::new(name).with_default_response("success");
        self.with_tool(success_tool)
    }

    /// Create a mock tool that always fails
    pub fn with_failure_tool(self, name: impl Into<String>) -> Self {
        let failure_tool = MockTool::new(name).with_default_failure("mock failure");
        self.with_tool(failure_tool)
    }

    /// Add standard mock tools for testing
    pub fn with_mock_tools(self) -> Self {
        self.with_echo_tool()
            .with_success_tool("test_tool")
            .with_failure_tool("fail_tool")
    }

    /// Get a reference to a mock tool for inspection
    pub fn get_mock_tool(&self, name: &str) -> Option<Arc<MockTool>> {
        let tool_name = ToolName::new(name).ok()?;
        self.tools.get(&tool_name).cloned()
    }

    /// Reset all mock tools' call tracking
    pub fn reset_all(&self) {
        for tool in self.tools.values() {
            tool.reset();
        }
    }
}

impl Default for MockToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry for MockToolRegistry {
    fn dispatch(&self, call: ToolCall) -> Option<ExecutionResult> {
        self.tools.get(&call.name).map(|tool| tool.call(call.input))
    }
}

/// Builder for creating complex mock tool scenarios
pub struct MockToolBuilder {
    name: String,
    patterns: Vec<(String, ExecutionResult)>,
    default: Option<ExecutionResult>,
}

impl MockToolBuilder {
    /// Create a new mock tool builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            patterns: Vec::new(),
            default: None,
        }
    }

    /// Add a pattern-based response
    pub fn when(self, input_pattern: impl Into<String>) -> MockResponseBuilder {
        MockResponseBuilder {
            builder: self,
            pattern: input_pattern.into(),
        }
    }

    /// Set the default response
    pub fn default_success(mut self, response: impl Into<String>) -> Self {
        self.default = Some(ExecutionResult::success(response.into()));
        self
    }

    /// Set the default failure
    pub fn default_failure(mut self, error: impl Into<String>) -> Self {
        self.default = Some(ExecutionResult::failure(error.into()));
        self
    }

    /// Build the mock tool
    pub fn build(self) -> MockTool {
        let mut tool = MockTool::new(self.name);

        for (pattern, result) in self.patterns {
            tool.responses.insert(pattern, result);
        }

        tool.default_response = self.default;
        tool
    }
}

/// Builder for mock tool responses
pub struct MockResponseBuilder {
    builder: MockToolBuilder,
    pattern: String,
}

impl MockResponseBuilder {
    /// Set a success response
    pub fn respond_with(mut self, response: impl Into<String>) -> MockToolBuilder {
        let result = ExecutionResult::success(response.into());
        self.builder.patterns.push((self.pattern, result));
        self.builder
    }

    /// Set a failure response
    pub fn fail_with(mut self, error: impl Into<String>) -> MockToolBuilder {
        let result = ExecutionResult::failure(error.into());
        self.builder.patterns.push((self.pattern, result));
        self.builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_tool_returns_configured_response() {
        let tool = MockTool::new("test")
            .with_response("input1", "response1")
            .with_response("input2", "response2");

        let result1 = tool.call("input1".to_string());
        let result2 = tool.call("input2".to_string());

        assert!(result1.is_success());
        assert_eq!(result1.output(), "response1");
        assert!(result2.is_success());
        assert_eq!(result2.output(), "response2");
    }

    #[test]
    fn mock_tool_tracks_calls() {
        let tool = MockTool::new("test").with_default_response("default");

        tool.call("input1".to_string());
        tool.call("input2".to_string());

        assert_eq!(tool.call_count(), 2);
        assert!(tool.was_called_with("input1"));
        assert!(tool.was_called_with("input2"));
    }

    #[test]
    fn mock_tool_registry_dispatches_correctly() {
        let tool = MockTool::new("test_tool").with_response("hello", "world");

        let registry = MockToolRegistry::new().with_tool(tool);

        let call = ToolCall {
            name: ToolName::new("test_tool").unwrap(),
            input: "hello".to_string(),
        };

        let result = registry.dispatch(call).unwrap();
        assert!(result.is_success());
        assert_eq!(result.output(), "world");
    }

    #[test]
    fn mock_tool_builder_works() {
        let tool = MockToolBuilder::new("complex_tool")
            .when("pattern1")
            .respond_with("response1")
            .when("pattern2")
            .fail_with("error2")
            .default_success("default response")
            .build();

        let result1 = tool.call("pattern1".to_string());
        let result2 = tool.call("pattern2".to_string());
        let result3 = tool.call("unknown".to_string());

        assert!(result1.is_success());
        assert_eq!(result1.output(), "response1");

        assert!(!result2.is_success());
        assert_eq!(result2.output(), "error2");

        assert!(result3.is_success());
        assert_eq!(result3.output(), "default response");
    }
}
