//! # Agent Core
//!
//! Core agent trait definition that forms the foundation of the Skreaver agent architecture.

use crate::memory::{MemoryReader, MemoryUpdate, MemoryWriter};
use crate::tool::{ExecutionResult, ToolCall};

/// Core agent trait defining the behavior of autonomous agents.
///
/// Agents are entities that can observe their environment, take actions,
/// use tools, and maintain persistent memory across interactions.
///
/// # Agent Lifecycle
///
/// 1. **Observe** - Process incoming observations from the environment
/// 2. **Reason** - Determine what tools to call based on observations and memory
/// 3. **Act** - Execute tools and generate responses
/// 4. **Update** - Store results and new context in memory
///
/// # Example
///
/// ```rust
/// use skreaver_core::{Agent, MemoryReader, MemoryWriter, MemoryUpdate};
/// use skreaver_core::{InMemoryMemory, ExecutionResult, ToolCall};
///
/// struct EchoAgent {
///     memory: InMemoryMemory,
/// }
///
/// impl Agent for EchoAgent {
///     type Observation = String;
///     type Action = String;
///
///     fn observe(&mut self, input: String) {
///         // Process the observation
///     }
///
///     fn act(&mut self) -> String {
///         "Hello, world!".to_string()
///     }
///
///     fn call_tools(&self) -> Vec<ToolCall> {
///         Vec::new() // No tools needed
///     }
///
///     fn handle_result(&mut self, _result: ExecutionResult) {
///         // Handle tool execution result
///     }
///
///     fn update_context(&mut self, update: MemoryUpdate) {
///         let _ = self.memory_writer().store(update);
///     }
///
///     fn memory_reader(&self) -> &dyn MemoryReader {
///         &self.memory
///     }
///
///     fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
///         &mut self.memory
///     }
/// }
/// ```
pub trait Agent {
    /// Type of observations the agent processes
    type Observation;
    /// Type of actions the agent produces
    type Action;

    /// Process an observation from the environment
    fn observe(&mut self, input: Self::Observation);

    /// Generate an action based on current state and memory
    fn act(&mut self) -> Self::Action;

    /// Determine what tools to call based on current state
    fn call_tools(&self) -> Vec<ToolCall>;

    /// Handle the result of tool execution
    fn handle_result(&mut self, result: ExecutionResult);

    /// Update the agent's context with new information
    fn update_context(&mut self, update: MemoryUpdate);

    /// Get read-only access to the agent's memory
    fn memory_reader(&self) -> &dyn MemoryReader;

    /// Get mutable access to the agent's memory
    fn memory_writer(&mut self) -> &mut dyn MemoryWriter;
}
