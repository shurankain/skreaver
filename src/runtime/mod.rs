//! # Runtime Module
//!
//! This module provides the execution runtime for Skreaver agents. The runtime
//! orchestrates the interaction between agents, tools, and memory systems,
//! managing the complete lifecycle of agent operations.
//!
//! ## Core Component
//!
//! - **[Coordinator]**: Central runtime that manages agent execution, tool dispatch,
//!   and memory operations in a coordinated manner
//!
//! ## Responsibilities
//!
//! - **Agent Execution**: Drives the agent observation-action cycle
//! - **Tool Orchestration**: Routes tool calls to appropriate implementations  
//! - **Memory Management**: Ensures state persistence across interactions
//! - **Error Handling**: Manages failures in tool execution and memory operations
//!
//! ## Usage Pattern
//!
//! ```rust
//! use skreaver::{Coordinator, Agent, Memory, MemoryUpdate};
//! use skreaver::memory::InMemoryMemory;
//! use skreaver::tool::registry::InMemoryToolRegistry;
//!
//! // Example agent implementation
//! struct SimpleAgent {
//!     memory: Box<dyn Memory + Send>,
//! }
//!
//! impl Agent for SimpleAgent {
//!     type Observation = String;
//!     type Action = String;
//!     
//!     fn memory(&mut self) -> &mut dyn Memory { &mut *self.memory }
//!     fn observe(&mut self, _input: String) {}
//!     fn act(&mut self) -> String { "response".to_string() }
//!     fn update_context(&mut self, update: MemoryUpdate) { self.memory().store(update); }
//! }
//!
//! // Create agent and coordinate execution
//! let agent = SimpleAgent { memory: Box::new(InMemoryMemory::new()) };
//! let registry = InMemoryToolRegistry::new();
//! let mut coordinator = Coordinator::new(agent, registry);
//! let result = coordinator.step("user input".to_string());
//! ```

/// Central coordinator for agent execution and tool dispatch.
pub mod coordinator;
pub use coordinator::Coordinator;
