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
//! use skreaver::{Coordinator, Agent};
//! use skreaver::memory::InMemoryMemory;
//! use skreaver::tool::registry::InMemoryToolRegistry;
//!
//! // Create agent, memory, and tool registry
//! let agent = MyAgent::new();
//! let registry = InMemoryToolRegistry::new();
//! 
//! // Coordinate execution
//! let mut coordinator = Coordinator::new(agent, registry);
//! let result = coordinator.step("user input");
//! ```

/// Central coordinator for agent execution and tool dispatch.
pub mod coordinator;
pub use coordinator::Coordinator;
