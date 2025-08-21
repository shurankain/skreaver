//! # Skreaver
//!
//! Skreaver is a Rust-native coordination runtime for building modular AI agents.
//! It provides a flexible architecture for creating autonomous agents that can
//! reason, use tools, and maintain memory across interactions.
//!
//! ## Core Components
//!
//! - **[Agent]**: Core trait defining agent behavior with observation, action, and tool usage
//! - **[Memory]**: Persistent storage for agent state and context
//! - **[Tool]**: External capabilities that agents can invoke
//! - **[Coordinator]**: Runtime that orchestrates agent execution and tool dispatch
//!
//! ## Quick Start
//!
//! ```rust
//! use skreaver::{Agent, Memory, MemoryUpdate, Coordinator};
//! use skreaver::memory::InMemoryMemory;
//! use skreaver::tool::registry::InMemoryToolRegistry;
//!
//! // Define your agent
//! struct MyAgent {
//!     memory: Box<dyn Memory + Send>,
//! }
//!
//! impl Agent for MyAgent {
//!     type Observation = String;
//!     type Action = String;
//!
//!     fn memory(&mut self) -> &mut dyn Memory {
//!         &mut *self.memory
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
//!     fn update_context(&mut self, update: MemoryUpdate) {
//!         self.memory().store(update);
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
pub mod memory;
pub mod runtime;
pub mod tool;

pub use agent::Agent;
pub use memory::{Memory, MemoryUpdate};
pub use runtime::Coordinator;
pub use tool::{ExecutionResult, Tool, ToolCall};
