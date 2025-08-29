//! Agent core types and traits.
//!
//! This module re-exports all agent-related types from skreaver-core
//! to maintain API compatibility while consolidating implementation.

// Re-export all core agent types and traits
pub use skreaver_core::agent::Agent;

// Re-export memory-related types that agents use
pub use skreaver_core::memory::{MemoryReader, MemoryUpdate, MemoryWriter};

// Re-export tool-related types that agents use
pub use skreaver_core::tool::{ExecutionResult, ToolCall};
