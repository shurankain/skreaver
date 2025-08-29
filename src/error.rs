//! Error types and utilities.
//!
//! This module re-exports all error-related types from skreaver-core
//! to maintain API compatibility while consolidating implementation.

// Re-export all core error types
pub use skreaver_core::error::{
    MemoryError, SkreverError, SkreverResult, ToolError, TransactionError,
};
