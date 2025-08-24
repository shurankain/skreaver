//! # Standard Tool Library
//!
//! This module provides a comprehensive collection of standard tools that agents
//! can use for common operations. These tools follow best practices and provide
//! reliable, well-tested functionality for typical agent workflows.
//!
//! ## Tool Categories
//!
//! - **HTTP Tools**: REST API interactions with authentication support
//! - **File Operations**: File system read/write/directory operations
//! - **JSON/XML Processing**: Data transformation and validation
//! - **Text Processing**: String manipulation and analysis
//!
//! ## Usage
//!
//! ```rust
//! use skreaver::tool::standard::{HttpGetTool, FileReadTool};
//! use skreaver::tool::registry::InMemoryToolRegistry;
//! use std::sync::Arc;
//!
//! let registry = InMemoryToolRegistry::new()
//!     .with_tool("http_get", Arc::new(HttpGetTool::new()))
//!     .with_tool("file_read", Arc::new(FileReadTool::new()));
//! ```

/// JSON and XML data processing tools
pub mod data;
/// File system operation tools
pub mod file;
/// HTTP client tools for REST API interactions
pub mod http;
/// Text processing and manipulation tools
pub mod text;

pub use data::{JsonParseTool, JsonTransformTool, XmlParseTool};
pub use file::{DirectoryCreateTool, DirectoryListTool, FileReadTool, FileWriteTool};
pub use http::{HttpDeleteTool, HttpGetTool, HttpPostTool, HttpPutTool};
pub use text::{
    TextAnalyzeTool, TextReverseTool, TextSearchTool, TextSplitTool, TextUppercaseTool,
};
