"""Tests for Gateway types."""

import pytest


def test_protocol_values():
    """Test Protocol enum values."""
    from skreaver import Protocol

    assert Protocol.Mcp is not None
    assert Protocol.A2a is not None
    assert Protocol.Mcp != Protocol.A2a


def test_protocol_gateway_creation():
    """Test ProtocolGateway creation."""
    from skreaver import ProtocolGateway

    gateway = ProtocolGateway()
    assert gateway is not None


def test_protocol_gateway_detect_mcp():
    """Test detecting MCP protocol."""
    from skreaver import ProtocolGateway, Protocol

    gateway = ProtocolGateway()

    mcp_message = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
    }

    detected = gateway.detect(mcp_message)
    assert detected == Protocol.Mcp


def test_protocol_gateway_detect_a2a():
    """Test detecting A2A protocol."""
    from skreaver import ProtocolGateway, Protocol

    gateway = ProtocolGateway()

    a2a_message = {
        "id": "task-123",
        "status": "working",
    }

    detected = gateway.detect(a2a_message)
    assert detected == Protocol.A2a


def test_protocol_gateway_translate():
    """Test protocol translation."""
    from skreaver import ProtocolGateway, Protocol

    gateway = ProtocolGateway()

    mcp_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": "test"},
    }

    result = gateway.translate_to(mcp_request, Protocol.A2a)
    assert isinstance(result, dict)


def test_protocol_detector():
    """Test ProtocolDetector."""
    from skreaver.gateway import ProtocolDetector, Protocol

    detector = ProtocolDetector()

    mcp_message = {"jsonrpc": "2.0", "method": "ping"}
    assert detector.detect(mcp_message) == Protocol.Mcp

    a2a_message = {"id": "task-1", "status": "completed"}
    assert detector.detect(a2a_message) == Protocol.A2a


def test_protocol_detector_strict():
    """Test strict ProtocolDetector."""
    from skreaver.gateway import ProtocolDetector

    detector = ProtocolDetector.strict()
    assert detector is not None


def test_protocol_detector_json_string():
    """Test detecting protocol from JSON string."""
    from skreaver.gateway import ProtocolDetector, Protocol

    detector = ProtocolDetector()

    mcp_json = '{"jsonrpc": "2.0", "method": "ping"}'
    assert detector.detect_str(mcp_json) == Protocol.Mcp
