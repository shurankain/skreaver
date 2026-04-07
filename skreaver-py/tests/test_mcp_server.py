"""Tests for MCP Server."""

import pytest


class TestMcpServer:
    """Tests for McpServer class."""

    def test_server_creation(self):
        """Test McpServer creation with name and version."""
        from skreaver.mcp import McpServer

        server = McpServer("test-server", "1.0.0")
        assert server.name == "test-server"
        assert server.version == "1.0.0"
        assert server.tool_count == 0

    def test_server_creation_default_version(self):
        """Test McpServer creation with default version."""
        from skreaver.mcp import McpServer

        server = McpServer("my-server")
        assert server.name == "my-server"
        assert server.version == "0.1.0"

    def test_add_tool(self):
        """Test adding a tool to the server."""
        from skreaver.mcp import McpServer

        server = McpServer("test-server")

        def my_handler(params):
            return {"result": "hello"}

        server.add_tool("greet", "Says hello", my_handler)
        assert server.tool_count == 1
        assert "greet" in server.list_tools()

    def test_add_tool_with_schema(self):
        """Test adding a tool with input schema."""
        from skreaver.mcp import McpServer

        server = McpServer("test-server")

        def calculator(params):
            a = params.get("a", 0)
            b = params.get("b", 0)
            return {"sum": a + b}

        schema = {
            "type": "object",
            "properties": {
                "a": {"type": "number"},
                "b": {"type": "number"},
            },
            "required": ["a", "b"],
        }

        server.add_tool("calculator", "Adds two numbers", calculator, schema)
        assert server.tool_count == 1
        assert "calculator" in server.list_tools()

    def test_add_multiple_tools(self):
        """Test adding multiple tools."""
        from skreaver.mcp import McpServer

        server = McpServer("test-server")

        server.add_tool("tool1", "First tool", lambda x: x)
        server.add_tool("tool2", "Second tool", lambda x: x)
        server.add_tool("tool3", "Third tool", lambda x: x)

        assert server.tool_count == 3
        tools = server.list_tools()
        assert "tool1" in tools
        assert "tool2" in tools
        assert "tool3" in tools

    def test_add_duplicate_tool_fails(self):
        """Test that adding duplicate tool name fails."""
        from skreaver.mcp import McpServer
        from skreaver.exceptions import McpError

        server = McpServer("test-server")
        server.add_tool("my_tool", "A tool", lambda x: x)

        with pytest.raises(McpError):
            server.add_tool("my_tool", "Same name", lambda x: x)

    def test_add_tool_empty_name_fails(self):
        """Test that empty tool name fails."""
        from skreaver.mcp import McpServer
        from skreaver.exceptions import McpError

        server = McpServer("test-server")

        with pytest.raises(McpError):
            server.add_tool("", "Empty name", lambda x: x)

    def test_add_tool_invalid_name_fails(self):
        """Test that invalid tool name fails."""
        from skreaver.mcp import McpServer
        from skreaver.exceptions import McpError

        server = McpServer("test-server")

        # Spaces not allowed
        with pytest.raises(McpError):
            server.add_tool("my tool", "Has space", lambda x: x)

        # Special chars not allowed
        with pytest.raises(McpError):
            server.add_tool("my@tool", "Has @", lambda x: x)

    def test_add_tool_long_name_fails(self):
        """Test that very long tool name fails."""
        from skreaver.mcp import McpServer
        from skreaver.exceptions import McpError

        server = McpServer("test-server")

        long_name = "x" * 200  # Over 128 char limit
        with pytest.raises(McpError):
            server.add_tool(long_name, "Too long", lambda x: x)

    def test_server_repr(self):
        """Test server string representation."""
        from skreaver.mcp import McpServer

        server = McpServer("my-server", "2.0.0")
        server.add_tool("tool1", "A tool", lambda x: x)

        repr_str = repr(server)
        assert "McpServer" in repr_str
        assert "my-server" in repr_str
        assert "2.0.0" in repr_str
        assert "tools=1" in repr_str


class TestMcpToolBuilder:
    """Tests for McpToolBuilder class."""

    def test_builder_creation(self):
        """Test McpToolBuilder creation."""
        from skreaver.mcp import McpToolBuilder

        builder = McpToolBuilder("my_tool")
        assert builder.name == "my_tool"

    def test_builder_with_description(self):
        """Test setting description."""
        from skreaver.mcp import McpToolBuilder

        builder = McpToolBuilder("my_tool").description("Does something")
        # Builder returns new instance
        assert builder.name == "my_tool"

    def test_builder_read_only(self):
        """Test marking as read-only."""
        from skreaver.mcp import McpToolBuilder

        builder = McpToolBuilder("my_tool").read_only()
        assert builder.name == "my_tool"

    def test_builder_destructive(self):
        """Test marking as destructive."""
        from skreaver.mcp import McpToolBuilder

        builder = McpToolBuilder("my_tool").destructive()
        assert builder.name == "my_tool"

    def test_builder_idempotent(self):
        """Test marking as idempotent."""
        from skreaver.mcp import McpToolBuilder

        builder = McpToolBuilder("my_tool").idempotent()
        assert builder.name == "my_tool"

    def test_builder_closed_world(self):
        """Test marking as closed world."""
        from skreaver.mcp import McpToolBuilder

        builder = McpToolBuilder("my_tool").closed_world()
        assert builder.name == "my_tool"

    def test_builder_with_input_schema(self):
        """Test setting input schema."""
        from skreaver.mcp import McpToolBuilder

        schema = {"type": "object", "properties": {"x": {"type": "number"}}}
        builder = McpToolBuilder("my_tool").input_schema(schema)
        assert builder.name == "my_tool"

    def test_builder_chaining(self):
        """Test fluent builder chaining."""
        from skreaver.mcp import McpToolBuilder

        builder = (
            McpToolBuilder("calculator")
            .description("Performs calculations")
            .read_only()
            .idempotent()
        )
        assert builder.name == "calculator"

    def test_builder_repr(self):
        """Test builder string representation."""
        from skreaver.mcp import McpToolBuilder

        builder = McpToolBuilder("test_tool")
        repr_str = repr(builder)

        assert "McpToolBuilder" in repr_str
        assert "test_tool" in repr_str


class TestMcpTypes:
    """Tests for MCP type imports."""

    def test_imports_from_mcp_module(self):
        """Test importing from skreaver.mcp."""
        from skreaver.mcp import (
            McpServer,
            McpTask,
            McpTaskStatus,
            McpToolAnnotations,
            McpToolBuilder,
            McpToolDefinition,
        )

        # Verify classes are available
        assert McpServer is not None
        assert McpToolBuilder is not None
        assert McpTask is not None
        assert McpTaskStatus is not None
        assert McpToolAnnotations is not None
        assert McpToolDefinition is not None

    def test_mcp_task_status_values(self):
        """Test MCP task status enum values."""
        from skreaver.mcp import McpTaskStatus

        assert McpTaskStatus.Working is not None
        assert McpTaskStatus.InputRequired is not None
        assert McpTaskStatus.Completed is not None
        assert McpTaskStatus.Failed is not None
        assert McpTaskStatus.Cancelled is not None

    def test_mcp_task_status_terminal(self):
        """Test is_terminal method."""
        from skreaver.mcp import McpTaskStatus

        assert not McpTaskStatus.Working.is_terminal()
        assert not McpTaskStatus.InputRequired.is_terminal()
        assert McpTaskStatus.Completed.is_terminal()
        assert McpTaskStatus.Failed.is_terminal()
        assert McpTaskStatus.Cancelled.is_terminal()

    def test_mcp_tool_definition_creation(self):
        """Test McpToolDefinition creation."""
        from skreaver.mcp import McpToolDefinition

        tool = McpToolDefinition("my_tool", "Does something useful")
        assert tool.name == "my_tool"
        assert tool.description == "Does something useful"

    def test_mcp_tool_annotations(self):
        """Test McpToolAnnotations fluent builder."""
        from skreaver.mcp import McpToolAnnotations

        annotations = (
            McpToolAnnotations()
            .with_read_only(True)
            .with_idempotent(True)
        )
        assert annotations.read_only_hint is True
        assert annotations.idempotent_hint is True


class TestMcpTask:
    """Tests for MCP task functionality."""

    def test_mcp_task_creation(self):
        """Test McpTask creation."""
        from skreaver.mcp import McpTask

        task = McpTask("task-123")
        assert task.task_id == "task-123"

    def test_mcp_task_with_ttl(self):
        """Test McpTask creation with TTL."""
        from skreaver.mcp import McpTask

        task = McpTask("task-456", ttl=60000)
        assert task.task_id == "task-456"
        assert task.ttl == 60000

    def test_mcp_task_status(self):
        """Test McpTask status."""
        from skreaver.mcp import McpTask, McpTaskStatus

        task = McpTask("task-789")
        # New tasks start in Working status
        assert task.status == McpTaskStatus.Working

    def test_mcp_task_is_terminal(self):
        """Test McpTask is_terminal method."""
        from skreaver.mcp import McpTask

        task = McpTask("task-abc")
        assert not task.is_terminal()

    def test_mcp_task_repr(self):
        """Test McpTask string representation."""
        from skreaver.mcp import McpTask

        task = McpTask("task-repr")
        repr_str = repr(task)

        assert "McpTask" in repr_str
        assert "task-repr" in repr_str
