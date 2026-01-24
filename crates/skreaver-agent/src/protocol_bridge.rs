//! Protocol bridging for translating between MCP and A2A protocols.
//!
//! This module provides bridges that enable seamless interoperability between
//! the Model Context Protocol (MCP) and Agent-to-Agent (A2A) protocols:
//!
//! - `McpToA2aBridge`: Exposes MCP tools as an A2A-compatible agent (requires `mcp` feature)
//! - `A2aToMcpBridge`: Exposes A2A agent skills as MCP tools (requires `a2a` feature)
//! - `ProtocolGateway`: Unified gateway managing multiple protocol bridges (requires both features)
//!
//! # Protocol Comparison
//!
//! | Aspect | MCP | A2A |
//! |--------|-----|-----|
//! | Purpose | Tool access for LLMs | Agent interoperability |
//! | Primitives | Tools, Resources, Prompts | Tasks, Messages, Artifacts |
//! | Communication | Bidirectional JSON-RPC | HTTP REST + SSE |
//! | State | Stateless tool calls | Stateful task lifecycle |
//! | Streaming | Native protocol support | Server-Sent Events |
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐         ┌─────────────────┐
//! │   MCP Client    │◄───────►│ McpToA2aBridge  │◄────► A2A Agent API
//! │ (Claude Desktop)│         │ (Exposes as A2A)│
//! └─────────────────┘         └─────────────────┘
//!
//! ┌─────────────────┐         ┌─────────────────┐
//! │   A2A Client    │◄───────►│ A2aToMcpBridge  │◄────► MCP Server
//! │                 │         │ (Exposes as MCP)│
//! └─────────────────┘         └─────────────────┘
//!
//! ┌─────────────────────────────────────────────┐
//! │              ProtocolGateway                │
//! │  ┌─────────────┐     ┌─────────────┐       │
//! │  │ MCP Agents  │◄───►│ A2A Agents  │       │
//! │  └─────────────┘     └─────────────┘       │
//! │           ▲               ▲                 │
//! │           └───────┬───────┘                 │
//! │                   │                         │
//! │         Unified Agent Registry              │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # Type Mapping
//!
//! ## MCP → A2A Conversion
//!
//! | MCP Concept | A2A Equivalent | Notes |
//! |-------------|----------------|-------|
//! | Tool | Skill | 1:1 mapping, input schemas preserved |
//! | Tool Call | Message + ToolCall part | Wrapped in task context |
//! | Tool Result | Message + ToolResult part | Added to task history |
//! | Resource | Artifact | URI-based content |
//! | Prompt | N/A | No direct equivalent |
//!
//! ## A2A → MCP Conversion
//!
//! | A2A Concept | MCP Equivalent | Notes |
//! |-------------|----------------|-------|
//! | Skill | Tool | Skill becomes callable tool |
//! | Task | Tool call session | Task ID tracks conversation |
//! | Message | Tool call/result | Depends on direction |
//! | Artifact | Tool result content | Serialized as JSON |
//! | Status updates | N/A | Polling required |
//!
//! # Usage Examples
//!
//! ## Exposing MCP Server as A2A Agent
//!
//! ```rust,ignore
//! use skreaver_agent::{McpToA2aBridge, McpAgentAdapter};
//!
//! // Connect to an MCP server (e.g., filesystem tools)
//! let mcp_agent = McpAgentAdapter::connect("npx @anthropic/mcp-server-fs").await?;
//!
//! // Create A2A bridge
//! let bridge = McpToA2aBridge::new(Arc::new(mcp_agent))
//!     .with_name("Filesystem Agent")
//!     .with_description("Access filesystem via A2A protocol");
//!
//! // Now 'bridge' can be used with A2A clients or A2aServer
//! ```
//!
//! ## Exposing A2A Agent as MCP Tools
//!
//! ```rust,ignore
//! use skreaver_agent::{A2aToMcpBridge, A2aAgentAdapter};
//!
//! // Connect to an A2A agent
//! let a2a_agent = A2aAgentAdapter::connect("https://agent.example.com").await?;
//!
//! // Create MCP bridge - each A2A skill becomes an MCP tool
//! let bridge = A2aToMcpBridge::new(Arc::new(a2a_agent));
//!
//! // Get skill mappings for MCP server registration
//! for (skill_id, mapping) in bridge.skill_mappings() {
//!     println!("Tool '{}' maps to skill '{}'", mapping.tool_name, skill_id);
//! }
//! ```
//!
//! ## Unified Gateway
//!
//! ```rust,ignore
//! use skreaver_agent::{ProtocolGateway, Protocol};
//!
//! let mut gateway = ProtocolGateway::new();
//!
//! // Register agents from both protocols
//! gateway.register_mcp_agent(mcp_filesystem);
//! gateway.register_mcp_agent(mcp_database);
//! gateway.register_a2a_agent(a2a_search);
//! gateway.register_a2a_agent(a2a_analysis);
//!
//! // Query agents by protocol
//! let a2a_agents = gateway.agents_for_protocol(Protocol::A2a);
//!
//! // Find by capability
//! let search_agents = gateway.find_by_capability("search");
//!
//! // Route message to best agent
//! let result = gateway.route_message(message, None).await?;
//! ```
//!
//! # Feature Flags
//!
//! - `mcp`: Enables MCP-related bridges (`McpToA2aBridge`)
//! - `a2a`: Enables A2A-related bridges (`A2aToMcpBridge`)
//! - Both: Enables `ProtocolGateway` for full bidirectional bridging
//!
//! # Error Handling
//!
//! Protocol bridges preserve error context while translating error types:
//!
//! - MCP errors → `AgentError::Internal` with original message
//! - A2A errors → `AgentError::ConnectionError` or `AgentError::Internal`
//! - Task not found → `AgentError::TaskNotFound`
//! - Capability not found → `AgentError::CapabilityNotFound`

#[cfg(any(feature = "mcp", feature = "a2a"))]
use async_trait::async_trait;
#[cfg(any(feature = "mcp", feature = "a2a"))]
use futures::Stream;
#[cfg(any(feature = "mcp", feature = "a2a"))]
use std::collections::HashMap;
#[cfg(any(feature = "mcp", feature = "a2a"))]
use std::pin::Pin;
#[cfg(any(feature = "mcp", feature = "a2a"))]
use std::sync::Arc;
#[cfg(any(feature = "mcp", feature = "a2a"))]
use tracing::debug;
#[cfg(all(feature = "mcp", feature = "a2a"))]
use tracing::info;

#[cfg(any(feature = "mcp", feature = "a2a"))]
use crate::error::{AgentError, AgentResult};
#[cfg(any(feature = "mcp", feature = "a2a"))]
use crate::traits::UnifiedAgent;
#[cfg(any(feature = "mcp", feature = "a2a"))]
use crate::types::{
    AgentInfo, Capability, ContentPart, Protocol, StreamEvent, UnifiedMessage, UnifiedTask,
};

// ============================================================================
// McpToA2aBridge - Expose MCP tools as A2A agent
// ============================================================================

/// Bridge that exposes MCP tools as an A2A-compatible agent.
///
/// This allows MCP servers (like those used by Claude Desktop) to be
/// accessed by A2A clients. Each MCP tool becomes an A2A skill.
///
/// # Example
/// ```rust,ignore
/// use skreaver_agent::{McpAgentAdapter, McpToA2aBridge};
///
/// // Connect to an MCP server
/// let mcp_agent = McpAgentAdapter::connect("npx @anthropic/mcp-server-fs").await?;
///
/// // Create bridge to expose as A2A
/// let bridge = McpToA2aBridge::new(Arc::new(mcp_agent))
///     .with_name("Filesystem Agent")
///     .with_description("A2A bridge to MCP filesystem tools");
///
/// // Now `bridge` can be registered as an A2A agent
/// ```
#[cfg(feature = "mcp")]
pub struct McpToA2aBridge {
    info: AgentInfo,
    mcp_agent: Arc<dyn UnifiedAgent>,
    tool_mapping: HashMap<String, ToolMapping>,
    tasks: tokio::sync::RwLock<HashMap<String, UnifiedTask>>,
}

/// Mapping between MCP tool and A2A skill.
#[cfg(feature = "mcp")]
#[derive(Debug, Clone)]
pub struct ToolMapping {
    /// MCP tool name
    pub mcp_name: String,
    /// A2A skill ID
    pub a2a_skill_id: String,
    /// Description for A2A
    pub description: Option<String>,
    /// Input transformation (if needed)
    pub input_transform: Option<InputTransform>,
}

/// Input transformation rules.
#[cfg(feature = "mcp")]
#[derive(Debug, Clone)]
pub enum InputTransform {
    /// Pass through unchanged
    PassThrough,
    /// Extract from specific field
    ExtractField(String),
    /// Apply custom JSON transformation
    JsonPath(String),
}

#[cfg(feature = "mcp")]
impl McpToA2aBridge {
    /// Create a new MCP to A2A bridge.
    pub fn new(mcp_agent: Arc<dyn UnifiedAgent>) -> Self {
        let source_info = mcp_agent.info();

        // Build tool mappings from MCP capabilities
        let mut tool_mapping = HashMap::new();
        for cap in &source_info.capabilities {
            tool_mapping.insert(
                cap.id.clone(),
                ToolMapping {
                    mcp_name: cap.id.clone(),
                    a2a_skill_id: cap.id.clone(),
                    description: cap.description.clone(),
                    input_transform: None,
                },
            );
        }

        // Create A2A-compatible info
        let info = AgentInfo::new(
            format!("a2a-bridge-{}", source_info.id),
            source_info.name.clone(),
        )
        .with_protocol(Protocol::A2a)
        .with_description(format!(
            "A2A bridge to MCP agent: {}",
            source_info
                .description
                .as_deref()
                .unwrap_or(&source_info.name)
        ));

        // Copy capabilities with A2A tag
        let mut info = info;
        for cap in &source_info.capabilities {
            info = info.with_capability(
                Capability::new(&cap.id, &cap.name)
                    .with_tag("mcp")
                    .with_tag("bridged"),
            );
        }

        if source_info.supports_streaming {
            info = info.with_streaming();
        }

        Self {
            info,
            mcp_agent,
            tool_mapping,
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Set custom name for the A2A agent.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.info = AgentInfo::new(&self.info.id, name)
            .with_protocol(Protocol::A2a)
            .with_description(
                self.info
                    .description
                    .clone()
                    .unwrap_or_else(|| "MCP to A2A bridge".to_string()),
            );
        // Re-add capabilities
        for cap in self.mcp_agent.capabilities() {
            self.info = self.info.with_capability(cap.clone());
        }
        self
    }

    /// Set description for the A2A agent.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.info.description = Some(desc.into());
        self
    }

    /// Add or override a tool mapping.
    pub fn with_tool_mapping(mut self, mapping: ToolMapping) -> Self {
        self.tool_mapping.insert(mapping.mcp_name.clone(), mapping);
        self
    }

    /// Get the underlying MCP agent.
    pub fn mcp_agent(&self) -> &dyn UnifiedAgent {
        self.mcp_agent.as_ref()
    }

    /// Get tool mappings.
    pub fn tool_mappings(&self) -> &HashMap<String, ToolMapping> {
        &self.tool_mapping
    }
}

#[cfg(feature = "mcp")]
#[async_trait]
impl UnifiedAgent for McpToA2aBridge {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        debug!(
            bridge = %self.info.id,
            "Forwarding message from A2A to MCP"
        );

        // Forward to MCP agent
        let mut result = self.mcp_agent.send_message(message).await?;

        // Add bridge metadata
        result
            .metadata
            .insert("bridged_from".to_string(), serde_json::json!("mcp"));
        result
            .metadata
            .insert("bridge_id".to_string(), serde_json::json!(self.info.id));

        // Store task
        self.tasks
            .write()
            .await
            .insert(result.id.clone(), result.clone());

        Ok(result)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        self.mcp_agent.send_message_to_task(task_id, message).await
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        self.mcp_agent.send_message_streaming(message).await
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        // Try local cache first
        if let Some(task) = self.tasks.read().await.get(task_id) {
            return Ok(task.clone());
        }
        self.mcp_agent.get_task(task_id).await
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.mcp_agent.cancel_task(task_id).await
    }
}

// ============================================================================
// A2aToMcpBridge - Expose A2A agents as MCP tools
// ============================================================================

/// Bridge that exposes A2A agent skills as MCP-compatible tools.
///
/// This allows A2A agents to be used by MCP clients like Claude Desktop.
/// Each A2A skill becomes an MCP tool.
///
/// # Example
/// ```rust,ignore
/// use skreaver_agent::{A2aAgentAdapter, A2aToMcpBridge};
///
/// // Connect to an A2A agent
/// let a2a_agent = A2aAgentAdapter::connect("https://agent.example.com").await?;
///
/// // Create bridge to expose as MCP tools
/// let bridge = A2aToMcpBridge::new(Arc::new(a2a_agent));
///
/// // Get tools to register with MCP server
/// let tools = bridge.as_mcp_tools();
/// ```
#[cfg(feature = "a2a")]
pub struct A2aToMcpBridge {
    info: AgentInfo,
    a2a_agent: Arc<dyn UnifiedAgent>,
    skill_to_tool: HashMap<String, SkillToToolMapping>,
    tasks: tokio::sync::RwLock<HashMap<String, UnifiedTask>>,
}

/// Mapping from A2A skill to MCP tool.
#[cfg(feature = "a2a")]
#[derive(Debug, Clone)]
pub struct SkillToToolMapping {
    /// A2A skill ID
    pub skill_id: String,
    /// MCP tool name
    pub tool_name: String,
    /// Tool description
    pub description: String,
    /// JSON Schema for input
    pub input_schema: Option<serde_json::Value>,
}

#[cfg(feature = "a2a")]
impl A2aToMcpBridge {
    /// Create a new A2A to MCP bridge.
    pub fn new(a2a_agent: Arc<dyn UnifiedAgent>) -> Self {
        let source_info = a2a_agent.info();

        // Build skill-to-tool mappings
        let mut skill_to_tool = HashMap::new();
        for cap in &source_info.capabilities {
            skill_to_tool.insert(
                cap.id.clone(),
                SkillToToolMapping {
                    skill_id: cap.id.clone(),
                    tool_name: sanitize_tool_name(&cap.id),
                    description: cap.description.clone().unwrap_or_else(|| cap.name.clone()),
                    input_schema: cap.input_schema.clone(),
                },
            );
        }

        // Create MCP-compatible info
        let info = AgentInfo::new(
            format!("mcp-bridge-{}", source_info.id),
            source_info.name.clone(),
        )
        .with_protocol(Protocol::Mcp)
        .with_description(format!(
            "MCP bridge to A2A agent: {}",
            source_info
                .description
                .as_deref()
                .unwrap_or(&source_info.name)
        ));

        // Copy capabilities with MCP tag
        let mut info = info;
        for cap in &source_info.capabilities {
            info = info.with_capability(
                Capability::new(&cap.id, &cap.name)
                    .with_tag("a2a")
                    .with_tag("bridged"),
            );
        }

        Self {
            info,
            a2a_agent,
            skill_to_tool,
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Set custom name for the bridged agent.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.info = AgentInfo::new(&self.info.id, name)
            .with_protocol(Protocol::Mcp)
            .with_description(self.info.description.clone().unwrap_or_default());
        for cap in self.a2a_agent.capabilities() {
            self.info = self.info.with_capability(cap.clone());
        }
        self
    }

    /// Add or override a skill-to-tool mapping.
    pub fn with_skill_mapping(mut self, mapping: SkillToToolMapping) -> Self {
        self.skill_to_tool.insert(mapping.skill_id.clone(), mapping);
        self
    }

    /// Get the underlying A2A agent.
    pub fn a2a_agent(&self) -> &dyn UnifiedAgent {
        self.a2a_agent.as_ref()
    }

    /// Get skill-to-tool mappings.
    pub fn skill_mappings(&self) -> &HashMap<String, SkillToToolMapping> {
        &self.skill_to_tool
    }

    /// Convert an MCP tool call to an A2A message.
    ///
    /// This is useful when you need to invoke a specific A2A skill
    /// by constructing a tool call message.
    pub fn tool_call_to_message(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> AgentResult<UnifiedMessage> {
        // Find the mapping
        let mapping = self
            .skill_to_tool
            .values()
            .find(|m| m.tool_name == tool_name)
            .ok_or_else(|| AgentError::CapabilityNotFound(tool_name.to_string()))?;

        // Create a message with tool call
        let mut msg = UnifiedMessage::user(format!("Execute skill: {}", mapping.skill_id));
        msg.content.push(ContentPart::ToolCall {
            id: uuid::Uuid::new_v4().to_string(),
            name: mapping.skill_id.clone(),
            arguments,
        });

        Ok(msg)
    }
}

#[cfg(feature = "a2a")]
#[async_trait]
impl UnifiedAgent for A2aToMcpBridge {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        debug!(
            bridge = %self.info.id,
            "Forwarding message from MCP to A2A"
        );

        // Forward to A2A agent
        let mut result = self.a2a_agent.send_message(message).await?;

        // Add bridge metadata
        result
            .metadata
            .insert("bridged_from".to_string(), serde_json::json!("a2a"));
        result
            .metadata
            .insert("bridge_id".to_string(), serde_json::json!(self.info.id));

        // Store task
        self.tasks
            .write()
            .await
            .insert(result.id.clone(), result.clone());

        Ok(result)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        self.a2a_agent.send_message_to_task(task_id, message).await
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        self.a2a_agent.send_message_streaming(message).await
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        if let Some(task) = self.tasks.read().await.get(task_id) {
            return Ok(task.clone());
        }
        self.a2a_agent.get_task(task_id).await
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.a2a_agent.cancel_task(task_id).await
    }
}

// ============================================================================
// ProtocolGateway - Unified gateway for protocol translation
// ============================================================================

/// A unified gateway that manages protocol translation between MCP and A2A.
///
/// The gateway maintains registries of both MCP and A2A agents and provides:
/// - Automatic protocol detection and routing
/// - Bidirectional protocol translation
/// - Unified agent discovery
///
/// # Example
/// ```rust,ignore
/// use skreaver_agent::ProtocolGateway;
///
/// let mut gateway = ProtocolGateway::new();
///
/// // Register MCP agents (will be exposed as A2A)
/// gateway.register_mcp_agent(mcp_fs_agent);
///
/// // Register A2A agents (will be exposed as MCP tools)
/// gateway.register_a2a_agent(a2a_search_agent);
///
/// // Get all agents with unified interface
/// let all_agents = gateway.all_agents();
///
/// // Find by protocol
/// let a2a_available = gateway.agents_for_protocol(Protocol::A2a);
/// ```
#[cfg(all(feature = "mcp", feature = "a2a"))]
pub struct ProtocolGateway {
    /// Original MCP agents
    mcp_agents: Vec<Arc<dyn UnifiedAgent>>,
    /// Original A2A agents
    a2a_agents: Vec<Arc<dyn UnifiedAgent>>,
    /// MCP agents bridged to A2A
    mcp_to_a2a_bridges: Vec<Arc<McpToA2aBridge>>,
    /// A2A agents bridged to MCP
    a2a_to_mcp_bridges: Vec<Arc<A2aToMcpBridge>>,
    /// Protocol preference for routing
    default_protocol: Protocol,
}

#[cfg(all(feature = "mcp", feature = "a2a"))]
impl Default for ProtocolGateway {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(feature = "mcp", feature = "a2a"))]
impl ProtocolGateway {
    /// Create a new protocol gateway.
    pub fn new() -> Self {
        Self {
            mcp_agents: Vec::new(),
            a2a_agents: Vec::new(),
            mcp_to_a2a_bridges: Vec::new(),
            a2a_to_mcp_bridges: Vec::new(),
            default_protocol: Protocol::A2a,
        }
    }

    /// Set the default protocol for routing.
    pub fn with_default_protocol(mut self, protocol: Protocol) -> Self {
        self.default_protocol = protocol;
        self
    }

    /// Register an MCP agent.
    ///
    /// This also creates an A2A bridge for the agent.
    pub fn register_mcp_agent(&mut self, agent: Arc<dyn UnifiedAgent>) {
        info!(
            agent_id = %agent.info().id,
            "Registering MCP agent in gateway"
        );

        // Create A2A bridge
        let bridge = Arc::new(McpToA2aBridge::new(Arc::clone(&agent)));
        self.mcp_to_a2a_bridges.push(bridge);

        self.mcp_agents.push(agent);
    }

    /// Register an A2A agent.
    ///
    /// This also creates an MCP bridge for the agent.
    pub fn register_a2a_agent(&mut self, agent: Arc<dyn UnifiedAgent>) {
        info!(
            agent_id = %agent.info().id,
            "Registering A2A agent in gateway"
        );

        // Create MCP bridge
        let bridge = Arc::new(A2aToMcpBridge::new(Arc::clone(&agent)));
        self.a2a_to_mcp_bridges.push(bridge);

        self.a2a_agents.push(agent);
    }

    /// Get all registered agents (originals only).
    pub fn all_agents(&self) -> Vec<Arc<dyn UnifiedAgent>> {
        let mut agents: Vec<Arc<dyn UnifiedAgent>> = Vec::new();
        agents.extend(self.mcp_agents.iter().cloned());
        agents.extend(self.a2a_agents.iter().cloned());
        agents
    }

    /// Get agents available for a specific protocol.
    ///
    /// This includes both native agents and bridged agents.
    pub fn agents_for_protocol(&self, protocol: Protocol) -> Vec<Arc<dyn UnifiedAgent>> {
        match protocol {
            Protocol::Mcp => {
                let mut agents: Vec<Arc<dyn UnifiedAgent>> = Vec::new();
                // Native MCP agents
                agents.extend(self.mcp_agents.iter().cloned());
                // A2A agents bridged to MCP
                for bridge in &self.a2a_to_mcp_bridges {
                    agents.push(Arc::clone(bridge) as Arc<dyn UnifiedAgent>);
                }
                agents
            }
            Protocol::A2a => {
                let mut agents: Vec<Arc<dyn UnifiedAgent>> = Vec::new();
                // Native A2A agents
                agents.extend(self.a2a_agents.iter().cloned());
                // MCP agents bridged to A2A
                for bridge in &self.mcp_to_a2a_bridges {
                    agents.push(Arc::clone(bridge) as Arc<dyn UnifiedAgent>);
                }
                agents
            }
        }
    }

    /// Find an agent by ID (searches both native and bridged).
    pub fn find_agent(&self, id: &str) -> Option<Arc<dyn UnifiedAgent>> {
        // Check native agents
        for agent in &self.mcp_agents {
            if agent.info().id == id {
                return Some(Arc::clone(agent));
            }
        }
        for agent in &self.a2a_agents {
            if agent.info().id == id {
                return Some(Arc::clone(agent));
            }
        }

        // Check bridges
        for bridge in &self.mcp_to_a2a_bridges {
            if bridge.info().id == id {
                return Some(Arc::clone(bridge) as Arc<dyn UnifiedAgent>);
            }
        }
        for bridge in &self.a2a_to_mcp_bridges {
            if bridge.info().id == id {
                return Some(Arc::clone(bridge) as Arc<dyn UnifiedAgent>);
            }
        }

        None
    }

    /// Find agents by capability.
    pub fn find_by_capability(&self, capability_id: &str) -> Vec<Arc<dyn UnifiedAgent>> {
        let mut results: Vec<Arc<dyn UnifiedAgent>> = Vec::new();

        for agent in self.all_agents() {
            if agent.capabilities().iter().any(|c| c.id == capability_id) {
                results.push(agent);
            }
        }

        results
    }

    /// Get the MCP to A2A bridges.
    pub fn mcp_bridges(&self) -> &[Arc<McpToA2aBridge>] {
        &self.mcp_to_a2a_bridges
    }

    /// Get the A2A to MCP bridges.
    pub fn a2a_bridges(&self) -> &[Arc<A2aToMcpBridge>] {
        &self.a2a_to_mcp_bridges
    }

    /// Route a message to the appropriate agent based on protocol preference.
    pub async fn route_message(
        &self,
        message: UnifiedMessage,
        target_agent_id: Option<&str>,
    ) -> AgentResult<UnifiedTask> {
        let agent = if let Some(id) = target_agent_id {
            self.find_agent(id)
                .ok_or_else(|| AgentError::Internal(format!("Agent not found: {}", id)))?
        } else {
            // Use first available agent for default protocol
            self.agents_for_protocol(self.default_protocol)
                .first()
                .cloned()
                .ok_or_else(|| AgentError::Internal("No agents available".to_string()))?
        };

        agent.send_message(message).await
    }

    /// Get count of all registered agents (including bridges).
    pub fn total_agent_count(&self) -> usize {
        self.mcp_agents.len()
            + self.a2a_agents.len()
            + self.mcp_to_a2a_bridges.len()
            + self.a2a_to_mcp_bridges.len()
    }
}

// ============================================================================
// Protocol Translation Helpers
// ============================================================================

/// Sanitize a name to be MCP-tool compatible.
///
/// MCP tools have naming restrictions - this ensures compatibility.
#[cfg(feature = "a2a")]
fn sanitize_tool_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Convert MCP tool result to A2A message parts.
#[cfg(all(feature = "mcp", feature = "a2a"))]
pub fn mcp_result_to_a2a_parts(result: &serde_json::Value) -> Vec<ContentPart> {
    match result {
        serde_json::Value::String(s) => vec![ContentPart::Text { text: s.clone() }],
        serde_json::Value::Null => vec![],
        other => vec![ContentPart::Text {
            text: serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
        }],
    }
}

/// Convert A2A message parts to MCP-compatible result.
#[cfg(all(feature = "mcp", feature = "a2a"))]
pub fn a2a_parts_to_mcp_result(parts: &[ContentPart]) -> serde_json::Value {
    if parts.is_empty() {
        return serde_json::Value::Null;
    }

    if parts.len() == 1 {
        return match &parts[0] {
            ContentPart::Text { text } => {
                // Try to parse as JSON
                serde_json::from_str(text).unwrap_or_else(|_| serde_json::json!(text))
            }
            ContentPart::Data {
                data, mime_type, ..
            } => {
                serde_json::json!({
                    "type": "data",
                    "data": data,
                    "mime_type": mime_type
                })
            }
            ContentPart::File {
                uri,
                mime_type,
                name,
            } => {
                serde_json::json!({
                    "type": "file",
                    "uri": uri,
                    "mime_type": mime_type,
                    "name": name
                })
            }
            _ => serde_json::Value::Null,
        };
    }

    // Multiple parts - return as array
    serde_json::Value::Array(
        parts
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(serde_json::json!(text)),
                ContentPart::Data {
                    data, mime_type, ..
                } => Some(serde_json::json!({
                    "type": "data",
                    "data": data,
                    "mime_type": mime_type
                })),
                ContentPart::File {
                    uri,
                    mime_type,
                    name,
                } => Some(serde_json::json!({
                    "type": "file",
                    "uri": uri,
                    "mime_type": mime_type,
                    "name": name
                })),
                _ => None,
            })
            .collect(),
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[cfg(feature = "a2a")]
    #[test]
    fn test_sanitize_tool_name() {
        assert_eq!(sanitize_tool_name("simple_tool"), "simple_tool");
        assert_eq!(sanitize_tool_name("tool-name"), "tool-name");
        assert_eq!(sanitize_tool_name("tool with spaces"), "tool_with_spaces");
        assert_eq!(sanitize_tool_name("tool.with.dots"), "tool_with_dots");
        assert_eq!(sanitize_tool_name("tool/with/slashes"), "tool_with_slashes");
    }

    #[cfg(all(feature = "mcp", feature = "a2a"))]
    #[test]
    fn test_mcp_result_to_a2a_parts() {
        let result = serde_json::json!("Hello, world!");
        let parts = mcp_result_to_a2a_parts(&result);
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], ContentPart::Text { text } if text == "Hello, world!"));
    }

    #[cfg(all(feature = "mcp", feature = "a2a"))]
    #[test]
    fn test_a2a_parts_to_mcp_result() {
        let parts = vec![ContentPart::Text {
            text: "Test result".to_string(),
        }];
        let result = a2a_parts_to_mcp_result(&parts);
        assert_eq!(result, serde_json::json!("Test result"));
    }

    #[cfg(all(feature = "mcp", feature = "a2a"))]
    #[test]
    fn test_protocol_gateway_creation() {
        let gateway = ProtocolGateway::new();
        assert_eq!(gateway.total_agent_count(), 0);
        assert!(gateway.all_agents().is_empty());
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn test_tool_mapping() {
        let mapping = ToolMapping {
            mcp_name: "read_file".to_string(),
            a2a_skill_id: "file_read".to_string(),
            description: Some("Read a file".to_string()),
            input_transform: None,
        };
        assert_eq!(mapping.mcp_name, "read_file");
        assert_eq!(mapping.a2a_skill_id, "file_read");
    }

    #[cfg(feature = "a2a")]
    #[test]
    fn test_skill_to_tool_mapping() {
        let mapping = SkillToToolMapping {
            skill_id: "web_search".to_string(),
            tool_name: "web_search".to_string(),
            description: "Search the web".to_string(),
            input_schema: None,
        };
        assert_eq!(mapping.skill_id, "web_search");
    }
}
