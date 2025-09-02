//! # Skreaver Core
//!
//! Core traits and types for the Skreaver agent framework.
//! This crate provides the fundamental building blocks for creating AI agents.

pub mod agent;
pub mod error;
pub mod in_memory;
pub mod memory;
pub mod tool;

pub use agent::Agent;
pub use error::{SkreverError, SkreverResult};
pub use in_memory::InMemoryMemory;
pub use memory::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter};
pub use tool::{ExecutionResult, StandardTool, Tool, ToolCall, ToolDispatch};

// Re-export agent extensions
pub use agent::{
    CompleteState, InitialState, ProcessingState, SimpleComplete, SimpleInitial, SimpleProcessing,
    SimpleStatefulAgent, SimpleToolExecution, StatefulAgent, StatefulAgentAdapter,
    StatefulAgentTransitions, ToolExecutionState,
};
