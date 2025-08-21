//! # Tool Module
//!
//! This module defines the tool system that allows agents to interact with external
//! capabilities and services. Tools extend agent functionality beyond internal reasoning
//! to include actions like API calls, file operations, calculations, and more.
//!
//! ## Core Components
//!
//! - **[Tool]**: Trait defining a callable capability with input/output
//! - **[ToolCall]**: Request structure for invoking a specific tool
//! - **[ExecutionResult]**: Response containing tool output and success status
//! - **[ToolRegistry]**: Collection managing available tools for dispatch
//!
//! ## Tool Lifecycle
//!
//! 1. **Registration** - Tools are added to a registry by name
//! 2. **Invocation** - Agents create ToolCall requests  
//! 3. **Dispatch** - Registry routes calls to appropriate tool implementations
//! 4. **Execution** - Tool processes input and returns structured result
//!
//! ## Usage
//!
//! ```rust
//! use skreaver::tool::{Tool, ExecutionResult, ToolCall};
//!
//! struct EchoTool;
//!
//! impl Tool for EchoTool {
//!     fn name(&self) -> &str { "echo" }
//!     
//!     fn call(&self, input: String) -> ExecutionResult {
//!         ExecutionResult {
//!             output: format!("Echo: {}", input),
//!             success: true,
//!         }
//!     }
//! }
//! ```

pub mod registry;
pub mod r#trait;
pub use registry::ToolRegistry;
pub use r#trait::{ExecutionResult, Tool, ToolCall};
