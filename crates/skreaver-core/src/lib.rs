//! # Skreaver Core
//!
//! Core traits and types for the Skreaver agent framework.
//! This crate provides the fundamental building blocks for creating AI agents.

pub mod agent;
pub mod error;
pub mod file_memory;
pub mod in_memory;
pub mod memory;
pub mod namespaced_memory;
#[cfg(feature = "redis")]
pub mod redis_memory;
pub mod tool;

pub use agent::Agent;
pub use error::{SkreverError, SkreverResult};
pub use file_memory::FileMemory;
pub use in_memory::InMemoryMemory;
pub use memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};
pub use namespaced_memory::NamespacedMemory;
#[cfg(feature = "redis")]
pub use redis_memory::RedisMemory;
pub use tool::{ExecutionResult, StandardTool, Tool, ToolCall, ToolDispatch};

// Re-export agent extensions
pub use agent::{
    CompleteState, InitialState, ProcessingState, SimpleComplete, SimpleInitial, SimpleProcessing,
    SimpleStatefulAgent, SimpleToolExecution, StatefulAgent, StatefulAgentAdapter,
    StatefulAgentTransitions, ToolExecutionState,
};
