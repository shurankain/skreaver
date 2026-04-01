# Skreaver Python Bindings

High-performance MCP and A2A protocol infrastructure for AI agents.

## Installation

```bash
pip install skreaver
```

## Quick Start

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
```

### Agent Cards

```python
from skreaver.a2a import AgentCard, AgentSkill

# Create an agent card
card = AgentCard("my-agent", "My Agent", "https://my-agent.example.com")
card = card.with_description("An AI agent that can help with tasks")
card = card.with_streaming()
card = card.with_skill(
    AgentSkill("search", "Search").with_description("Search the web")
)

print(f"Agent: {card.name}")
print(f"Supports streaming: {card.supports_streaming()}")
```

## Features

- **Protocol Gateway**: Bidirectional MCP <-> A2A translation
- **A2A Types**: Task, Message, Artifact, AgentCard
- **High Performance**: <5ms overhead vs pure Rust
- **Type Safety**: Full type stubs for IDE support

## Documentation

- [Skreaver Documentation](https://docs.rs/skreaver)
- [GitHub Repository](https://github.com/shurankain/skreaver)

## License

MIT
