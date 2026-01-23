# skreaver-agent

Unified agent interface for the Skreaver platform, providing protocol-agnostic abstractions for working with AI agents across MCP and A2A protocols.

## Overview

This crate provides a unified interface that abstracts away protocol differences, allowing you to:

- Work with agents regardless of their underlying protocol (MCP or A2A)
- Build complex multi-agent orchestrations
- Bridge between protocols seamlessly

## Features

| Feature | Description |
|---------|-------------|
| `mcp` | MCP protocol adapter |
| `a2a` | A2A protocol adapter |
| `full` | Both protocols enabled |

## Quick Start

### Using MCP Agents

```rust
use skreaver_agent::{McpAgentAdapter, UnifiedAgent, UnifiedMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to an MCP server
    let agent = McpAgentAdapter::connect("npx @anthropic/mcp-server-fs").await?;

    // Use the unified interface
    let task = agent.send_message(UnifiedMessage::user("List files")).await?;
    println!("Result: {:?}", task.status);

    Ok(())
}
```

### Using A2A Agents

```rust
use skreaver_agent::{A2aAgentAdapter, UnifiedAgent, UnifiedMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to an A2A agent
    let agent = A2aAgentAdapter::connect("http://agent.example.com").await?;

    // Same unified interface
    let task = agent.send_message(UnifiedMessage::user("Hello!")).await?;
    println!("Response: {:?}", task.messages);

    Ok(())
}
```

## Core Abstractions

### UnifiedAgent Trait

The central abstraction that all agents implement:

```rust
#[async_trait]
pub trait UnifiedAgent: Send + Sync {
    fn info(&self) -> &AgentInfo;
    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask>;
    async fn send_message_to_task(&self, task_id: &str, message: UnifiedMessage) -> AgentResult<UnifiedTask>;
    async fn send_message_streaming(&self, message: UnifiedMessage) -> AgentResult<Box<dyn Stream<...>>>;
    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask>;
    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask>;
}
```

### UnifiedMessage

Protocol-agnostic message type:

```rust
let msg = UnifiedMessage::user("Hello")
    .with_part(ContentPart::text("Additional context"));

// Tool calls work the same way
let tool_call = UnifiedMessage::user("Call a tool")
    .with_part(ContentPart::ToolCall {
        id: "call-1".into(),
        name: "read_file".into(),
        arguments: json!({ "path": "/tmp/file.txt" }),
    });
```

### UnifiedTask

Protocol-agnostic task tracking:

```rust
let task = agent.send_message(message).await?;

// Check status
match task.status {
    TaskStatus::Completed => println!("Done!"),
    TaskStatus::InputRequired => println!("Need more input"),
    TaskStatus::Failed => println!("Task failed"),
    TaskStatus::Rejected => println!("Task rejected"),
    _ => {}
}

// Access messages and artifacts
for msg in &task.messages {
    println!("Message: {:?}", msg.content);
}
```

## Agent Registry

Manage multiple agents with unified discovery:

```rust
use skreaver_agent::{AgentRegistry, Protocol};

let mut registry = AgentRegistry::new();

// Register agents
registry.register(mcp_agent.into());
registry.register(a2a_agent.into());

// Find by protocol
let mcp_agents = registry.find_by_protocol(Protocol::Mcp);

// Find by capability
let search_agents = registry.find_by_capability("search");

// Find by ID
if let Some(agent) = registry.find("my-agent") {
    agent.send_message(message).await?;
}
```

## Agent Discovery Service

For more advanced discovery with health checking and event notifications:

```rust
use skreaver_agent::{
    DiscoveryService, AgentRegistration, DiscoveryQuery,
    HealthStatus, Protocol
};

// Create discovery service
let discovery = DiscoveryService::new();

// Register an agent with metadata
let registration = AgentRegistration::new("search-agent", "Search Agent")
    .with_protocol(Protocol::A2a)
    .with_capability("web-search")
    .with_capability("document-search")
    .with_endpoint("https://search.example.com")
    .with_tag("production")
    .with_ttl(300); // 5 minute heartbeat TTL

let registration_id = discovery.register(registration).await?;

// Query for agents by capability
let search_agents = discovery.find_by_capability("web-search").await?;

// Query with multiple filters
let agents = discovery.query(
    DiscoveryQuery::new()
        .with_protocol(Protocol::A2a)
        .with_capability("search")
        .with_tag("production")
        .with_health_status(HealthStatus::Healthy)
).await?;

// Send heartbeats to keep registration alive
discovery.heartbeat(&registration_id).await?;

// Start background health checking and cleanup
let handle = Arc::new(discovery).start_background_tasks();

// Later, stop background tasks
handle.stop();
```

### Discovery Events

Subscribe to agent lifecycle events:

```rust
use skreaver_agent::{InMemoryDiscoveryProvider, DiscoveryEvent};

let provider = InMemoryDiscoveryProvider::new();
let mut events = provider.subscribe();

// In another task, listen for events
tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        match event {
            DiscoveryEvent::AgentRegistered { agent_id, name, .. } => {
                println!("New agent: {} ({})", name, agent_id);
            }
            DiscoveryEvent::AgentDeregistered { agent_id, reason, .. } => {
                println!("Agent left: {} ({:?})", agent_id, reason);
            }
            DiscoveryEvent::HealthStatusChanged { agent_id, new_status, .. } => {
                println!("Health changed: {} -> {}", agent_id, new_status);
            }
            _ => {}
        }
    }
});
```

## Orchestration Patterns

### Sequential Pipeline

Chain agents where output flows to the next:

```rust
use skreaver_agent::SequentialPipeline;

let pipeline = SequentialPipeline::new("analysis", "Analysis Pipeline")
    .add_stage(preprocessor)
    .add_stage(analyzer)
    .add_stage(summarizer);

let result = pipeline.send_message(UnifiedMessage::user("Analyze this")).await?;
```

### Parallel Agent

Run multiple agents concurrently:

```rust
use skreaver_agent::ParallelAgent;

let mut parallel = ParallelAgent::new("multi-search", "Multi Search");
parallel.add_agent(google_agent);
parallel.add_agent(bing_agent);

// Both run simultaneously
let result = parallel.send_message(UnifiedMessage::user("rust async")).await?;
```

### Router Agent

Route messages based on rules:

```rust
use skreaver_agent::{RouterAgent, RoutingRule};

let router = RouterAgent::new("router", "Task Router")
    .add_agent(code_agent)
    .add_agent(search_agent)
    .add_rule(RoutingRule::contains("code", "code_agent"))
    .add_rule(RoutingRule::capability_based("search", "search_agent"));
```

### Supervisor Agent

Coordinate complex workflows:

```rust
use skreaver_agent::SupervisorAgent;

// Capability-based supervisor
let supervisor = SupervisorAgent::with_capability_supervisor("coordinator", "Coordinator")
    .add_agent(analysis_agent)
    .add_agent(code_agent)
    .add_agent(search_agent);

// Routes to agent with matching capabilities
let result = supervisor.send_message(message).await?;
```

## Protocol Bridging

### MCP → A2A Bridge

Expose MCP tools as A2A skills:

```rust
use skreaver_agent::McpToA2aBridge;

let bridge = McpToA2aBridge::new(mcp_agent)
    .with_name("Filesystem Agent")
    .with_description("Access filesystem via A2A");

// Now usable as an A2A agent
```

### A2A → MCP Bridge

Expose A2A skills as MCP tools:

```rust
use skreaver_agent::A2aToMcpBridge;

let bridge = A2aToMcpBridge::new(a2a_agent);

// Each A2A skill becomes an MCP tool
for mapping in bridge.skill_mappings().values() {
    println!("Tool: {} -> Skill: {}", mapping.tool_name, mapping.skill_id);
}
```

### Protocol Gateway

Unified gateway for bidirectional bridging:

```rust
use skreaver_agent::ProtocolGateway;

let mut gateway = ProtocolGateway::new();

gateway.register_mcp_agent(mcp_agent1);
gateway.register_a2a_agent(a2a_agent1);

// Access agents by protocol
let a2a_available = gateway.agents_for_protocol(Protocol::A2a);

// Route to best agent
let result = gateway.route_message(message, None).await?;
```

## Type Conversions

### MCP ↔ A2A Mapping

| MCP | A2A | Notes |
|-----|-----|-------|
| Tool | Skill | 1:1 mapping |
| Tool Call | Message w/ ToolCall | Wrapped in task |
| Tool Result | Message w/ ToolResult | Added to history |
| Resource | Artifact | URI-based content |

### ContentPart Types

```rust
// Text content
ContentPart::text("Hello")

// Tool call
ContentPart::ToolCall { id, name, arguments }

// Tool result
ContentPart::ToolResult { id, result, is_error }

// Data (base64)
ContentPart::Data { data, mime_type, name }

// File reference
ContentPart::File { uri, mime_type, name }
```

## Error Handling

```rust
use skreaver_agent::{AgentError, AgentResult};

match agent.send_message(msg).await {
    Ok(task) => { /* success */ }
    Err(AgentError::TaskNotFound(id)) => {
        println!("Task {} not found", id);
    }
    Err(AgentError::CapabilityNotFound(cap)) => {
        println!("No agent has capability: {}", cap);
    }
    Err(AgentError::ConnectionError(msg)) => {
        println!("Connection failed: {}", msg);
    }
    Err(e) => println!("Error: {}", e),
}

// Errors include helpful methods
let err = AgentError::ConnectionError("timeout".into());
if err.is_retryable() {
    // Retry the operation
}
println!("Error code: {}", err.error_code()); // "CONNECTION_ERROR"
```

## Related Crates

- [`skreaver-a2a`](../skreaver-a2a) - A2A protocol types and client/server
- [`skreaver-mcp`](../skreaver-mcp) - MCP protocol implementation

## License

MIT
