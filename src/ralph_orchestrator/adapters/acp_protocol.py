# ABOUTME: JSON-RPC 2.0 protocol handler for ACP (Agent Client Protocol)
# ABOUTME: Handles message serialization, parsing, and protocol state

"""JSON-RPC 2.0 protocol handling for ACP."""

import json
from enum import Enum, auto
from typing import Any


class MessageType(Enum):
    """Types of JSON-RPC 2.0 messages."""

    REQUEST = auto()  # Has id and method
    NOTIFICATION = auto()  # Has method but no id
    RESPONSE = auto()  # Has id and result
    ERROR = auto()  # Has id and error
    PARSE_ERROR = auto()  # Failed to parse JSON
    INVALID = auto()  # Invalid JSON-RPC message


class ACPErrorCodes:
    """Standard JSON-RPC 2.0 and ACP-specific error codes."""

    # Standard JSON-RPC 2.0 error codes
    PARSE_ERROR = -32700
    INVALID_REQUEST = -32600
    METHOD_NOT_FOUND = -32601
    INVALID_PARAMS = -32602
    INTERNAL_ERROR = -32603

    # ACP-specific error codes
    PERMISSION_DENIED = -32001
    FILE_NOT_FOUND = -32002
    FILE_ACCESS_ERROR = -32003
    TERMINAL_ERROR = -32004


class ACPProtocol:
    """JSON-RPC 2.0 protocol handler for ACP.

    Handles serialization and deserialization of JSON-RPC messages
    for the Agent Client Protocol.

    Attributes:
        _request_id: Auto-incrementing request ID counter.
    """

    JSONRPC_VERSION = "2.0"

    def __init__(self) -> None:
        """Initialize protocol handler with request ID counter at 0."""
        self._request_id: int = 0

    def create_request(self, method: str, params: dict[str, Any]) -> tuple[int, str]:
        """Create a JSON-RPC 2.0 request message.

        Args:
            method: The RPC method name (e.g., "session/prompt").
            params: The request parameters.

        Returns:
            Tuple of (request_id, json_string) for tracking and sending.
        """
        self._request_id += 1
        request_id = self._request_id

        message = {
            "jsonrpc": self.JSONRPC_VERSION,
            "id": request_id,
            "method": method,
            "params": params,
        }

        return request_id, json.dumps(message)

    def create_notification(self, method: str, params: dict[str, Any]) -> str:
        """Create a JSON-RPC 2.0 notification message (no id, no response expected).

        Args:
            method: The RPC method name.
            params: The notification parameters.

        Returns:
            JSON string of the notification.
        """
        message = {
            "jsonrpc": self.JSONRPC_VERSION,
            "method": method,
            "params": params,
        }

        return json.dumps(message)

    def parse_message(self, data: str) -> dict[str, Any]:
        """Parse an incoming JSON-RPC 2.0 message.

        Determines the message type and validates structure.

        Args:
            data: Raw JSON string to parse.

        Returns:
            Dict with 'type' key indicating MessageType and parsed fields.
            On error, includes 'error' key with description.
        """
        # Try to parse JSON
        try:
            message = json.loads(data)
        except json.JSONDecodeError as e:
            return {
                "type": MessageType.PARSE_ERROR,
                "error": f"JSON parse error: {e}",
            }

        # Validate jsonrpc version field
        if message.get("jsonrpc") != self.JSONRPC_VERSION:
            return {
                "type": MessageType.INVALID,
                "error": f"Invalid or missing jsonrpc field. Expected '2.0', got '{message.get('jsonrpc')}'",
            }

        # Determine message type based on fields
        has_id = "id" in message
        has_method = "method" in message
        has_result = "result" in message
        has_error = "error" in message

        if has_error and has_id:
            # Error response
            return {
                "type": MessageType.ERROR,
                "id": message["id"],
                "error": message["error"],
            }
        elif has_result and has_id:
            # Success response
            return {
                "type": MessageType.RESPONSE,
                "id": message["id"],
                "result": message["result"],
            }
        elif has_method and has_id:
            # Request (has id, expects response)
            return {
                "type": MessageType.REQUEST,
                "id": message["id"],
                "method": message["method"],
                "params": message.get("params", {}),
            }
        elif has_method and not has_id:
            # Notification (no id, no response expected)
            return {
                "type": MessageType.NOTIFICATION,
                "method": message["method"],
                "params": message.get("params", {}),
            }
        else:
            return {
                "type": MessageType.INVALID,
                "error": "Invalid JSON-RPC message structure",
            }

    def create_response(self, request_id: int, result: Any) -> str:
        """Create a JSON-RPC 2.0 success response.

        Args:
            request_id: The ID from the original request.
            result: The result data to return.

        Returns:
            JSON string of the response.
        """
        message = {
            "jsonrpc": self.JSONRPC_VERSION,
            "id": request_id,
            "result": result,
        }

        return json.dumps(message)

    def create_error_response(
        self,
        request_id: int,
        code: int,
        message: str,
        data: Any = None,
    ) -> str:
        """Create a JSON-RPC 2.0 error response.

        Args:
            request_id: The ID from the original request.
            code: The error code (use ACPErrorCodes constants).
            message: Human-readable error message.
            data: Optional additional error data.

        Returns:
            JSON string of the error response.
        """
        error_obj: dict[str, Any] = {
            "code": code,
            "message": message,
        }

        if data is not None:
            error_obj["data"] = data

        response = {
            "jsonrpc": self.JSONRPC_VERSION,
            "id": request_id,
            "error": error_obj,
        }

        return json.dumps(response)
