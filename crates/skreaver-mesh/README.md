# Skreaver Mesh

Multi-agent communication layer for Skreaver agent systems.

## Features

- **Typed Messages**: Strongly-typed message schemas with automatic serialization
- **Multiple Patterns**: Point-to-point, broadcast, and pub/sub messaging
- **Redis Backend**: Production-ready Redis Pub/Sub implementation
- **Backpressure**: Queue depth monitoring and flow control
- **Reliability**: Message correlation for request/reply patterns
- **Observability**: Built-in tracing and metrics

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
skreaver-mesh = { version = "0.1", features = ["redis"] }
```

## Quick Start

### Point-to-Point Messaging

```rust
use skreaver_mesh::{RedisMesh, AgentMesh, AgentId, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to mesh
    let mesh = RedisMesh::new("redis://localhost:6379").await?;

    // Register agents
    let sender = AgentId::from("agent-1");
    let receiver = AgentId::from("agent-2");
    mesh.register_presence(&sender, 60).await?;
    mesh.register_presence(&receiver, 60).await?;

    // Send message
    let msg = Message::new("Hello from agent-1").from(sender.clone());
    mesh.send(&receiver, msg).await?;

    // Receive message
    if let Some(received) = mesh.receive(&receiver, 5).await? {
        println!("Received: {:?}", received);
    }

    Ok(())
}
```

### Pub/Sub Messaging

```rust
use skreaver_mesh::{RedisMesh, AgentMesh, AgentId, Topic, Message};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mesh = RedisMesh::new("redis://localhost:6379").await?;
    let topic = Topic::from("notifications");

    // Subscribe to topic
    let mut stream = mesh.subscribe(&topic).await?;

    // Publish message
    let msg = Message::new("Important notification");
    mesh.publish(&topic, msg).await?;

    // Receive from stream
    if let Some(Ok(received)) = stream.next().await {
        println!("Received: {:?}", received);
    }

    Ok(())
}
```

### Broadcast

```rust
use skreaver_mesh::{RedisMesh, AgentMesh, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mesh = RedisMesh::new("redis://localhost:6379").await?;

    // Broadcast to all agents
    let msg = Message::new("System announcement");
    mesh.broadcast(msg).await?;

    Ok(())
}
```

## Message Types

Messages support multiple payload types:

```rust
use skreaver_mesh::Message;

// Text message
let text_msg = Message::new("hello");

// JSON message
let json_msg = Message::new(serde_json::json!({"key": "value"}));

// Binary message
let binary_msg = Message::new(vec![1u8, 2, 3]);

// With metadata
let msg = Message::new("data")
    .from("agent-1")
    .to("agent-2")
    .with_metadata("priority", "high")
    .with_correlation_id("req-123");
```

## Agent Presence

Track which agents are active in the mesh:

```rust
// Register agent with 60s TTL
mesh.register_presence(&agent_id, 60).await?;

// Check if agent is reachable
if mesh.is_reachable(&agent_id).await {
    println!("Agent is online");
}

// List all active agents
let agents = mesh.list_agents().await?;

// Deregister when done
mesh.deregister_presence(&agent_id).await?;
```

## Examples

Run the included examples:

```bash
# Ping-pong example (requires Redis)
cargo run --example mesh_ping_pong --features redis

# Broadcast example (requires Redis)
cargo run --example mesh_broadcast --features redis
```

## Requirements

- Rust 1.70+
- Redis server for the Redis backend

## Architecture

```
┌─────────────────────────────────────────────┐
│            AgentMesh Trait                  │
├─────────────────────────────────────────────┤
│  • send()      - Point-to-point messaging   │
│  • broadcast() - One-to-many messaging      │
│  • publish()   - Topic-based pub/sub        │
│  • subscribe() - Topic subscription         │
│  • register_presence() - Agent tracking     │
└─────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────┐
│         Redis Backend (RedisMesh)           │
├─────────────────────────────────────────────┤
│  • Connection pooling                       │
│  • Pub/Sub channels                         │
│  • Agent mailboxes (Redis lists)            │
│  • Presence tracking (Redis sets + TTL)     │
└─────────────────────────────────────────────┘
```

## Performance

- Point-to-point: <5ms latency (local Redis)
- Pub/Sub: <10ms latency for delivery to 10 subscribers
- Queue depth monitoring: O(n) where n = number of agents

## License

MIT

## Contributing

Contributions welcome! Please see the main [Skreaver repository](https://github.com/shurankain/skreaver) for contribution guidelines.
