//! # Skreaver Agent - Unified Agent Interface
//!
//! This crate provides a unified interface for working with AI agents
//! across different protocols (MCP, A2A).
//!
//! ## Features
//!
//! - **Unified Types**: Protocol-agnostic message, task, and capability types
//! - **Unified Traits**: Common interface for all agent implementations
//! - **MCP Adapter**: Use MCP servers through the unified interface (requires `mcp` feature)
//! - **A2A Adapter**: Use A2A agents through the unified interface (requires `a2a` feature)
//! - **Protocol Bridge**: Connect agents across protocols
//!
//! ## Example: Using an MCP Server
//!
//! ```rust,ignore
//! use skreaver_agent::{McpAgentAdapter, UnifiedAgent, UnifiedMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to an MCP server
//!     let agent = McpAgentAdapter::connect("npx @modelcontextprotocol/server-weather").await?;
//!
//!     // Send a message
//!     let task = agent.send_message(UnifiedMessage::user("What's the weather?")).await?;
//!
//!     println!("Response: {:?}", task);
//!     Ok(())
//! }
//! ```
//!
//! ## Example: Using an A2A Agent
//!
//! ```rust,ignore
//! use skreaver_agent::{A2aAgentAdapter, UnifiedAgent, UnifiedMessage};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to an A2A agent
//!     let agent = A2aAgentAdapter::connect("https://agent.example.com").await?;
//!
//!     // Send a message
//!     let task = agent.send_message(UnifiedMessage::user("Hello!")).await?;
//!
//!     println!("Response: {:?}", task);
//!     Ok(())
//! }
//! ```
//!
//! ## Example: Agent Discovery
//!
//! ```rust,ignore
//! use skreaver_agent::{DiscoveryService, Protocol};
//!
//! let service = DiscoveryService::new();
//!
//! // Register agents
//! service.register_agent(mcp_agent).await?;
//! service.register_agent(a2a_agent).await?;
//!
//! // Find agents by protocol
//! let a2a_agents = service.find_by_protocol(Protocol::A2a).await?;
//!
//! // Find agents by capability
//! let search_agents = service.find_by_capability("search").await?;
//! ```

pub mod bridge;
pub mod discovery;
pub mod error;
pub mod orchestration;
pub mod protocol_bridge;
pub mod storage;
pub mod traits;
pub mod types;

// MCP adapter (requires mcp feature)
#[cfg(feature = "mcp")]
pub mod mcp;

// A2A adapter (requires a2a feature)
#[cfg(feature = "a2a")]
pub mod a2a;

// Re-export core types
pub use error::{AgentError, AgentResult};
pub use traits::{
    AgentServer, MessageBuilder, StreamingAgentServer, TaskBuilder, ToolInvoker, UnifiedAgent,
    artifact_added, error_event, message_added, status_update,
};
pub use types::{
    AgentInfo, Artifact, Capability, ContentPart, MessageRole, Protocol, StreamEvent, TaskStatus,
    UnifiedMessage, UnifiedTask,
};

// Re-export bridge types
pub use bridge::{FanOutAgent, ProxyAgent};

// Re-export discovery types
pub use discovery::{
    AgentRegistration, BackgroundTaskHandle, DeregistrationReason, DiscoveryConfig, DiscoveryEvent,
    DiscoveryProvider, DiscoveryQuery, DiscoveryService, HealthStatus, InMemoryDiscoveryProvider,
};

// Re-export orchestration types
pub use orchestration::{
    AggregationMode, CapabilityBasedSupervisor, ParallelAgent, RouterAgent, RoutingRule,
    SequentialPipeline, SupervisorAgent, SupervisorDecision, SupervisorLogic, TransformMode,
};

// Re-export storage types
pub use storage::{FileTaskStore, InMemoryTaskStore, TaskQuery, TaskStore, TaskStoreExt};

// Re-export MCP adapter
#[cfg(feature = "mcp")]
pub use mcp::McpAgentAdapter;

// Re-export A2A adapter
#[cfg(feature = "a2a")]
pub use a2a::A2aAgentAdapter;

// Re-export A2A bridge handler
#[cfg(feature = "a2a")]
pub use bridge::A2aBridgeHandler;

// Re-export protocol bridge types
#[cfg(feature = "mcp")]
pub use protocol_bridge::{InputTransform, McpToA2aBridge, ToolMapping};

#[cfg(feature = "a2a")]
pub use protocol_bridge::{A2aToMcpBridge, SkillToToolMapping};

#[cfg(all(feature = "mcp", feature = "a2a"))]
pub use protocol_bridge::{ProtocolGateway, a2a_parts_to_mcp_result, mcp_result_to_a2a_parts};
