//! # Skreaver Tools
//!
//! This crate provides a comprehensive standard library of tools for Skreaver agents,
//! including I/O operations, network communication, and data processing capabilities.
//!
//! ## Features
//!
//! - **I/O Tools** (`io`): File system operations and directory management
//! - **Network Tools** (`network`): HTTP/REST API interactions
//! - **Data Tools** (`data`): JSON/XML/text processing and transformation
//!
//! ## Tool Categories
//!
//! ### I/O Tools
//! - File operations: read, write, directory listing
//! - Path validation and security checks
//!
//! ### Network Tools  
//! - HTTP methods: GET, POST, PUT, DELETE
//! - Authentication and header management
//!
//! ### Data Tools
//! - JSON parsing and transformation
//! - XML processing
//! - Text analysis and manipulation

/// Core tool trait definitions and data structures.
pub mod core;
/// Tool registry implementations for managing collections of tools.
pub mod registry;
/// Secure tool registry with RBAC enforcement.
pub mod secure_registry;
/// Standard tool library providing common functionality.
pub mod standard;

pub use core::{ToolCallBuildError, ToolCallBuilder};
// Type aliases for backward compatibility - ToolName now maps to ToolId
pub use core::{InvalidToolName, ToolName};
pub use registry::{InMemoryToolRegistry, ToolRegistry};
pub use secure_registry::SecureToolRegistry;
pub use skreaver_core::{ExecutionResult, StandardTool, Tool, ToolCall, ToolDispatch};
pub use standard::*;
