//! Tool core types and traits.
//!
//! This module re-exports all tool-related types from skreaver-core
//! to maintain API compatibility while consolidating implementation.

// Re-export all core tool types and traits
pub use skreaver_core::tool::{
    ExecutionResult, Tool, ToolCall, ToolCallBuildError, ToolCallBuilder,
};

// Type aliases for backward compatibility
// ToolName has been deprecated in favor of ToolId which provides the same validation
pub use skreaver_core::ToolId as ToolName;
pub use skreaver_core::ValidationError as InvalidToolName;
