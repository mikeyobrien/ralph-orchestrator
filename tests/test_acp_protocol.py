# ABOUTME: Unit tests for ACPProtocol JSON-RPC 2.0 handling
# ABOUTME: Tests request/notification creation, message parsing, and error responses

"""Tests for ACP Protocol JSON-RPC 2.0 handling."""

import json
import pytest

from ralph_orchestrator.adapters.acp_protocol import (
    ACPProtocol,
    ACPErrorCodes,
    MessageType,
)


class TestACPProtocolRequestCreation:
    """Tests for JSON-RPC request creation."""

    def test_create_request_returns_id_and_json_string(self):
        """create_request returns (id, json_string) tuple."""
        protocol = ACPProtocol()
        request_id, json_str = protocol.create_request("initialize", {"version": "1.0"})

        assert isinstance(request_id, int)
        assert isinstance(json_str, str)

    def test_create_request_increments_id(self):
        """Each request gets a unique incrementing ID."""
        protocol = ACPProtocol()

        id1, _ = protocol.create_request("method1", {})
        id2, _ = protocol.create_request("method2", {})
        id3, _ = protocol.create_request("method3", {})

        assert id2 == id1 + 1
        assert id3 == id2 + 1

    def test_create_request_includes_jsonrpc_version(self):
        """Request includes jsonrpc: 2.0 field."""
        protocol = ACPProtocol()
        _, json_str = protocol.create_request("test", {})

        data = json.loads(json_str)
        assert data["jsonrpc"] == "2.0"

    def test_create_request_includes_method(self):
        """Request includes method field."""
        protocol = ACPProtocol()
        _, json_str = protocol.create_request("session/prompt", {})

        data = json.loads(json_str)
        assert data["method"] == "session/prompt"

    def test_create_request_includes_params(self):
        """Request includes params field."""
        protocol = ACPProtocol()
        params = {"sessionId": "abc123", "messages": []}
        _, json_str = protocol.create_request("session/prompt", params)

        data = json.loads(json_str)
        assert data["params"] == params

    def test_create_request_includes_id(self):
        """Request includes id field."""
        protocol = ACPProtocol()
        request_id, json_str = protocol.create_request("test", {})

        data = json.loads(json_str)
        assert data["id"] == request_id

    def test_create_request_with_complex_params(self):
        """Request handles complex nested params."""
        protocol = ACPProtocol()
        params = {
            "sessionId": "session123",
            "messages": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi there!"},
            ],
            "options": {"temperature": 0.7, "maxTokens": 1000},
        }
        _, json_str = protocol.create_request("session/prompt", params)

        data = json.loads(json_str)
        assert data["params"] == params


class TestACPProtocolNotificationCreation:
    """Tests for JSON-RPC notification creation."""

    def test_create_notification_returns_json_string(self):
        """create_notification returns json string."""
        protocol = ACPProtocol()
        json_str = protocol.create_notification("session/update", {"kind": "message"})

        assert isinstance(json_str, str)

    def test_create_notification_has_no_id(self):
        """Notification must NOT have id field."""
        protocol = ACPProtocol()
        json_str = protocol.create_notification("session/update", {})

        data = json.loads(json_str)
        assert "id" not in data

    def test_create_notification_includes_jsonrpc_version(self):
        """Notification includes jsonrpc: 2.0 field."""
        protocol = ACPProtocol()
        json_str = protocol.create_notification("test", {})

        data = json.loads(json_str)
        assert data["jsonrpc"] == "2.0"

    def test_create_notification_includes_method(self):
        """Notification includes method field."""
        protocol = ACPProtocol()
        json_str = protocol.create_notification("session/cancel", {})

        data = json.loads(json_str)
        assert data["method"] == "session/cancel"

    def test_create_notification_includes_params(self):
        """Notification includes params field."""
        protocol = ACPProtocol()
        params = {"sessionId": "abc123"}
        json_str = protocol.create_notification("session/cancel", params)

        data = json.loads(json_str)
        assert data["params"] == params


class TestACPProtocolMessageParsing:
    """Tests for JSON-RPC message parsing."""

    def test_parse_request_message(self):
        """Parse valid request message."""
        protocol = ACPProtocol()
        json_str = json.dumps({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "fs/read_text_file",
            "params": {"path": "/test.txt"},
        })

        result = protocol.parse_message(json_str)

        assert result["type"] == MessageType.REQUEST
        assert result["id"] == 1
        assert result["method"] == "fs/read_text_file"
        assert result["params"] == {"path": "/test.txt"}

    def test_parse_notification_message(self):
        """Parse notification (no id field)."""
        protocol = ACPProtocol()
        json_str = json.dumps({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {"kind": "agent_message_chunk", "content": "Hello"},
        })

        result = protocol.parse_message(json_str)

        assert result["type"] == MessageType.NOTIFICATION
        assert "id" not in result or result.get("id") is None
        assert result["method"] == "session/update"
        assert result["params"]["kind"] == "agent_message_chunk"

    def test_parse_response_message(self):
        """Parse response with result."""
        protocol = ACPProtocol()
        json_str = json.dumps({
            "jsonrpc": "2.0",
            "id": 5,
            "result": {"sessionId": "abc123"},
        })

        result = protocol.parse_message(json_str)

        assert result["type"] == MessageType.RESPONSE
        assert result["id"] == 5
        assert result["result"] == {"sessionId": "abc123"}

    def test_parse_error_response_message(self):
        """Parse error response."""
        protocol = ACPProtocol()
        json_str = json.dumps({
            "jsonrpc": "2.0",
            "id": 3,
            "error": {
                "code": -32601,
                "message": "Method not found",
            },
        })

        result = protocol.parse_message(json_str)

        assert result["type"] == MessageType.ERROR
        assert result["id"] == 3
        assert result["error"]["code"] == -32601
        assert result["error"]["message"] == "Method not found"

    def test_parse_invalid_json(self):
        """Parse invalid JSON returns parse error."""
        protocol = ACPProtocol()

        result = protocol.parse_message("not valid json{")

        assert result["type"] == MessageType.PARSE_ERROR
        assert "error" in result

    def test_parse_missing_jsonrpc_field(self):
        """Parse message without jsonrpc field returns error."""
        protocol = ACPProtocol()
        json_str = json.dumps({"id": 1, "method": "test", "params": {}})

        result = protocol.parse_message(json_str)

        assert result["type"] == MessageType.INVALID
        assert "error" in result

    def test_parse_wrong_jsonrpc_version(self):
        """Parse message with wrong jsonrpc version returns error."""
        protocol = ACPProtocol()
        json_str = json.dumps({
            "jsonrpc": "1.0",
            "id": 1,
            "method": "test",
            "params": {},
        })

        result = protocol.parse_message(json_str)

        assert result["type"] == MessageType.INVALID
        assert "error" in result


class TestACPProtocolResponseCreation:
    """Tests for JSON-RPC response creation."""

    def test_create_response_includes_jsonrpc_version(self):
        """Response includes jsonrpc: 2.0 field."""
        protocol = ACPProtocol()
        json_str = protocol.create_response(1, {"success": True})

        data = json.loads(json_str)
        assert data["jsonrpc"] == "2.0"

    def test_create_response_includes_id(self):
        """Response includes matching id."""
        protocol = ACPProtocol()
        json_str = protocol.create_response(42, {})

        data = json.loads(json_str)
        assert data["id"] == 42

    def test_create_response_includes_result(self):
        """Response includes result field."""
        protocol = ACPProtocol()
        result = {"content": "file contents here", "encoding": "utf-8"}
        json_str = protocol.create_response(1, result)

        data = json.loads(json_str)
        assert data["result"] == result

    def test_create_response_no_error_field(self):
        """Successful response does not include error field."""
        protocol = ACPProtocol()
        json_str = protocol.create_response(1, {"ok": True})

        data = json.loads(json_str)
        assert "error" not in data


class TestACPProtocolErrorResponseCreation:
    """Tests for JSON-RPC error response creation."""

    def test_create_error_response_includes_jsonrpc_version(self):
        """Error response includes jsonrpc: 2.0 field."""
        protocol = ACPProtocol()
        json_str = protocol.create_error_response(1, -32600, "Invalid Request")

        data = json.loads(json_str)
        assert data["jsonrpc"] == "2.0"

    def test_create_error_response_includes_id(self):
        """Error response includes matching id."""
        protocol = ACPProtocol()
        json_str = protocol.create_error_response(99, -32600, "Invalid Request")

        data = json.loads(json_str)
        assert data["id"] == 99

    def test_create_error_response_includes_error_object(self):
        """Error response includes error object with code and message."""
        protocol = ACPProtocol()
        json_str = protocol.create_error_response(
            1, ACPErrorCodes.METHOD_NOT_FOUND, "Method not found"
        )

        data = json.loads(json_str)
        assert "error" in data
        assert data["error"]["code"] == ACPErrorCodes.METHOD_NOT_FOUND
        assert data["error"]["message"] == "Method not found"

    def test_create_error_response_no_result_field(self):
        """Error response does not include result field."""
        protocol = ACPProtocol()
        json_str = protocol.create_error_response(1, -32600, "Error")

        data = json.loads(json_str)
        assert "result" not in data

    def test_create_error_response_with_data(self):
        """Error response can include optional data field."""
        protocol = ACPProtocol()
        json_str = protocol.create_error_response(
            1,
            ACPErrorCodes.INVALID_PARAMS,
            "Missing required field",
            data={"field": "sessionId"},
        )

        data = json.loads(json_str)
        assert data["error"]["data"] == {"field": "sessionId"}

    def test_standard_json_rpc_error_codes(self):
        """Verify standard JSON-RPC error codes are defined."""
        assert ACPErrorCodes.PARSE_ERROR == -32700
        assert ACPErrorCodes.INVALID_REQUEST == -32600
        assert ACPErrorCodes.METHOD_NOT_FOUND == -32601
        assert ACPErrorCodes.INVALID_PARAMS == -32602
        assert ACPErrorCodes.INTERNAL_ERROR == -32603

    def test_acp_specific_error_codes(self):
        """Verify ACP-specific error codes are defined."""
        assert ACPErrorCodes.PERMISSION_DENIED == -32001
        assert ACPErrorCodes.FILE_NOT_FOUND == -32002
        assert ACPErrorCodes.FILE_ACCESS_ERROR == -32003
        assert ACPErrorCodes.TERMINAL_ERROR == -32004


class TestACPProtocolRoundTrip:
    """Tests for round-trip serialization."""

    def test_request_roundtrip(self):
        """Request created by protocol can be parsed back."""
        protocol = ACPProtocol()
        request_id, json_str = protocol.create_request(
            "session/prompt",
            {"sessionId": "test", "messages": [{"role": "user", "content": "Hi"}]},
        )

        parsed = protocol.parse_message(json_str)

        assert parsed["type"] == MessageType.REQUEST
        assert parsed["id"] == request_id
        assert parsed["method"] == "session/prompt"
        assert parsed["params"]["sessionId"] == "test"

    def test_notification_roundtrip(self):
        """Notification created by protocol can be parsed back."""
        protocol = ACPProtocol()
        json_str = protocol.create_notification(
            "session/cancel", {"sessionId": "test123"}
        )

        parsed = protocol.parse_message(json_str)

        assert parsed["type"] == MessageType.NOTIFICATION
        assert parsed["method"] == "session/cancel"
        assert parsed["params"]["sessionId"] == "test123"

    def test_response_roundtrip(self):
        """Response created by protocol can be parsed back."""
        protocol = ACPProtocol()
        json_str = protocol.create_response(5, {"content": "Hello world"})

        parsed = protocol.parse_message(json_str)

        assert parsed["type"] == MessageType.RESPONSE
        assert parsed["id"] == 5
        assert parsed["result"]["content"] == "Hello world"

    def test_error_response_roundtrip(self):
        """Error response created by protocol can be parsed back."""
        protocol = ACPProtocol()
        json_str = protocol.create_error_response(
            7, ACPErrorCodes.FILE_NOT_FOUND, "File not found: /test.txt"
        )

        parsed = protocol.parse_message(json_str)

        assert parsed["type"] == MessageType.ERROR
        assert parsed["id"] == 7
        assert parsed["error"]["code"] == ACPErrorCodes.FILE_NOT_FOUND
