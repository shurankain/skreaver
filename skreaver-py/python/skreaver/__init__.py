"""
Skreaver - High-performance MCP and A2A protocol infrastructure for AI agents.

This package provides Python bindings for Skreaver's Rust implementation,
offering high-performance protocol translation and agent communication.

Example:
    >>> from skreaver import Task, TaskStatus, ProtocolGateway, Protocol
    >>>
    >>> # Create a task
    >>> task = Task()
    >>> task.add_message(Message.user("Hello!"))
    >>> print(f"Task {task.id} status: {task.status}")
    >>>
    >>> # Protocol translation
    >>> gateway = ProtocolGateway()
    >>> mcp_request = {"jsonrpc": "2.0", "id": 1, "method": "ping"}
    >>> a2a_task = gateway.translate_to(mcp_request, Protocol.A2a)
"""

from skreaver._skreaver import (
    # Version
    __version__,
    # A2A types
    Task,
    TaskStatus,
    Message,
    AgentCard,
    # Gateway types
    Protocol,
    ProtocolGateway,
    # Submodules
    a2a,
    gateway,
    mcp,
    memory,
    exceptions,
)

__all__ = [
    # Version
    "__version__",
    # A2A types
    "Task",
    "TaskStatus",
    "Message",
    "AgentCard",
    # Gateway types
    "Protocol",
    "ProtocolGateway",
    # Submodules
    "a2a",
    "gateway",
    "mcp",
    "memory",
    "exceptions",
]
