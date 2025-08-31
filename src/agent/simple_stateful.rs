use crate::error::MemoryError;
/// A simplified, concrete implementation of the stateful agent pattern.
///
/// This provides a practical example of how to implement compile-time state safety
/// without the complexity of generic associated types. Based on the proven pattern
/// from the CLI reasoning agent.
use crate::{MemoryKey, MemoryReader};
use skreaver_core::{ExecutionResult, ToolCall};

/// Simple stateful agent with compile-time state transitions.
///
/// This agent demonstrates the typestate pattern for state safety.
/// Invalid state transitions are prevented at compile time.
pub struct SimpleStatefulAgent<State> {
    memory: Box<dyn MemoryReader + Send + Sync>,
    state: State,
}

/// Initial state - agent is ready to receive observations
#[derive(Debug)]
pub struct SimpleInitial;

/// Processing state - agent is analyzing input and may need tools
#[derive(Debug)]
pub struct SimpleProcessing {
    pub input: String,
    pub context: Vec<String>,
}

/// Tool execution state - agent has requested tools and awaits results  
#[derive(Debug)]
pub struct SimpleToolExecution {
    pub input: String,
    pub context: Vec<String>,
    pub pending_tools: Vec<ToolCall>,
}

/// Complete state - agent is ready to provide final output
#[derive(Debug)]
pub struct SimpleComplete {
    pub input: String,
    pub context: Vec<String>,
    pub result: String,
}

// Marker trait implementations for type safety
impl crate::agent::stateful::InitialState for SimpleInitial {}
impl crate::agent::stateful::ProcessingState for SimpleProcessing {}
impl crate::agent::stateful::ToolExecutionState for SimpleToolExecution {}
impl crate::agent::stateful::CompleteState for SimpleComplete {}

// Implementation for Initial state
impl SimpleStatefulAgent<SimpleInitial> {
    /// Create a new agent in initial state
    pub fn new(memory: Box<dyn MemoryReader + Send + Sync>) -> Self {
        Self {
            memory,
            state: SimpleInitial,
        }
    }

    /// Process an observation and transition to processing state
    pub fn observe(self, input: String) -> SimpleStatefulAgent<SimpleProcessing> {
        // Load previous context from memory if available
        let context = self.load_context().unwrap_or_default();

        SimpleStatefulAgent {
            memory: self.memory,
            state: SimpleProcessing { input, context },
        }
    }

    fn load_context(&self) -> Result<Vec<String>, MemoryError> {
        // Try to load previous conversation context
        let key = MemoryKey::new("context").map_err(|_| MemoryError::LoadFailed {
            key: "context".to_string(),
            reason: "Invalid key".to_string(),
        })?;

        Ok(if let Some(context_json) = self.memory.load(&key)? {
            serde_json::from_str(&context_json).unwrap_or_else(|_| Vec::new())
        } else {
            Vec::new()
        })
    }
}

// Implementation for Processing state
impl SimpleStatefulAgent<SimpleProcessing> {
    /// Get tool calls needed for this input
    pub fn get_tool_calls(&self) -> Vec<ToolCall> {
        // Simple logic: if input contains "search", request search tool
        if self.state.input.to_lowercase().contains("search") {
            vec![
                ToolCall::new("web_search", &self.state.input)
                    .unwrap_or_else(|_| ToolCall::new("echo", "fallback").expect("Valid fallback")),
            ]
        } else {
            // No tools needed, can go directly to complete
            Vec::new()
        }
    }

    /// Check if this agent needs tool execution
    pub fn needs_tools(&self) -> bool {
        !self.get_tool_calls().is_empty()
    }

    /// Transition to tool execution state (if tools are needed)
    pub fn request_tools(self) -> SimpleStatefulAgent<SimpleToolExecution> {
        let tools = self.get_tool_calls();
        SimpleStatefulAgent {
            memory: self.memory,
            state: SimpleToolExecution {
                input: self.state.input,
                context: self.state.context,
                pending_tools: tools,
            },
        }
    }

    /// Transition directly to complete state (if no tools needed)
    pub fn complete_without_tools(self) -> SimpleStatefulAgent<SimpleComplete> {
        let result = format!("Processed: {}", self.state.input);
        SimpleStatefulAgent {
            memory: self.memory,
            state: SimpleComplete {
                input: self.state.input,
                context: self.state.context,
                result,
            },
        }
    }
}

// Implementation for ToolExecution state
impl SimpleStatefulAgent<SimpleToolExecution> {
    /// Get the pending tool calls
    pub fn pending_tools(&self) -> &[ToolCall] {
        &self.state.pending_tools
    }

    /// Handle tool execution results and transition to next state
    pub fn handle_result(
        self,
        result: ExecutionResult,
    ) -> Result<SimpleStatefulAgent<SimpleComplete>, SimpleStatefulAgent<SimpleProcessing>> {
        if result.is_success() {
            // Tool succeeded, we can complete
            let final_result = format!(
                "Input: {}\nTool result: {}",
                self.state.input,
                result.output()
            );

            Ok(SimpleStatefulAgent {
                memory: self.memory,
                state: SimpleComplete {
                    input: self.state.input,
                    context: self.state.context,
                    result: final_result,
                },
            })
        } else {
            // Tool failed, go back to processing for retry
            Err(SimpleStatefulAgent {
                memory: self.memory,
                state: SimpleProcessing {
                    input: self.state.input,
                    context: self.state.context,
                },
            })
        }
    }
}

// Implementation for Complete state
impl SimpleStatefulAgent<SimpleComplete> {
    /// Generate the final action/response
    pub fn act(self) -> String {
        self.state.result.clone()
    }

    /// Get the final result without consuming the agent
    pub fn result(&self) -> &str {
        &self.state.result
    }

    /// Check if processing is complete
    pub fn is_complete(&self) -> bool {
        true
    }
}

// Shared implementations across all states
impl<State> SimpleStatefulAgent<State> {
    /// Get read-only memory access
    pub fn memory_reader(&self) -> &dyn MemoryReader {
        &*self.memory
    }
}

// Implement the StatefulAgent trait for all states
impl<State> crate::agent::stateful::StatefulAgent<State> for SimpleStatefulAgent<State> {
    type Observation = String;
    type Action = String;

    fn memory_reader(&self) -> &dyn MemoryReader {
        &*self.memory
    }

    fn get_tool_calls(&self) -> Vec<ToolCall> {
        // Default empty - specific states override this via their own methods
        Vec::new()
    }

    fn is_complete(&self) -> bool {
        // Default false - only Complete state returns true
        false
    }
}

// Specialized implementations for specific states
impl SimpleStatefulAgent<SimpleComplete> {
    pub fn is_complete_state(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InMemoryMemory;

    #[test]
    fn test_stateful_agent_lifecycle_without_tools() {
        let memory = Box::new(InMemoryMemory::new());

        // Start in initial state
        let agent = SimpleStatefulAgent::new(memory);

        // Observe input -> processing state
        let processing_agent = agent.observe("Hello world".to_string());

        // Check if tools are needed
        assert!(!processing_agent.needs_tools());

        // Complete without tools
        let complete_agent = processing_agent.complete_without_tools();

        // Generate final action
        let result = complete_agent.act();
        assert!(result.contains("Processed: Hello world"));
    }

    #[test]
    fn test_stateful_agent_lifecycle_with_tools() {
        let memory = Box::new(InMemoryMemory::new());

        // Start in initial state
        let agent = SimpleStatefulAgent::new(memory);

        // Observe input that needs tools -> processing state
        let processing_agent = agent.observe("search for rust patterns".to_string());

        // Check if tools are needed
        assert!(processing_agent.needs_tools());

        // Request tools -> tool execution state
        let tool_agent = processing_agent.request_tools();

        // Verify pending tools
        assert!(!tool_agent.pending_tools().is_empty());

        // Simulate successful tool execution
        let result = ExecutionResult::success("Found great rust patterns!".to_string());
        let complete_agent = match tool_agent.handle_result(result) {
            Ok(agent) => agent,
            Err(_) => panic!("Tool should have succeeded"),
        };

        // Generate final action
        let final_result = complete_agent.act();
        assert!(final_result.contains("Tool result: Found great rust patterns!"));
    }

    #[test]
    fn test_tool_failure_handling() {
        let memory = Box::new(InMemoryMemory::new());
        let agent = SimpleStatefulAgent::new(memory);

        // Process input that needs tools
        let processing_agent = agent.observe("search for something".to_string());
        let tool_agent = processing_agent.request_tools();

        // Simulate failed tool execution
        let result = ExecutionResult::failure("Network error".to_string());
        let back_to_processing = match tool_agent.handle_result(result) {
            Ok(_) => panic!("Tool should have failed"),
            Err(agent) => agent,
        };

        // Should be back in processing state, can try different approach
        assert!(!back_to_processing.get_tool_calls().is_empty());
    }
}
