# skreaver-a2a

A2A (Agent-to-Agent) protocol implementation for Skreaver, enabling interoperability between AI agents using Google's A2A protocol specification.

## Overview

The A2A protocol defines how AI agents discover each other and collaborate through a standard HTTP-based interface. This crate provides both client and server implementations.

## Features

| Feature | Description |
|---------|-------------|
| `client` | HTTP client for connecting to A2A agents |
| `server` | HTTP server for exposing agents via A2A |

## Quick Start

### Creating an A2A Server

```rust
use skreaver_a2a::{A2aServer, AgentHandler, AgentCard, Task, Message};
use async_trait::async_trait;

struct MyAgent;

#[async_trait]
impl AgentHandler for MyAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new("my-agent", "My Agent", "http://localhost:3000")
            .with_description("A helpful AI agent")
            .with_skill(AgentSkill::new("chat", "Chat"))
    }

    async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String> {
        task.add_message(Message::agent("Hello! How can I help?"));
        task.set_status(TaskStatus::Completed);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let server = A2aServer::new(MyAgent);
    server.serve("0.0.0.0:3000").await.unwrap();
}
```

### Using the A2A Client

```rust
use skreaver_a2a::A2aClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = A2aClient::new("http://localhost:3000")?;

    // Discover agent capabilities
    let card = client.get_agent_card().await?;
    println!("Agent: {} with {} skills", card.name, card.skills.len());

    // Send a message
    let task = client.send_message("Hello!").await?;
    println!("Response status: {:?}", task.status);

    Ok(())
}
```

## Protocol Overview

### Agent Discovery

Agents expose their capabilities at `/.well-known/agent.json`:

```json
{
  "agentId": "my-agent",
  "name": "My Agent",
  "url": "http://localhost:3000",
  "skills": [
    { "id": "chat", "name": "Chat", "description": "General conversation" }
  ]
}
```

### Task Lifecycle

```
          ┌─────────┐
          │ Created │
          └────┬────┘
               │ send_message
               ▼
          ┌─────────┐
     ┌───►│ Working │◄───┐
     │    └────┬────┘    │
     │         │         │
     │    ┌────┴────┐    │
     │    ▼         ▼    │
┌────┴─────┐   ┌────────┴───┐
│InputReq'd│   │  Completed │
└──────────┘   └────────────┘
                     │
          ┌──────────┼──────────┐
          ▼          ▼          ▼
     ┌────────┐ ┌────────┐ ┌─────────┐
     │ Failed │ │Cancelled│ │Rejected │
     └────────┘ └────────┘ └─────────┘
```

### Streaming

The server supports Server-Sent Events (SSE) for real-time updates:

```rust
// Client-side streaming
let mut stream = client.send_message_streaming("Count to 5").await?;

while let Some(event) = stream.next().await {
    match event? {
        StreamingEvent::TaskStatusUpdate(update) => {
            println!("Status: {:?}", update.status);
        }
        StreamingEvent::TaskArtifactUpdate(update) => {
            println!("Artifact: {:?}", update.artifact);
        }
    }
}
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/.well-known/agent.json` | GET | Agent card discovery |
| `/tasks/send` | POST | Send message (sync) |
| `/tasks/sendSubscribe` | POST | Send message (streaming) |
| `/tasks/{id}` | GET | Get task status |
| `/tasks/{id}/cancel` | POST | Cancel a task |
| `/tasks/{id}/subscribe` | GET | Subscribe to task updates |

## Authentication

The client supports multiple authentication methods:

```rust
// Bearer token
let client = A2aClient::new(url)?
    .with_bearer_token("your-token");

// API key in header
let client = A2aClient::new(url)?
    .with_api_key("X-API-Key", "your-key");
```

## Error Handling

The crate uses `A2aError` for all error conditions:

```rust
use skreaver_a2a::{A2aError, A2aResult};

match client.send_message("test").await {
    Ok(task) => println!("Success: {:?}", task.status),
    Err(A2aError::TaskNotFound { task_id }) => {
        println!("Task {} not found", task_id);
    }
    Err(A2aError::AuthenticationRequired) => {
        println!("Need to authenticate");
    }
    Err(e) => println!("Error: {}", e),
}
```

## Related Crates

- [`skreaver-agent`](../skreaver-agent) - Unified agent interface supporting both A2A and MCP
- [`skreaver-mcp`](../skreaver-mcp) - MCP protocol implementation

## License

MIT
