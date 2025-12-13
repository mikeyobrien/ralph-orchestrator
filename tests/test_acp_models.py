# ABOUTME: Unit tests for ACP data models
# ABOUTME: Tests dataclass creation, from_dict parsing, and session state accumulation

"""Tests for ACP data models."""

import pytest
from typing import Any

from ralph_orchestrator.adapters.acp_models import (
    ACPRequest,
    ACPNotification,
    ACPResponse,
    ACPError,
    ACPErrorObject,
    SessionUpdate,
    UpdatePayload,
    ToolCall,
    ACPSession,
    ACPAdapterConfig,
)


class TestACPRequest:
    """Tests for ACPRequest dataclass."""

    def test_create_request(self):
        """ACPRequest can be created with required fields."""
        request = ACPRequest(
            id=1,
            method="session/prompt",
            params={"sessionId": "abc123"},
        )
        assert request.id == 1
        assert request.method == "session/prompt"
        assert request.params == {"sessionId": "abc123"}

    def test_from_dict_valid(self):
        """ACPRequest.from_dict parses valid dict."""
        data = {
            "id": 5,
            "method": "initialize",
            "params": {"version": "1.0"},
        }
        request = ACPRequest.from_dict(data)
        assert request.id == 5
        assert request.method == "initialize"
        assert request.params == {"version": "1.0"}

    def test_from_dict_missing_params(self):
        """ACPRequest.from_dict defaults params to empty dict."""
        data = {"id": 1, "method": "test"}
        request = ACPRequest.from_dict(data)
        assert request.params == {}

    def test_from_dict_invalid_missing_id(self):
        """ACPRequest.from_dict raises on missing id."""
        data = {"method": "test", "params": {}}
        with pytest.raises(KeyError):
            ACPRequest.from_dict(data)

    def test_from_dict_invalid_missing_method(self):
        """ACPRequest.from_dict raises on missing method."""
        data = {"id": 1, "params": {}}
        with pytest.raises(KeyError):
            ACPRequest.from_dict(data)


class TestACPNotification:
    """Tests for ACPNotification dataclass."""

    def test_create_notification(self):
        """ACPNotification can be created with required fields."""
        notification = ACPNotification(
            method="session/update",
            params={"kind": "agent_message_chunk"},
        )
        assert notification.method == "session/update"
        assert notification.params == {"kind": "agent_message_chunk"}

    def test_from_dict_valid(self):
        """ACPNotification.from_dict parses valid dict."""
        data = {
            "method": "session/update",
            "params": {"kind": "tool_call", "toolName": "read_file"},
        }
        notification = ACPNotification.from_dict(data)
        assert notification.method == "session/update"
        assert notification.params["toolName"] == "read_file"

    def test_from_dict_missing_params(self):
        """ACPNotification.from_dict defaults params to empty dict."""
        data = {"method": "session/cancel"}
        notification = ACPNotification.from_dict(data)
        assert notification.params == {}


class TestACPResponse:
    """Tests for ACPResponse dataclass."""

    def test_create_response(self):
        """ACPResponse can be created with required fields."""
        response = ACPResponse(
            id=1,
            result={"sessionId": "abc123"},
        )
        assert response.id == 1
        assert response.result == {"sessionId": "abc123"}

    def test_from_dict_valid(self):
        """ACPResponse.from_dict parses valid dict."""
        data = {
            "id": 10,
            "result": {"content": "Hello world"},
        }
        response = ACPResponse.from_dict(data)
        assert response.id == 10
        assert response.result["content"] == "Hello world"

    def test_from_dict_null_result(self):
        """ACPResponse.from_dict handles null result."""
        data = {"id": 1, "result": None}
        response = ACPResponse.from_dict(data)
        assert response.result is None


class TestACPErrorObject:
    """Tests for ACPErrorObject dataclass."""

    def test_create_error_object(self):
        """ACPErrorObject can be created with required fields."""
        error_obj = ACPErrorObject(
            code=-32600,
            message="Invalid Request",
        )
        assert error_obj.code == -32600
        assert error_obj.message == "Invalid Request"
        assert error_obj.data is None

    def test_create_with_data(self):
        """ACPErrorObject can include optional data field."""
        error_obj = ACPErrorObject(
            code=-32602,
            message="Invalid params",
            data={"field": "sessionId"},
        )
        assert error_obj.data == {"field": "sessionId"}

    def test_from_dict_valid(self):
        """ACPErrorObject.from_dict parses valid dict."""
        data = {"code": -32601, "message": "Method not found"}
        error_obj = ACPErrorObject.from_dict(data)
        assert error_obj.code == -32601
        assert error_obj.message == "Method not found"


class TestACPError:
    """Tests for ACPError dataclass."""

    def test_create_error(self):
        """ACPError can be created with required fields."""
        error_obj = ACPErrorObject(code=-32600, message="Error")
        error = ACPError(id=1, error=error_obj)
        assert error.id == 1
        assert error.error.code == -32600

    def test_from_dict_valid(self):
        """ACPError.from_dict parses valid dict."""
        data = {
            "id": 3,
            "error": {"code": -32001, "message": "Permission denied"},
        }
        error = ACPError.from_dict(data)
        assert error.id == 3
        assert error.error.code == -32001
        assert error.error.message == "Permission denied"


class TestUpdatePayload:
    """Tests for UpdatePayload dataclass."""

    def test_create_message_chunk_payload(self):
        """UpdatePayload for agent_message_chunk."""
        payload = UpdatePayload(
            kind="agent_message_chunk",
            content="Hello",
        )
        assert payload.kind == "agent_message_chunk"
        assert payload.content == "Hello"

    def test_create_tool_call_payload(self):
        """UpdatePayload for tool_call."""
        payload = UpdatePayload(
            kind="tool_call",
            tool_name="read_file",
            tool_call_id="call_123",
            arguments={"path": "/test.txt"},
        )
        assert payload.kind == "tool_call"
        assert payload.tool_name == "read_file"
        assert payload.tool_call_id == "call_123"

    def test_from_dict_message_chunk(self):
        """UpdatePayload.from_dict parses message chunk."""
        data = {"kind": "agent_message_chunk", "content": "World"}
        payload = UpdatePayload.from_dict(data)
        assert payload.kind == "agent_message_chunk"
        assert payload.content == "World"

    def test_from_dict_tool_call(self):
        """UpdatePayload.from_dict parses tool call."""
        data = {
            "kind": "tool_call",
            "toolName": "write_file",
            "toolCallId": "id_456",
            "arguments": {"path": "/out.txt", "content": "data"},
        }
        payload = UpdatePayload.from_dict(data)
        assert payload.kind == "tool_call"
        assert payload.tool_name == "write_file"
        assert payload.tool_call_id == "id_456"
        assert payload.arguments["content"] == "data"

    def test_from_dict_thought_chunk(self):
        """UpdatePayload.from_dict parses thought chunk."""
        data = {"kind": "agent_thought_chunk", "content": "I should..."}
        payload = UpdatePayload.from_dict(data)
        assert payload.kind == "agent_thought_chunk"
        assert payload.content == "I should..."


class TestSessionUpdate:
    """Tests for SessionUpdate dataclass."""

    def test_create_session_update(self):
        """SessionUpdate wraps a method and payload."""
        payload = UpdatePayload(kind="agent_message_chunk", content="Hi")
        update = SessionUpdate(method="session/update", payload=payload)
        assert update.method == "session/update"
        assert update.payload.content == "Hi"

    def test_from_dict_valid(self):
        """SessionUpdate.from_dict parses valid dict."""
        data = {
            "method": "session/update",
            "params": {"kind": "agent_message_chunk", "content": "test"},
        }
        update = SessionUpdate.from_dict(data)
        assert update.method == "session/update"
        assert update.payload.kind == "agent_message_chunk"


class TestToolCall:
    """Tests for ToolCall dataclass."""

    def test_create_tool_call(self):
        """ToolCall can be created with required fields."""
        tool_call = ToolCall(
            tool_call_id="call_abc",
            tool_name="read_file",
            arguments={"path": "/file.txt"},
        )
        assert tool_call.tool_call_id == "call_abc"
        assert tool_call.tool_name == "read_file"
        assert tool_call.status == "pending"

    def test_tool_call_default_status(self):
        """ToolCall defaults to pending status."""
        tool_call = ToolCall(
            tool_call_id="id",
            tool_name="test",
            arguments={},
        )
        assert tool_call.status == "pending"
        assert tool_call.result is None
        assert tool_call.error is None

    def test_from_dict_valid(self):
        """ToolCall.from_dict parses valid dict."""
        data = {
            "toolCallId": "call_xyz",
            "toolName": "execute_command",
            "arguments": {"command": "ls"},
        }
        tool_call = ToolCall.from_dict(data)
        assert tool_call.tool_call_id == "call_xyz"
        assert tool_call.tool_name == "execute_command"

    def test_tool_call_update_status(self):
        """ToolCall status can be updated."""
        tool_call = ToolCall(
            tool_call_id="id",
            tool_name="test",
            arguments={},
        )
        tool_call.status = "completed"
        tool_call.result = {"success": True}
        assert tool_call.status == "completed"
        assert tool_call.result == {"success": True}


class TestACPSession:
    """Tests for ACPSession session state accumulation."""

    def test_create_session(self):
        """ACPSession can be created with session_id."""
        session = ACPSession(session_id="sess_123")
        assert session.session_id == "sess_123"
        assert session.output == ""
        assert session.thoughts == ""
        assert session.tool_calls == []

    def test_append_output(self):
        """ACPSession accumulates output chunks."""
        session = ACPSession(session_id="test")
        session.append_output("Hello ")
        session.append_output("World")
        assert session.output == "Hello World"

    def test_append_thought(self):
        """ACPSession accumulates thought chunks."""
        session = ACPSession(session_id="test")
        session.append_thought("I need to ")
        session.append_thought("read the file.")
        assert session.thoughts == "I need to read the file."

    def test_add_tool_call(self):
        """ACPSession tracks tool calls."""
        session = ACPSession(session_id="test")
        tool_call = ToolCall(
            tool_call_id="call_1",
            tool_name="read_file",
            arguments={"path": "/a.txt"},
        )
        session.add_tool_call(tool_call)
        assert len(session.tool_calls) == 1
        assert session.tool_calls[0].tool_name == "read_file"

    def test_get_tool_call_by_id(self):
        """ACPSession can retrieve tool call by ID."""
        session = ACPSession(session_id="test")
        tc1 = ToolCall("id1", "tool1", {})
        tc2 = ToolCall("id2", "tool2", {})
        session.add_tool_call(tc1)
        session.add_tool_call(tc2)

        result = session.get_tool_call("id2")
        assert result is not None
        assert result.tool_name == "tool2"

    def test_get_tool_call_not_found(self):
        """ACPSession.get_tool_call returns None for unknown ID."""
        session = ACPSession(session_id="test")
        assert session.get_tool_call("unknown") is None

    def test_process_update_message_chunk(self):
        """ACPSession.process_update handles message chunks."""
        session = ACPSession(session_id="test")
        payload = UpdatePayload(kind="agent_message_chunk", content="Hi!")
        session.process_update(payload)
        assert session.output == "Hi!"

    def test_process_update_thought_chunk(self):
        """ACPSession.process_update handles thought chunks."""
        session = ACPSession(session_id="test")
        payload = UpdatePayload(kind="agent_thought_chunk", content="Thinking...")
        session.process_update(payload)
        assert session.thoughts == "Thinking..."

    def test_process_update_tool_call(self):
        """ACPSession.process_update handles tool calls."""
        session = ACPSession(session_id="test")
        payload = UpdatePayload(
            kind="tool_call",
            tool_name="read_file",
            tool_call_id="call_1",
            arguments={"path": "/x.txt"},
        )
        session.process_update(payload)
        assert len(session.tool_calls) == 1
        assert session.tool_calls[0].tool_name == "read_file"

    def test_process_update_tool_call_update(self):
        """ACPSession.process_update handles tool call updates."""
        session = ACPSession(session_id="test")
        # First add a tool call
        tc = ToolCall("call_1", "test_tool", {})
        session.add_tool_call(tc)

        # Then process an update for it
        payload = UpdatePayload(
            kind="tool_call_update",
            tool_call_id="call_1",
            status="completed",
            result={"data": "output"},
        )
        session.process_update(payload)

        updated_tc = session.get_tool_call("call_1")
        assert updated_tc.status == "completed"
        assert updated_tc.result == {"data": "output"}

    def test_reset_session(self):
        """ACPSession can be reset for a new prompt."""
        session = ACPSession(session_id="test")
        session.append_output("text")
        session.append_thought("thought")
        session.add_tool_call(ToolCall("id", "tool", {}))

        session.reset()

        assert session.output == ""
        assert session.thoughts == ""
        assert session.tool_calls == []
        assert session.session_id == "test"  # ID preserved


class TestACPAdapterConfig:
    """Tests for ACPAdapterConfig dataclass."""

    def test_create_config_defaults(self):
        """ACPAdapterConfig has sensible defaults."""
        config = ACPAdapterConfig()
        assert config.agent_command == "gemini"
        assert config.agent_args == []
        assert config.timeout == 300
        assert config.permission_mode == "auto_approve"
        assert config.permission_allowlist == []

    def test_create_config_custom(self):
        """ACPAdapterConfig accepts custom values."""
        config = ACPAdapterConfig(
            agent_command="claude",
            agent_args=["--model", "opus"],
            timeout=600,
            permission_mode="deny_all",
        )
        assert config.agent_command == "claude"
        assert config.agent_args == ["--model", "opus"]
        assert config.timeout == 600
        assert config.permission_mode == "deny_all"

    def test_from_dict_valid(self):
        """ACPAdapterConfig.from_dict parses valid dict."""
        data = {
            "agent_command": "custom_agent",
            "timeout": 120,
            "permission_mode": "allowlist",
            "permission_allowlist": ["fs/*", "terminal/*"],
        }
        config = ACPAdapterConfig.from_dict(data)
        assert config.agent_command == "custom_agent"
        assert config.timeout == 120
        assert config.permission_mode == "allowlist"
        assert "fs/*" in config.permission_allowlist

    def test_from_dict_partial(self):
        """ACPAdapterConfig.from_dict uses defaults for missing keys."""
        data = {"timeout": 60}
        config = ACPAdapterConfig.from_dict(data)
        assert config.timeout == 60
        assert config.agent_command == "gemini"  # default
        assert config.permission_mode == "auto_approve"  # default

    def test_permission_modes_valid(self):
        """ACPAdapterConfig accepts all valid permission modes."""
        for mode in ["auto_approve", "deny_all", "allowlist", "interactive"]:
            config = ACPAdapterConfig(permission_mode=mode)
            assert config.permission_mode == mode
