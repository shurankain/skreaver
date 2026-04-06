"""Tests for A2A types."""

import pytest


def test_task_creation():
    """Test creating a task."""
    from skreaver import Task, TaskStatus

    task = Task()
    assert task.id is not None
    assert len(task.id) > 0
    assert task.status == TaskStatus.Working


def test_task_with_custom_id():
    """Test creating a task with custom ID."""
    from skreaver import Task

    task = Task("my-custom-id")
    assert task.id == "my-custom-id"


def test_task_status_values():
    """Test all TaskStatus values exist."""
    from skreaver import TaskStatus

    assert TaskStatus.Working is not None
    assert TaskStatus.Completed is not None
    assert TaskStatus.Failed is not None
    assert TaskStatus.Cancelled is not None
    assert TaskStatus.Rejected is not None
    assert TaskStatus.InputRequired is not None


def test_task_status_terminal():
    """Test TaskStatus.is_terminal() method."""
    from skreaver import TaskStatus

    assert not TaskStatus.Working.is_terminal()
    assert not TaskStatus.InputRequired.is_terminal()
    assert TaskStatus.Completed.is_terminal()
    assert TaskStatus.Failed.is_terminal()
    assert TaskStatus.Cancelled.is_terminal()
    assert TaskStatus.Rejected.is_terminal()


def test_task_lifecycle():
    """Test task status changes."""
    from skreaver import Task, TaskStatus, Message

    task = Task()
    assert not task.is_terminal()

    task.add_message(Message.user("Hello"))
    task.status = TaskStatus.Completed
    assert task.is_terminal()


def test_message_creation():
    """Test creating messages."""
    from skreaver import Message

    user_msg = Message.user("Hello, agent!")
    assert user_msg.role == "user"
    assert user_msg.text == "Hello, agent!"

    agent_msg = Message.agent("Hello, human!")
    assert agent_msg.role == "agent"
    assert agent_msg.text == "Hello, human!"


def test_task_to_dict():
    """Test task serialization to dict."""
    from skreaver import Task

    task = Task("test-id")
    data = task.to_dict()

    assert isinstance(data, dict)
    assert data["id"] == "test-id"
    assert "status" in data


def test_task_from_dict():
    """Test task deserialization from dict."""
    from skreaver import Task

    task1 = Task("test-id")
    data = task1.to_dict()

    task2 = Task.from_dict(data)
    assert task2.id == task1.id


def test_agent_card():
    """Test AgentCard creation."""
    from skreaver.a2a import AgentCard, AgentSkill

    card = AgentCard("my-agent", "My Agent", "https://example.com")
    assert card.agent_id == "my-agent"
    assert card.name == "My Agent"

    # Test builder pattern
    card = card.with_description("A test agent")
    assert card.description == "A test agent"

    card = card.with_streaming()
    assert card.supports_streaming()


def test_agent_skill():
    """Test AgentSkill creation."""
    from skreaver.a2a import AgentSkill

    skill = AgentSkill("search", "Web Search")
    assert skill.id == "search"
    assert skill.name == "Web Search"

    skill = skill.with_description("Search the web")
    assert skill.description == "Search the web"


def test_artifact():
    """Test Artifact creation."""
    from skreaver.a2a import Artifact

    artifact = Artifact.text("Hello, world!")
    assert artifact.id is not None

    artifact = artifact.with_label("greeting")
    assert artifact.label == "greeting"

    artifact = artifact.with_description("A greeting message")
    assert artifact.description == "A greeting message"


def test_part():
    """Test Part creation."""
    from skreaver.a2a import Part

    text_part = Part.text("Hello")
    assert text_part.part_type == "text"

    file_part = Part.file("https://example.com/file.txt", "text/plain")
    assert file_part.part_type == "file"
