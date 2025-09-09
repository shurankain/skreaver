//! # Skreaver
//!
//! Skreaver is a Rust-native coordination runtime for building modular AI agents.
//! This is a compatibility wrapper that re-exports from the main skreaver crate.
//!
//! ## Migration Note
//!
//! For new code, prefer importing from the main `skreaver` crate instead:
//!
//! ```rust
//! // New preferred way
//! use skreaver::{Agent, MemoryReader, MemoryWriter};
//!
//! // Or use specific crates for more fine-grained control  
//! use skreaver_core::{Agent as CoreAgent, MemoryReader as CoreMemoryReader, MemoryWriter as CoreMemoryWriter};
//! use skreaver_tools::{HttpGetTool, JsonParseTool};
//! use skreaver_http::runtime::Coordinator;
//! ```

// Re-export everything from the main skreaver crate for compatibility
#[allow(ambiguous_glob_reexports)]
pub use skreaver_core::*;
#[allow(ambiguous_glob_reexports)]
pub use skreaver_http::*;
pub use skreaver_tools::{ExecutionResult, Tool, ToolCall};

/// Production benchmark framework for performance measurement and regression detection
pub mod benchmarks;
