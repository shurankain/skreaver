//! # Standard Tool Library
//!
//! This module provides a comprehensive collection of standard tools that agents
//! can use for common operations. These tools follow best practices and provide
//! reliable, well-tested functionality for typical agent workflows.
//!
//! ## Tool Domains
//!
//! - **I/O Tools**: File system operations and directory management
//! - **Network Tools**: HTTP/REST API interactions with authentication support
//! - **Data Tools**: JSON/XML/text processing and transformation
//!
//! ## Usage
//!
//! ```rust
//! use skreaver_tools::{HttpGetTool, FileReadTool, InMemoryToolRegistry};
//! use std::sync::Arc;
//!
//! let registry = InMemoryToolRegistry::new()
//!     .with_tool("http_get", Arc::new(HttpGetTool::new()))
//!     .with_tool("file_read", Arc::new(FileReadTool::new()));
//! ```

/// Data processing and transformation tools
pub mod data;
/// File system I/O operations
pub mod io;
/// Network communication tools
pub mod network;

pub use data::{JsonParseTool, JsonTransformTool, XmlParseTool};
pub use data::{
    TextAnalyzeTool, TextReverseTool, TextSearchTool, TextSplitTool, TextUppercaseTool,
};
pub use io::{DirectoryCreateTool, DirectoryListTool, FileReadTool, FileWriteTool};
pub use network::{HttpDeleteTool, HttpGetTool, HttpPostTool, HttpPutTool};
