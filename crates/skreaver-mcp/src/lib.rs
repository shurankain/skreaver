//! # Skreaver MCP - Model Context Protocol Integration
//!
//! This crate provides MCP (Model Context Protocol) integration for Skreaver,
//! implementing the **2025-11-25 specification** with support for tasks,
//! elicitation, tool annotations, and sampling with tools.
//!
//! ## Features
//!
//! - **MCP Server**: Expose Skreaver tools as MCP resources
//! - **MCP Bridge**: Use external MCP servers as Skreaver tools (requires `client` feature)
//! - **Tasks**: Long-running operations with polling and deferred results (2025-11-25)
//! - **Elicitation**: Server-initiated user input requests (2025-11-25)
//! - **Tool Annotations**: Behavior hints (readOnly, destructive, idempotent, openWorld)
//! - **Type Safety**: Full type-safe MCP protocol implementation
//! - **Async Runtime**: Built on Tokio for high performance
//!
//! ## Example: MCP Server
//!
//! ```rust,no_run
//! use skreaver_mcp::McpServer;
//! use skreaver_tools::InMemoryToolRegistry;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create tool registry with your tools
//!     let tools = InMemoryToolRegistry::new();
//!
//!     // Create and start MCP server
//!     let server = McpServer::new(&tools);
//!     server.serve_stdio().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Example: MCP Bridge (requires `client` feature)
//!
//! ```rust,ignore
//! use skreaver_mcp::McpBridge;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to an external MCP server
//!     let bridge = McpBridge::connect_stdio("npx @modelcontextprotocol/server-weather").await?;
//!
//!     // Use the discovered tools
//!     for tool in bridge.tools() {
//!         println!("Found tool: {}", tool.name());
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod adapter;
pub mod elicitation;
pub mod error;
pub mod server;
pub mod tasks;

// Bridge module requires client feature
#[cfg(feature = "client")]
pub mod bridge;

// Core re-exports
pub use adapter::{McpToolAnnotations, McpToolDefinition, ToolAdapter};
pub use error::{McpError, McpResult};
pub use server::McpServer;

// Tasks re-exports (2025-11-25 spec)
pub use tasks::{McpTask, McpTaskManager, McpTaskStatus};

// Elicitation re-exports (2025-11-25 spec)
pub use elicitation::{
    ElicitationAction, ElicitationMode, ElicitationRequest, ElicitationResponse,
    ElicitationSchemaBuilder,
};

#[cfg(feature = "client")]
pub use bridge::McpBridge;
