//! Tool core types and traits.
//!
//! This module re-exports all tool-related types from skreaver-core
//! to maintain API compatibility while consolidating implementation.

// Re-export all core tool types and traits
pub use skreaver_core::tool::{
    ExecutionResult, InvalidToolName, Tool, ToolCall, ToolCallBuildError, ToolCallBuilder, ToolName,
};
