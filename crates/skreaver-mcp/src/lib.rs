//! # Skreaver MCP - Model Context Protocol Integration
//!
//! This crate provides MCP (Model Context Protocol) integration for Skreaver,
//! enabling interoperability with Claude Desktop and other MCP-compatible clients.
//!
//! ## Features
//!
//! - **MCP Server**: Expose Skreaver tools as MCP resources
//! - **MCP Bridge**: Use external MCP servers as Skreaver tools
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

pub mod error;
pub mod server;
pub mod bridge;
pub mod adapter;

pub use error::{McpError, McpResult};
pub use server::McpServer;
pub use bridge::McpBridge;
pub use adapter::ToolAdapter;
