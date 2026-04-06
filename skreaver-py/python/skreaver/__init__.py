"""
Skreaver - High-performance MCP and A2A protocol infrastructure for AI agents.

This package provides Python bindings for Skreaver's Rust implementation,
offering high-performance protocol translation and agent communication.

Example:
    >>> import asyncio
    >>> from skreaver import A2aClient, Task, TaskStatus, ProtocolGateway, Protocol
    >>>
    >>> async def main():
    ...     # Connect to an A2A agent
    ...     client = A2aClient("https://agent.example.com")
    ...     card = await client.get_agent_card()
    ...     print(f"Connected to: {card.name}")
    ...
    ...     # Send a message
    ...     task = await client.send_message("Hello!")
    ...     print(f"Task {task.id} status: {task.status}")
    >>>
    >>> asyncio.run(main())

MCP Example:
    >>> from skreaver import McpToolDefinition, McpToolAnnotations
    >>>
    >>> # Define an MCP tool
    >>> tool = McpToolDefinition("my_tool", "Does something useful")
    >>> tool = tool.with_annotations(
    ...     McpToolAnnotations().with_read_only(True)
    ... )

Protocol Translation:
    >>> from skreaver import ProtocolGateway, Protocol
    >>>
    >>> gateway = ProtocolGateway()
    >>> mcp_request = {"jsonrpc": "2.0", "id": 1, "method": "ping"}
    >>> a2a_task = gateway.translate_to(mcp_request, Protocol.A2a)
"""

import sys

from skreaver._skreaver import (
    # Version
    __version__,
    # A2A types (core)
    Task,
    TaskStatus,
    Message,
    AgentCard,
    # A2A client
    A2aClient,
    # Gateway types
    Protocol,
    ProtocolGateway,
    # MCP types
    McpTaskStatus,
    McpToolDefinition,
    # Submodules
    a2a,
    gateway,
    mcp,
    memory,
    exceptions,
)

# Register submodules in sys.modules for `from skreaver.xxx import yyy` syntax
# This is required for PyO3 submodules to be importable as packages
sys.modules["skreaver.a2a"] = a2a
sys.modules["skreaver.gateway"] = gateway
sys.modules["skreaver.mcp"] = mcp
sys.modules["skreaver.memory"] = memory
sys.modules["skreaver.exceptions"] = exceptions

__all__ = [
    # Version
    "__version__",
    # A2A types (core)
    "Task",
    "TaskStatus",
    "Message",
    "AgentCard",
    # A2A client
    "A2aClient",
    # Gateway types
    "Protocol",
    "ProtocolGateway",
    # MCP types
    "McpTaskStatus",
    "McpToolDefinition",
    # Submodules
    "a2a",
    "gateway",
    "mcp",
    "memory",
    "exceptions",
]
