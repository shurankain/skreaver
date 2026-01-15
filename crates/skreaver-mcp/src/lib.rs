//! # Skreaver MCP - Model Context Protocol Integration
//!
//! This crate provides MCP (Model Context Protocol) integration for Skreaver,
//! enabling interoperability with Claude Desktop and other MCP-compatible clients.
//!
//! ## Features
//!
//! - **MCP Server**: Expose Skreaver tools as MCP resources
//! - **MCP Bridge**: Use external MCP servers as Skreaver tools (requires `client` feature)
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
pub mod error;
pub mod server;

// Bridge module requires client feature
#[cfg(feature = "client")]
pub mod bridge;

pub use adapter::ToolAdapter;
pub use error::{McpError, McpResult};
pub use server::McpServer;

#[cfg(feature = "client")]
pub use bridge::McpBridge;
