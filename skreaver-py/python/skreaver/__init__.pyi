"""Type stubs for skreaver package."""

from typing import Any, Awaitable, Optional

# Version
__version__: str

# =============================================================================
# A2A Types
# =============================================================================

class TaskStatus:
    """Task lifecycle status."""

    Working: TaskStatus
    Completed: TaskStatus
    Failed: TaskStatus
    Cancelled: TaskStatus
    Rejected: TaskStatus
    InputRequired: TaskStatus

    def is_terminal(self) -> bool:
        """Check if this status is terminal (no further transitions)."""
        ...

class Task:
    """A2A Task - core unit of work."""

    id: str
    status: TaskStatus
    context_id: Optional[str]

    def __init__(self, id: Optional[str] = None) -> None:
        """Create a new task with optional ID."""
        ...

    def add_message(self, message: Message) -> None:
        """Add a message to the task."""
        ...

    def add_artifact(self, artifact: Artifact) -> None:
        """Add an artifact to the task."""
        ...

    def is_terminal(self) -> bool:
        """Check if task is in terminal state."""
        ...

    def requires_input(self) -> bool:
        """Check if task requires user input."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Convert to Python dict."""
        ...

    @staticmethod
    def from_dict(data: dict[str, Any]) -> Task:
        """Create from Python dict."""
        ...

class Message:
    """A2A Message - communication between agents."""

    id: Optional[str]
    role: str
    text: Optional[str]

    @staticmethod
    def user(text: str) -> Message:
        """Create a user message."""
        ...

    @staticmethod
    def agent(text: str) -> Message:
        """Create an agent message."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Convert to Python dict."""
        ...

class Part:
    """A2A Part - content part of a message."""

    part_type: str

    @staticmethod
    def text(content: str) -> Part:
        """Create a text part."""
        ...

    @staticmethod
    def file(url: str, mime_type: str) -> Part:
        """Create a file part."""
        ...

class Artifact:
    """A2A Artifact - output produced by a task."""

    id: str
    label: Optional[str]
    description: Optional[str]

    @staticmethod
    def text(content: str) -> Artifact:
        """Create a text artifact with auto-generated ID."""
        ...

    def with_label(self, label: str) -> Artifact:
        """Set artifact label (returns new instance)."""
        ...

    def with_description(self, desc: str) -> Artifact:
        """Set artifact description (returns new instance)."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Convert to Python dict."""
        ...

class AgentCard:
    """A2A AgentCard - describes agent capabilities."""

    agent_id: str
    name: str
    description: Optional[str]

    def __init__(self, agent_id: str, name: str, base_url: str) -> None:
        """Create a new agent card."""
        ...

    def with_description(self, desc: str) -> AgentCard:
        """Set description (returns new instance)."""
        ...

    def with_streaming(self) -> AgentCard:
        """Enable streaming capability (returns new instance)."""
        ...

    def with_push_notifications(self) -> AgentCard:
        """Enable push notifications capability (returns new instance)."""
        ...

    def with_skill(self, skill: AgentSkill) -> AgentCard:
        """Add a skill to the agent (returns new instance)."""
        ...

    def supports_streaming(self) -> bool:
        """Check if streaming is supported."""
        ...

    def supports_push_notifications(self) -> bool:
        """Check if push notifications are supported."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Convert to Python dict."""
        ...

    @staticmethod
    def from_dict(data: dict[str, Any]) -> AgentCard:
        """Create from Python dict."""
        ...

class AgentSkill:
    """A2A AgentSkill - describes a skill the agent can perform."""

    id: str
    name: str
    description: Optional[str]

    def __init__(self, id: str, name: str) -> None:
        """Create a new skill."""
        ...

    def with_description(self, desc: str) -> AgentSkill:
        """Set description (returns new instance)."""
        ...

# =============================================================================
# A2A Client
# =============================================================================

class A2aClient:
    """A2A Protocol Client - communicates with A2A-compatible agents."""

    base_url: str

    def __init__(self, url: str) -> None:
        """Create a new A2A client for the given agent URL."""
        ...

    def with_bearer_token(self, token: str) -> A2aClient:
        """Set bearer token authentication (returns new client)."""
        ...

    def with_api_key(self, header_name: str, api_key: str) -> A2aClient:
        """Set API key authentication in header (returns new client)."""
        ...

    def with_api_key_query(self, param_name: str, api_key: str) -> A2aClient:
        """Set API key authentication in query parameter (returns new client)."""
        ...

    def get_agent_card(self) -> Awaitable[AgentCard]:
        """Fetch the agent card (async)."""
        ...

    def send_message(self, text: str) -> Awaitable[Task]:
        """Send a message to the agent (async)."""
        ...

    def continue_task(self, task_id: str, text: str) -> Awaitable[Task]:
        """Continue an existing task with a new message (async)."""
        ...

    def get_task(self, task_id: str) -> Awaitable[Task]:
        """Get the current state of a task (async)."""
        ...

    def cancel_task(
        self, task_id: str, reason: Optional[str] = None
    ) -> Awaitable[Task]:
        """Cancel a running task (async)."""
        ...

    def wait_for_task(
        self,
        task_id: str,
        poll_interval_ms: int = 5000,
        timeout_ms: int = 300000,
    ) -> Awaitable[Task]:
        """Wait for a task to complete (async)."""
        ...

    def send(
        self,
        message: Message,
        task_id: Optional[str] = None,
        context_id: Optional[str] = None,
    ) -> Awaitable[Task]:
        """Send a message with full control (async)."""
        ...

# =============================================================================
# Gateway Types
# =============================================================================

class Protocol:
    """Protocol type enum."""

    Mcp: Protocol
    A2a: Protocol

class ProtocolDetector:
    """Protocol detector - identifies message format."""

    def __init__(self) -> None:
        """Create a new protocol detector."""
        ...

    @staticmethod
    def strict() -> ProtocolDetector:
        """Create a strict protocol detector."""
        ...

    def detect(self, message: dict[str, Any]) -> Protocol:
        """Detect protocol from a Python dict."""
        ...

    def detect_str(self, json_str: str) -> Protocol:
        """Detect protocol from a JSON string."""
        ...

class ProtocolGateway:
    """Protocol gateway - translates between MCP and A2A."""

    def __init__(self) -> None:
        """Create a new protocol gateway."""
        ...

    def translate_to(
        self, message: dict[str, Any], target: Protocol
    ) -> dict[str, Any]:
        """Translate message to target protocol."""
        ...

    def translate_opposite(self, message: dict[str, Any]) -> dict[str, Any]:
        """Translate to the opposite protocol."""
        ...

    def detect(self, message: dict[str, Any]) -> Protocol:
        """Detect the protocol of a message."""
        ...

# =============================================================================
# MCP Types
# =============================================================================

class McpTaskStatus:
    """MCP Task Status - lifecycle states for long-running operations."""

    Working: McpTaskStatus
    InputRequired: McpTaskStatus
    Completed: McpTaskStatus
    Failed: McpTaskStatus
    Cancelled: McpTaskStatus

    def is_terminal(self) -> bool:
        """Check if this status is terminal."""
        ...

class McpTask:
    """MCP Task - tracks long-running operations."""

    task_id: str
    status: McpTaskStatus
    status_message: Optional[str]
    created_at: str
    last_updated_at: Optional[str]
    ttl: Optional[int]
    poll_interval: Optional[int]
    result: Optional[Any]

    def __init__(self, task_id: str, ttl: Optional[int] = None) -> None:
        """Create a new MCP task."""
        ...

    def is_terminal(self) -> bool:
        """Check if task is in terminal state."""
        ...

    def is_expired(self) -> bool:
        """Check if task has expired based on TTL."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Convert to Python dict."""
        ...

class McpToolAnnotations:
    """MCP Tool Annotations - behavior hints."""

    read_only_hint: Optional[bool]
    destructive_hint: Optional[bool]
    idempotent_hint: Optional[bool]
    open_world_hint: Optional[bool]

    def __init__(self) -> None:
        """Create new tool annotations."""
        ...

    def with_read_only(self, value: bool) -> McpToolAnnotations:
        """Set read-only hint (returns new instance)."""
        ...

    def with_destructive(self, value: bool) -> McpToolAnnotations:
        """Set destructive hint (returns new instance)."""
        ...

    def with_idempotent(self, value: bool) -> McpToolAnnotations:
        """Set idempotent hint (returns new instance)."""
        ...

    def with_open_world(self, value: bool) -> McpToolAnnotations:
        """Set open-world hint (returns new instance)."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Convert to Python dict."""
        ...

class McpToolDefinition:
    """MCP Tool Definition - describes a tool's interface."""

    name: str
    title: Optional[str]
    description: str
    input_schema: dict[str, Any]
    output_schema: Optional[dict[str, Any]]
    annotations: Optional[McpToolAnnotations]

    def __init__(
        self,
        name: str,
        description: str,
        input_schema: Optional[dict[str, Any]] = None,
    ) -> None:
        """Create a new tool definition."""
        ...

    def with_title(self, title: str) -> McpToolDefinition:
        """Set title (returns new instance)."""
        ...

    def with_output_schema(self, schema: dict[str, Any]) -> McpToolDefinition:
        """Set output schema (returns new instance)."""
        ...

    def with_annotations(self, annotations: McpToolAnnotations) -> McpToolDefinition:
        """Set annotations (returns new instance)."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Convert to Python dict."""
        ...

    @staticmethod
    def from_dict(data: dict[str, Any]) -> McpToolDefinition:
        """Create from Python dict."""
        ...

# =============================================================================
# Submodules
# =============================================================================

class a2a:
    """A2A protocol submodule."""

    TaskStatus: type[TaskStatus]
    Task: type[Task]
    Message: type[Message]
    Part: type[Part]
    Artifact: type[Artifact]
    AgentCard: type[AgentCard]
    AgentSkill: type[AgentSkill]
    A2aClient: type[A2aClient]

class gateway:
    """Gateway submodule."""

    Protocol: type[Protocol]
    ProtocolDetector: type[ProtocolDetector]
    ProtocolGateway: type[ProtocolGateway]

class mcp:
    """MCP submodule."""

    McpTaskStatus: type[McpTaskStatus]
    McpTask: type[McpTask]
    McpToolAnnotations: type[McpToolAnnotations]
    McpToolDefinition: type[McpToolDefinition]

class memory:
    """Memory backends submodule."""

    pass

class exceptions:
    """Exception types submodule."""

    class SkreavorError(Exception):
        """Base exception for all Skreaver errors."""

        pass

    class A2aError(SkreavorError):
        """A2A protocol error."""

        pass

    class TaskNotFoundError(A2aError):
        """Task not found error."""

        pass

    class GatewayError(SkreavorError):
        """Gateway error."""

        pass

    class ProtocolDetectionError(GatewayError):
        """Protocol detection error."""

        pass

    class TranslationError(GatewayError):
        """Translation error."""

        pass

    class MemoryError(SkreavorError):
        """Memory backend error."""

        pass

    class McpError(SkreavorError):
        """MCP error."""

        pass
