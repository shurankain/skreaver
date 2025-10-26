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
//! use skreaver::{Agent, MemoryReader, MemoryWriter, MemoryUpdate, InMemoryMemory};
//! use skreaver::{InMemoryToolRegistry, ToolCall, ExecutionResult};
//! # use skreaver::runtime::Coordinator;
//!
//! // Define your agent
//! struct MyAgent {
//!     memory: InMemoryMemory,
//! }
//!
//! impl Agent for MyAgent {
//!     type Observation = String;
//!     type Action = String;
//!     type Error = std::convert::Infallible;
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

#[allow(ambiguous_glob_reexports)]
pub use skreaver_core::*;
pub use skreaver_memory::*;
pub use skreaver_tools::*;

#[allow(ambiguous_glob_reexports)]
pub use skreaver_http::*;

// Conditionally re-export testing utilities
#[cfg(feature = "testing")]
pub use skreaver_testing::*;

// Create convenient module aliases
pub use skreaver_core as core;
pub use skreaver_memory as memory;
pub use skreaver_tools as tools;

pub use skreaver_http as http;

#[cfg(feature = "testing")]
pub use skreaver_testing as testing;

// Memory types already exported via skreaver_core::*

// Memory backend implementations
pub use skreaver_memory::{FileMemory, NamespacedMemory};

#[cfg(feature = "redis")]
pub use skreaver_memory::RedisMemory;
