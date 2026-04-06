"""Tests for MCP types."""

import pytest


def test_mcp_task_status_values():
    """Test McpTaskStatus enum values."""
    from skreaver.mcp import McpTaskStatus

    assert McpTaskStatus.Working is not None
    assert McpTaskStatus.InputRequired is not None
    assert McpTaskStatus.Completed is not None
    assert McpTaskStatus.Failed is not None
    assert McpTaskStatus.Cancelled is not None


def test_mcp_task_status_terminal():
    """Test McpTaskStatus.is_terminal() method."""
    from skreaver.mcp import McpTaskStatus

    assert not McpTaskStatus.Working.is_terminal()
    assert not McpTaskStatus.InputRequired.is_terminal()
    assert McpTaskStatus.Completed.is_terminal()
    assert McpTaskStatus.Failed.is_terminal()
    assert McpTaskStatus.Cancelled.is_terminal()


def test_mcp_task_creation():
    """Test McpTask creation."""
    from skreaver.mcp import McpTask, McpTaskStatus

    task = McpTask("task-123")
    assert task.task_id == "task-123"
    assert task.status == McpTaskStatus.Working
    assert not task.is_terminal()


def test_mcp_task_with_ttl():
    """Test McpTask with TTL."""
    from skreaver.mcp import McpTask

    task = McpTask("task-123", ttl=60000)
    assert task.ttl == 60000


def test_mcp_task_timestamps():
    """Test McpTask timestamps."""
    from skreaver.mcp import McpTask

    task = McpTask("task-123")
    assert task.created_at is not None
    # ISO-8601 format
    assert "T" in task.created_at


def test_mcp_tool_annotations_creation():
    """Test McpToolAnnotations creation."""
    from skreaver.mcp import McpToolAnnotations

    annotations = McpToolAnnotations()
    assert annotations.read_only_hint is None
    assert annotations.destructive_hint is None
    assert annotations.idempotent_hint is None
    assert annotations.open_world_hint is None


def test_mcp_tool_annotations_builder():
    """Test McpToolAnnotations builder pattern."""
    from skreaver.mcp import McpToolAnnotations

    annotations = (
        McpToolAnnotations()
        .with_read_only(True)
        .with_destructive(False)
        .with_idempotent(True)
        .with_open_world(True)
    )

    assert annotations.read_only_hint is True
    assert annotations.destructive_hint is False
    assert annotations.idempotent_hint is True
    assert annotations.open_world_hint is True


def test_mcp_tool_definition_creation():
    """Test McpToolDefinition creation."""
    from skreaver.mcp import McpToolDefinition

    tool = McpToolDefinition("my_tool", "A test tool")
    assert tool.name == "my_tool"
    assert tool.description == "A test tool"


def test_mcp_tool_definition_with_schema():
    """Test McpToolDefinition with input schema."""
    from skreaver.mcp import McpToolDefinition

    schema = {
        "type": "object",
        "properties": {
            "query": {"type": "string"}
        },
        "required": ["query"]
    }

    tool = McpToolDefinition("search", "Search tool", input_schema=schema)
    input_schema = tool.input_schema
    assert input_schema["type"] == "object"
    assert "query" in input_schema["properties"]


def test_mcp_tool_definition_builder():
    """Test McpToolDefinition builder pattern."""
    from skreaver.mcp import McpToolDefinition, McpToolAnnotations

    annotations = McpToolAnnotations().with_read_only(True)

    tool = (
        McpToolDefinition("my_tool", "A test tool")
        .with_title("My Tool")
        .with_annotations(annotations)
    )

    assert tool.title == "My Tool"
    assert tool.annotations is not None
    assert tool.annotations.read_only_hint is True


def test_mcp_tool_definition_serialization():
    """Test McpToolDefinition to_dict/from_dict."""
    from skreaver.mcp import McpToolDefinition

    tool1 = McpToolDefinition("my_tool", "A test tool")
    data = tool1.to_dict()

    assert isinstance(data, dict)
    assert data["name"] == "my_tool"
    assert data["description"] == "A test tool"

    tool2 = McpToolDefinition.from_dict(data)
    assert tool2.name == tool1.name
    assert tool2.description == tool1.description
