//! # Protocol Bridge Demo - MCP + A2A Interoperability
//!
//! This example demonstrates how to bridge between MCP and A2A protocols,
//! enabling interoperability between different types of agents.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Protocol Gateway                             │
//! │                                                                   │
//! │   ┌─────────────────┐         ┌─────────────────┐               │
//! │   │   MCP Agents    │◄───────►│ McpToA2aBridge  │◄──► A2A API   │
//! │   │ (Tool Servers)  │         │ (Exposes as A2A)│               │
//! │   └─────────────────┘         └─────────────────┘               │
//! │                                                                   │
//! │   ┌─────────────────┐         ┌─────────────────┐               │
//! │   │   A2A Agents    │◄───────►│ A2aToMcpBridge  │◄──► MCP API   │
//! │   │ (Remote Agents) │         │ (Exposes as MCP)│               │
//! │   └─────────────────┘         └─────────────────┘               │
//! │                                                                   │
//! │                 Unified Agent Discovery                          │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Use Cases
//!
//! 1. **MCP to A2A**: Expose MCP tool servers (like filesystem, database) to A2A clients
//! 2. **A2A to MCP**: Use A2A agents as MCP tools in Claude Desktop
//! 3. **Unified Gateway**: Route requests between protocols transparently
//!
//! ## Running
//!
//! ```bash
//! cargo run --example protocol_bridge_demo
//! ```
//!
//! Note: This example demonstrates the API without requiring actual MCP servers.

use async_trait::async_trait;
use skreaver_agent::{
    AgentInfo, AgentRegistry, Capability, ContentPart, MessageRole, Protocol, StreamEvent,
    TaskStatus, UnifiedAgent, UnifiedMessage, UnifiedTask,
};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

// =============================================================================
// Mock Agents for Demonstration
// =============================================================================

/// A mock MCP-style agent that provides tool capabilities
struct MockMcpToolAgent {
    info: AgentInfo,
    tasks: tokio::sync::RwLock<HashMap<String, UnifiedTask>>,
}

impl MockMcpToolAgent {
    fn new() -> Self {
        let info = AgentInfo::new("mock-mcp-tools", "Mock MCP Tool Server")
            .with_protocol(Protocol::Mcp)
            .with_description("A simulated MCP server with filesystem-like tools")
            .with_capability(
                Capability::new("read_file", "Read File")
                    .with_description("Read contents of a file")
                    .with_tag("mcp")
                    .with_tag("filesystem"),
            )
            .with_capability(
                Capability::new("write_file", "Write File")
                    .with_description("Write contents to a file")
                    .with_tag("mcp")
                    .with_tag("filesystem"),
            )
            .with_capability(
                Capability::new("list_dir", "List Directory")
                    .with_description("List contents of a directory")
                    .with_tag("mcp")
                    .with_tag("filesystem"),
            );

        Self {
            info,
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    fn handle_tool_call(&self, tool_name: &str, args: &serde_json::Value) -> String {
        match tool_name {
            "read_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/example.txt");
                format!("File contents of '{}': Hello from MCP tool!", path)
            }
            "write_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("/output.txt");
                format!("Successfully wrote to '{}'", path)
            }
            "list_dir" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                format!("Contents of '{}': [example.txt, output.txt, data/]", path)
            }
            _ => format!("Unknown tool: {}", tool_name),
        }
    }
}

#[async_trait]
impl UnifiedAgent for MockMcpToolAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(
        &self,
        message: UnifiedMessage,
    ) -> skreaver_agent::AgentResult<UnifiedTask> {
        let mut task = UnifiedTask::new_with_uuid();
        task.add_message(message.clone());

        // Process tool calls
        for part in &message.content {
            if let ContentPart::ToolCall {
                id,
                name,
                arguments,
            } = part
            {
                let result = self.handle_tool_call(name, arguments);
                let mut response = UnifiedMessage::agent(&result);
                response.content = vec![ContentPart::ToolResult {
                    id: id.clone(),
                    result: serde_json::json!({ "output": result }),
                    is_error: Some(false),
                }];
                task.add_message(response);
            }
        }

        // If no tool calls, just echo
        if task.messages.len() == 1 {
            let text = message
                .content
                .first()
                .and_then(|p| {
                    if let ContentPart::Text { text } = p {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or("(empty)");
            task.add_message(UnifiedMessage::agent(format!(
                "MCP Agent received: {}",
                text
            )));
        }

        task.set_status(TaskStatus::Completed);
        self.tasks
            .write()
            .await
            .insert(task.id.clone(), task.clone());
        Ok(task)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> skreaver_agent::AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| skreaver_agent::AgentError::TaskNotFound(task_id.to_string()))?;
        task.add_message(message);
        Ok(task.clone())
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> skreaver_agent::AgentResult<
        Pin<Box<dyn futures::Stream<Item = skreaver_agent::AgentResult<StreamEvent>> + Send>>,
    > {
        let task = self.send_message(message).await?;
        let task_id = task.id.clone();

        let stream = async_stream::stream! {
            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Working,
                message: None,
            });

            for msg in &task.messages {
                yield Ok(StreamEvent::MessageAdded {
                    task_id: task_id.clone(),
                    message: msg.clone(),
                });
            }

            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Completed,
                message: None,
            });
        };

        Ok(Box::pin(stream))
    }

    async fn get_task(&self, task_id: &str) -> skreaver_agent::AgentResult<UnifiedTask> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .cloned()
            .ok_or_else(|| skreaver_agent::AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn cancel_task(&self, task_id: &str) -> skreaver_agent::AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| skreaver_agent::AgentError::TaskNotFound(task_id.to_string()))?;
        task.set_status(TaskStatus::Cancelled);
        Ok(task.clone())
    }
}

/// A mock A2A-style agent that provides conversational capabilities
struct MockA2aConversationalAgent {
    info: AgentInfo,
    tasks: tokio::sync::RwLock<HashMap<String, UnifiedTask>>,
}

impl MockA2aConversationalAgent {
    fn new() -> Self {
        let info = AgentInfo::new("mock-a2a-assistant", "Mock A2A Assistant")
            .with_protocol(Protocol::A2a)
            .with_description("A simulated A2A conversational agent")
            .with_streaming()
            .with_capability(
                Capability::new("chat", "Chat")
                    .with_description("General conversation and Q&A")
                    .with_tag("a2a")
                    .with_tag("conversational"),
            )
            .with_capability(
                Capability::new("summarize", "Summarize")
                    .with_description("Summarize text content")
                    .with_tag("a2a")
                    .with_tag("text-processing"),
            );

        Self {
            info,
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    fn process_input(&self, input: &str) -> String {
        let input_lower = input.to_lowercase();

        if input_lower.contains("summarize") {
            "Summary: This is a summarized version of the input text. Key points extracted."
                .to_string()
        } else if input_lower.contains("hello") || input_lower.contains("hi") {
            "Hello! I'm an A2A conversational agent. How can I help you today?".to_string()
        } else if input_lower.contains("help") {
            "I can help with:\n- General conversation (just chat with me)\n- Text summarization (say 'summarize: <text>')\n- Answer questions about various topics".to_string()
        } else {
            format!(
                "I understand you said: '{}'. As an A2A agent, I can engage in conversation and help with text processing tasks.",
                input
            )
        }
    }
}

#[async_trait]
impl UnifiedAgent for MockA2aConversationalAgent {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(
        &self,
        message: UnifiedMessage,
    ) -> skreaver_agent::AgentResult<UnifiedTask> {
        let mut task = UnifiedTask::new_with_uuid();
        task.add_message(message.clone());

        let text = message
            .content
            .first()
            .and_then(|p| {
                if let ContentPart::Text { text } = p {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .unwrap_or("");

        let response = self.process_input(text);
        task.add_message(UnifiedMessage::agent(&response));
        task.set_status(TaskStatus::Completed);

        self.tasks
            .write()
            .await
            .insert(task.id.clone(), task.clone());
        Ok(task)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> skreaver_agent::AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| skreaver_agent::AgentError::TaskNotFound(task_id.to_string()))?;

        let text = message
            .content
            .first()
            .and_then(|p| {
                if let ContentPart::Text { text } = p {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        task.add_message(message);
        let response = self.process_input(&text);
        task.add_message(UnifiedMessage::agent(&response));

        Ok(task.clone())
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> skreaver_agent::AgentResult<
        Pin<Box<dyn futures::Stream<Item = skreaver_agent::AgentResult<StreamEvent>> + Send>>,
    > {
        let task = self.send_message(message).await?;
        let task_id = task.id.clone();

        let stream = async_stream::stream! {
            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Working,
                message: None,
            });

            // Simulate streaming by yielding messages one at a time
            for msg in &task.messages {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                yield Ok(StreamEvent::MessageAdded {
                    task_id: task_id.clone(),
                    message: msg.clone(),
                });
            }

            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Completed,
                message: None,
            });
        };

        Ok(Box::pin(stream))
    }

    async fn get_task(&self, task_id: &str) -> skreaver_agent::AgentResult<UnifiedTask> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .cloned()
            .ok_or_else(|| skreaver_agent::AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn cancel_task(&self, task_id: &str) -> skreaver_agent::AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| skreaver_agent::AgentError::TaskNotFound(task_id.to_string()))?;
        task.set_status(TaskStatus::Cancelled);
        Ok(task.clone())
    }
}

// =============================================================================
// Demo Functions
// =============================================================================

/// Demonstrate the Agent Registry for unified discovery
async fn demo_agent_registry() {
    println!("=== Agent Registry Demo ===");
    println!();

    let mut registry = AgentRegistry::new();

    // Register agents
    let mcp_agent: Arc<dyn UnifiedAgent> = Arc::new(MockMcpToolAgent::new());
    let a2a_agent: Arc<dyn UnifiedAgent> = Arc::new(MockA2aConversationalAgent::new());

    registry.register(mcp_agent);
    registry.register(a2a_agent);

    // List all agents
    println!("Registered Agents:");
    for agent in registry.list() {
        let info = agent.info();
        println!("  - {} ({:?})", info.name, info.protocols);
        if let Some(desc) = &info.description {
            println!("    {}", desc);
        }
        println!("    Capabilities:");
        for cap in &info.capabilities {
            println!("      * {} - {}", cap.id, cap.name);
        }
        println!();
    }

    // Find by protocol
    println!("MCP Protocol Agents:");
    for agent in registry.find_by_protocol(Protocol::Mcp) {
        println!("  - {}", agent.info().name);
    }
    println!();

    println!("A2A Protocol Agents:");
    for agent in registry.find_by_protocol(Protocol::A2a) {
        println!("  - {}", agent.info().name);
    }
    println!();

    // Find by capability
    println!("Agents with 'filesystem' capability:");
    for agent in registry.find_by_capability("read_file") {
        println!("  - {}", agent.info().name);
    }
    println!();

    println!("Agents with 'chat' capability:");
    for agent in registry.find_by_capability("chat") {
        println!("  - {}", agent.info().name);
    }
    println!();
}

/// Demonstrate cross-protocol message routing
async fn demo_message_routing() {
    println!("=== Cross-Protocol Message Routing Demo ===");
    println!();

    let mut registry = AgentRegistry::new();

    let mcp_agent: Arc<dyn UnifiedAgent> = Arc::new(MockMcpToolAgent::new());
    let a2a_agent: Arc<dyn UnifiedAgent> = Arc::new(MockA2aConversationalAgent::new());

    registry.register(mcp_agent);
    registry.register(a2a_agent);

    // Send message to MCP agent
    println!("Sending tool call to MCP agent...");
    if let Some(agent) = registry.find("mock-mcp-tools") {
        let mut message = UnifiedMessage::user("Call a tool");
        message.content = vec![ContentPart::ToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({ "path": "/documents/readme.md" }),
        }];

        match agent.send_message(message).await {
            Ok(task) => {
                println!("  Task ID: {}", task.id);
                println!("  Status: {:?}", task.status);
                for msg in task
                    .messages
                    .iter()
                    .filter(|m| m.role == MessageRole::Agent)
                {
                    for part in &msg.content {
                        match part {
                            ContentPart::ToolResult { result, .. } => {
                                println!("  Result: {}", result);
                            }
                            ContentPart::Text { text } => {
                                println!("  Response: {}", text);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(e) => println!("  Error: {}", e),
        }
    }
    println!();

    // Send message to A2A agent
    println!("Sending conversation message to A2A agent...");
    if let Some(agent) = registry.find("mock-a2a-assistant") {
        let message = UnifiedMessage::user("Hello! Can you help me?");

        match agent.send_message(message).await {
            Ok(task) => {
                println!("  Task ID: {}", task.id);
                println!("  Status: {:?}", task.status);
                for msg in task
                    .messages
                    .iter()
                    .filter(|m| m.role == MessageRole::Agent)
                {
                    for part in &msg.content {
                        if let ContentPart::Text { text } = part {
                            println!("  Response: {}", text);
                        }
                    }
                }
            }
            Err(e) => println!("  Error: {}", e),
        }
    }
    println!();
}

/// Demonstrate the ProxyAgent pattern
async fn demo_proxy_agent() {
    println!("=== Proxy Agent Pattern Demo ===");
    println!();

    let inner_agent: Arc<dyn UnifiedAgent> = Arc::new(MockA2aConversationalAgent::new());

    // ProxyAgent wraps another agent, useful for logging, metrics, or access control
    let proxy = skreaver_agent::ProxyAgent::new("proxied-assistant", inner_agent);

    println!("Proxy agent info:");
    println!("  ID: {}", proxy.info().id);
    println!("  Name: {}", proxy.info().name);
    println!("  Target: {}", proxy.target().info().name);
    println!();

    println!("Sending message through proxy...");
    let message = UnifiedMessage::user("Hello from the proxy demo!");

    match proxy.send_message(message).await {
        Ok(task) => {
            println!("  Task ID: {}", task.id);
            for msg in &task.messages {
                for part in &msg.content {
                    if let ContentPart::Text { text } = part {
                        println!("  Message: {}", text);
                    }
                }
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
    println!();
}

/// Demonstrate FanOut to multiple agents
async fn demo_fanout_agent() {
    println!("=== FanOut Agent Pattern Demo ===");
    println!();

    let agent1: Arc<dyn UnifiedAgent> = Arc::new(MockMcpToolAgent::new());
    let agent2: Arc<dyn UnifiedAgent> = Arc::new(MockA2aConversationalAgent::new());

    let mut fanout = skreaver_agent::FanOutAgent::new("multi-agent", "Multi-Agent FanOut");
    fanout.add_target(agent1);
    fanout.add_target(agent2);

    println!("Sending message to multiple agents simultaneously...");
    let message = UnifiedMessage::user("Process this message");

    match fanout.send_message(message).await {
        Ok(task) => {
            println!("  Combined task created: {}", task.id);
            println!("  Total responses: {} messages", task.messages.len());

            // Show aggregated results
            for (i, msg) in task
                .messages
                .iter()
                .filter(|m| m.role == MessageRole::Agent)
                .enumerate()
            {
                for part in &msg.content {
                    if let ContentPart::Text { text } = part {
                        println!("  Agent {} response: {}", i + 1, text);
                    }
                }
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
    println!();
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry().with(fmt::layer()).init();

    println!("==============================================");
    println!("   Protocol Bridge Demo - MCP + A2A");
    println!("==============================================");
    println!();
    println!("This demo shows how to bridge between MCP and A2A protocols");
    println!("using the Skreaver unified agent interface.");
    println!();

    // Run demos
    demo_agent_registry().await;
    println!();

    demo_message_routing().await;
    println!();

    demo_proxy_agent().await;
    println!();

    demo_fanout_agent().await;
    println!();

    println!("==============================================");
    println!("   Demo Complete!");
    println!("==============================================");
    println!();
    println!("Key Concepts Demonstrated:");
    println!("  1. AgentRegistry - Unified discovery across protocols");
    println!("  2. Protocol-agnostic messaging via UnifiedMessage");
    println!("  3. ProxyAgent - Transform requests/responses");
    println!("  4. FanOutAgent - Broadcast to multiple agents");
    println!();
    println!("For production use with real MCP/A2A agents:");
    println!("  - McpAgentAdapter::connect() - Connect to MCP servers");
    println!("  - A2aAgentAdapter::connect() - Connect to A2A endpoints");
    println!("  - ProtocolGateway - Full bidirectional bridging");
    println!();

    Ok(())
}
