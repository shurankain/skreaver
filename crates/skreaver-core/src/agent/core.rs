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
/// 1. **Initialize** - Optional setup before first use (can fail)
/// 2. **Observe** - Process incoming observations from the environment
/// 3. **Reason** - Determine what tools to call based on observations and memory
/// 4. **Act** - Execute tools and generate responses
/// 5. **Update** - Store results and new context in memory
/// 6. **Cleanup** - Optional teardown on shutdown (can fail)
///
/// # Error Handling
///
/// Agents can specify a custom error type for initialization and cleanup operations.
/// If your agent doesn't need error handling, use `std::convert::Infallible` as the error type.
///
/// # Example
///
/// ```rust
/// use skreaver_core::{Agent, MemoryReader, MemoryWriter, MemoryUpdate};
/// use skreaver_core::{ExecutionResult, ToolCall};
/// use skreaver_core::InMemoryMemory;
///
/// // Agent without error handling
/// struct SimpleAgent {
///     memory: InMemoryMemory,
/// }
///
/// impl Agent for SimpleAgent {
///     type Observation = String;
///     type Action = String;
///     type Error = std::convert::Infallible;  // No errors possible
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
///         Vec::new()
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
///
/// // Agent with custom error handling
/// #[derive(Debug, thiserror::Error)]
/// enum MyError {
///     #[error("Initialization failed: {0}")]
///     InitFailed(String),
///     #[error("Cleanup failed: {0}")]
///     CleanupFailed(String),
/// }
///
/// struct FallibleAgent {
///     memory: InMemoryMemory,
///     initialized: bool,
/// }
///
/// impl Agent for FallibleAgent {
///     type Observation = String;
///     type Action = String;
///     type Error = MyError;
///
///     fn initialize(&mut self) -> Result<(), Self::Error> {
///         self.initialized = true;
///         Ok(())
///     }
///
///     fn observe(&mut self, input: String) {
///         // Process the observation
///     }
///
///     fn act(&mut self) -> String {
///         "Response".to_string()
///     }
///
///     fn call_tools(&self) -> Vec<ToolCall> {
///         Vec::new()
///     }
///
///     fn handle_result(&mut self, _result: ExecutionResult) {}
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
///
///     fn cleanup(&mut self) -> Result<(), Self::Error> {
///         self.initialized = false;
///         Ok(())
///     }
/// }
/// ```
pub trait Agent {
    /// Type of observations the agent processes
    type Observation;
    /// Type of actions the agent produces
    type Action;
    /// Type of errors the agent can produce during lifecycle operations
    ///
    /// Use `std::convert::Infallible` if your agent doesn't need error handling.
    /// This type is used for `initialize()` and `cleanup()` operations that may fail.
    type Error: std::error::Error;

    /// Initialize the agent (called once before first use)
    ///
    /// This hook allows agents to perform setup operations that might fail,
    /// such as loading configuration, establishing connections, or validating state.
    ///
    /// # Default Implementation
    ///
    /// The default implementation does nothing and always succeeds.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn initialize(&mut self) -> Result<(), Self::Error> {
    ///     self.load_config()?;
    ///     self.connect_to_database()?;
    ///     Ok(())
    /// }
    /// ```
    fn initialize(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Process an observation from the environment
    ///
    /// Note: Consider using the new error-returning methods if your agent
    /// needs to propagate errors during observation processing.
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

    /// Cleanup hook (called on shutdown or before dropping)
    ///
    /// This hook allows agents to perform cleanup operations such as
    /// closing connections, flushing buffers, or saving state.
    ///
    /// # Default Implementation
    ///
    /// The default implementation does nothing and always succeeds.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn cleanup(&mut self) -> Result<(), Self::Error> {
    ///     self.flush_memory()?;
    ///     self.close_connections()?;
    ///     Ok(())
    /// }
    /// ```
    fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
