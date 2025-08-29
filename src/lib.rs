//! # Skreaver
//!
//! Skreaver is a Rust-native coordination runtime for building modular AI agents.
//! It provides a flexible architecture for creating autonomous agents that can
//! reason, use tools, and maintain memory across interactions.
//!
//! ## Core Components
//!
//! - **[Agent]**: Core trait defining agent behavior with observation, action, and tool usage
//! - **[MemoryReader], [MemoryWriter]**: Persistent storage for agent state and context
//! - **[Tool]**: External capabilities that agents can invoke
//! - **[Coordinator]**: Runtime that orchestrates agent execution and tool dispatch
//!
//! ## Quick Start
//!
//! ```rust
//! use skreaver::{Agent, MemoryReader, MemoryWriter, MemoryUpdate, Coordinator};
//! use skreaver::memory::InMemoryMemory;
//! use skreaver::tool::registry::InMemoryToolRegistry;
//! use skreaver::tool::{ToolCall, ExecutionResult};
//!
//! // Define your agent
//! struct MyAgent {
//!     memory: InMemoryMemory,
//! }
//!
//! impl Agent for MyAgent {
//!     type Observation = String;
//!     type Action = String;
//!
//!     fn memory_reader(&self) -> &dyn MemoryReader {
//!         &self.memory
//!     }
//!
//!     fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
//!         &mut self.memory
//!     }
//!
//!     fn observe(&mut self, input: String) {
//!         // Process observation
//!     }
//!
//!     fn act(&mut self) -> String {
//!         "Hello from agent".to_string()
//!     }
//!
//!     fn call_tools(&self) -> Vec<ToolCall> {
//!         Vec::new()
//!     }
//!
//!     fn handle_result(&mut self, result: ExecutionResult) {
//!         // Handle tool execution results
//!     }
//!
//!     fn update_context(&mut self, update: MemoryUpdate) {
//!         let _ = self.memory_writer().store(update);
//!     }
//! }
//! ```
//!
//! ## Architecture
//!
//! Skreaver follows a modular architecture where agents coordinate through a runtime
//! that manages tool execution and memory persistence. This enables building complex
//! AI systems with clear separation of concerns and robust error handling.

pub mod agent;
pub mod error;
pub mod memory;
pub mod runtime;
pub mod testing;
pub mod tool;

pub use agent::Agent;
pub use error::{SkreverError, SkreverResult};
pub use memory::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter};
pub use runtime::Coordinator;
pub use tool::{ExecutionResult, Tool, ToolCall};
