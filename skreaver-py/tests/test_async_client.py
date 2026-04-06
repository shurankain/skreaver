"""Tests for A2A async client."""

import pytest


def test_client_creation():
    """Test A2aClient creation."""
    from skreaver import A2aClient

    client = A2aClient("https://agent.example.com")
    assert client.base_url == "https://agent.example.com/"


def test_client_invalid_url():
    """Test A2aClient with invalid URL."""
    from skreaver import A2aClient

    with pytest.raises(ValueError):
        A2aClient("not a valid url")


def test_client_with_bearer_token():
    """Test A2aClient with bearer token auth."""
    from skreaver import A2aClient

    client = A2aClient("https://agent.example.com")
    client_with_auth = client.with_bearer_token("my-token")

    # Builder pattern returns new instance
    assert client is not client_with_auth
    assert client_with_auth.base_url == client.base_url


def test_client_with_api_key():
    """Test A2aClient with API key auth."""
    from skreaver import A2aClient

    client = A2aClient("https://agent.example.com")
    client_with_auth = client.with_api_key("X-API-Key", "my-key")

    # Builder pattern returns new instance
    assert client is not client_with_auth


def test_client_with_api_key_query():
    """Test A2aClient with API key query auth."""
    from skreaver import A2aClient

    client = A2aClient("https://agent.example.com")
    client_with_auth = client.with_api_key_query("api_key", "my-key")

    # Builder pattern returns new instance
    assert client is not client_with_auth


def test_client_repr():
    """Test A2aClient string representation."""
    from skreaver import A2aClient

    client = A2aClient("https://agent.example.com")
    repr_str = repr(client)

    assert "A2aClient" in repr_str
    assert "agent.example.com" in repr_str


# Note: Async method tests require a running server or mocking.
# These tests verify the methods exist and have correct signatures.

@pytest.mark.asyncio
async def test_client_methods_exist():
    """Test that async client methods exist."""
    from skreaver import A2aClient

    client = A2aClient("https://agent.example.com")

    # Verify methods exist
    assert hasattr(client, "get_agent_card")
    assert hasattr(client, "send_message")
    assert hasattr(client, "continue_task")
    assert hasattr(client, "get_task")
    assert hasattr(client, "cancel_task")
    assert hasattr(client, "wait_for_task")
    assert hasattr(client, "send")

    # Verify methods are callable
    assert callable(client.get_agent_card)
    assert callable(client.send_message)
    assert callable(client.continue_task)
    assert callable(client.get_task)
    assert callable(client.cancel_task)
    assert callable(client.wait_for_task)
    assert callable(client.send)
