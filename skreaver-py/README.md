# Skreaver Python Bindings

High-performance MCP and A2A protocol infrastructure for AI agents.

**Skreaver** provides Python bindings to a Rust-based protocol backbone, enabling:
- Sub-millisecond protocol translation between MCP and A2A
- Native async support for Python coroutines
- Full type hints for IDE support

## Installation

```bash
pip install skreaver
```

## Quick Start

### A2A Client (Async)

```python
import asyncio
from skreaver import A2aClient

async def main():
    # Connect to an A2A-compatible agent
    client = A2aClient("https://agent.example.com")

    # Optional: Add authentication
    client = client.with_bearer_token("your-token")

    # Fetch agent capabilities
    card = await client.get_agent_card()
    print(f"Connected to: {card.name}")
    print(f"Skills: {[s.name for s in card.skills]}")

    # Send a message and create a task
    task = await client.send_message("Hello, agent!")
    print(f"Task {task.id}: {task.status}")

    # Wait for completion
    task = await client.wait_for_task(task.id, poll_interval_ms=1000, timeout_ms=60000)
    print(f"Final status: {task.status}")

asyncio.run(main())
```

### Protocol Gateway

```python
from skreaver import ProtocolGateway, Protocol

gateway = ProtocolGateway()

# Translate MCP request to A2A task
mcp_request = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {"name": "search", "arguments": {"query": "rust"}}
}

a2a_task = gateway.translate_to(mcp_request, Protocol.A2a)
print(f"Translated to A2A: {a2a_task}")

# Auto-detect and translate to opposite protocol
result = gateway.translate_opposite(mcp_request)
```

### A2A Tasks

```python
from skreaver import Task, TaskStatus, Message

# Create a task
task = Task()
print(f"Task ID: {task.id}")
print(f"Status: {task.status}")

# Add messages
task.add_message(Message.user("Hello, agent!"))
task.add_message(Message.agent("Hello! How can I help you?"))

# Update status
task.status = TaskStatus.Completed
assert task.is_terminal()

# Serialize to dict
data = task.to_dict()
task2 = Task.from_dict(data)
```

### MCP Tool Definitions

```python
from skreaver.mcp import McpToolDefinition, McpToolAnnotations

# Define an MCP tool with annotations
tool = McpToolDefinition(
    name="web_search",
    description="Search the web for information",
    input_schema={
        "type": "object",
        "properties": {
            "query": {"type": "string", "description": "Search query"}
        },
        "required": ["query"]
    }
)

# Add behavior hints
annotations = (
    McpToolAnnotations()
    .with_read_only(True)
    .with_idempotent(True)
    .with_open_world(True)
)
tool = tool.with_annotations(annotations)

print(f"Tool: {tool.name}")
print(f"Read-only: {tool.annotations.read_only_hint}")
```

### Agent Cards

```python
from skreaver.a2a import AgentCard, AgentSkill

# Create an agent card
card = AgentCard("my-agent", "My Agent", "https://my-agent.example.com")
card = (
    card.with_description("An AI agent that can help with tasks")
    .with_streaming()
    .with_push_notifications()
    .with_skill(
        AgentSkill("search", "Web Search")
        .with_description("Search the web for information")
    )
)

print(f"Agent: {card.name}")
print(f"Supports streaming: {card.supports_streaming()}")
```

## Features

- **A2A Client**: Full async client for A2A-compatible agents
- **Protocol Gateway**: Bidirectional MCP <-> A2A translation
- **A2A Types**: Task, Message, Part, Artifact, AgentCard, AgentSkill
- **MCP Types**: McpTask, McpTaskStatus, McpToolDefinition, McpToolAnnotations
- **High Performance**: <5ms overhead vs pure Rust
- **Type Safety**: Full `.pyi` type stubs for IDE support
- **Python 3.10+**: Modern Python with async/await support

## API Reference

### A2A Types

| Type | Description |
|------|-------------|
| `Task` | Core unit of work with status lifecycle |
| `TaskStatus` | Enum: Working, Completed, Failed, Cancelled, Rejected, InputRequired |
| `Message` | Communication between user and agent |
| `Part` | Content part (text, file, data) |
| `Artifact` | Task output with optional label and description |
| `AgentCard` | Agent capability discovery |
| `AgentSkill` | Individual agent skill |
| `A2aClient` | Async HTTP client for A2A agents |

### Gateway Types

| Type | Description |
|------|-------------|
| `Protocol` | Enum: Mcp, A2a |
| `ProtocolGateway` | Translates between MCP and A2A |
| `ProtocolDetector` | Detects protocol from message format |

### MCP Types

| Type | Description |
|------|-------------|
| `McpTaskStatus` | Enum: Working, InputRequired, Completed, Failed, Cancelled |
| `McpTask` | Long-running operation tracking |
| `McpToolDefinition` | Tool metadata with JSON Schema |
| `McpToolAnnotations` | Tool behavior hints |

## Development

```bash
# Install maturin
pip install maturin

# Build and install in development mode
cd skreaver-py
maturin develop

# Run tests
pytest tests/

# Type checking
mypy .
```

## Documentation

- [Skreaver Documentation](https://docs.rs/skreaver)
- [GitHub Repository](https://github.com/shurankain/skreaver)
- [MCP Specification](https://modelcontextprotocol.io/)
- [A2A Protocol](https://google.github.io/A2A/)

## License

MIT
