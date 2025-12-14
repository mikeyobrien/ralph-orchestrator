# ABOUTME: Typed dataclasses for ACP (Agent Client Protocol) messages
# ABOUTME: Defines request/response types, session state, and configuration

"""Typed data models for ACP messages and session state."""

from __future__ import annotations

import logging
import os
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from ralph_orchestrator.main import AdapterConfig

# Get logger for this module
_logger = logging.getLogger(__name__)


class UpdateKind(str, Enum):
    """Types of session updates."""

    AGENT_MESSAGE_CHUNK = "agent_message_chunk"
    AGENT_THOUGHT_CHUNK = "agent_thought_chunk"
    TOOL_CALL = "tool_call"
    TOOL_CALL_UPDATE = "tool_call_update"
    PLAN = "plan"


class ToolCallStatus(str, Enum):
    """Status of a tool call."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"


class PermissionMode(str, Enum):
    """Permission modes for ACP operations."""

    AUTO_APPROVE = "auto_approve"
    DENY_ALL = "deny_all"
    ALLOWLIST = "allowlist"
    INTERACTIVE = "interactive"


@dataclass
class ACPRequest:
    """JSON-RPC 2.0 request message.

    Attributes:
        id: Request identifier for matching response.
        method: The RPC method to invoke.
        params: Method parameters.
    """

    id: int
    method: str
    params: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ACPRequest":
        """Parse ACPRequest from dict.

        Args:
            data: Dict with id, method, and optional params.

        Returns:
            ACPRequest instance.

        Raises:
            KeyError: If id or method is missing.
        """
        return cls(
            id=data["id"],
            method=data["method"],
            params=data.get("params", {}),
        )


@dataclass
class ACPNotification:
    """JSON-RPC 2.0 notification (no response expected).

    Attributes:
        method: The notification method.
        params: Method parameters.
    """

    method: str
    params: dict[str, Any] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ACPNotification":
        """Parse ACPNotification from dict.

        Args:
            data: Dict with method and optional params.

        Returns:
            ACPNotification instance.
        """
        return cls(
            method=data["method"],
            params=data.get("params", {}),
        )


@dataclass
class ACPResponse:
    """JSON-RPC 2.0 success response.

    Attributes:
        id: Request identifier this response matches.
        result: The response result data.
    """

    id: int
    result: Any

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ACPResponse":
        """Parse ACPResponse from dict.

        Args:
            data: Dict with id and result.

        Returns:
            ACPResponse instance.
        """
        return cls(
            id=data["id"],
            result=data.get("result"),
        )


@dataclass
class ACPErrorObject:
    """JSON-RPC 2.0 error object.

    Attributes:
        code: Error code (negative integers for standard errors).
        message: Human-readable error message.
        data: Optional additional error data.
    """

    code: int
    message: str
    data: Any = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ACPErrorObject":
        """Parse ACPErrorObject from dict.

        Args:
            data: Dict with code, message, and optional data.

        Returns:
            ACPErrorObject instance.
        """
        return cls(
            code=data["code"],
            message=data["message"],
            data=data.get("data"),
        )


@dataclass
class ACPError:
    """JSON-RPC 2.0 error response.

    Attributes:
        id: Request identifier this error matches.
        error: The error object with code and message.
    """

    id: int
    error: ACPErrorObject

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ACPError":
        """Parse ACPError from dict.

        Args:
            data: Dict with id and error object.

        Returns:
            ACPError instance.
        """
        return cls(
            id=data["id"],
            error=ACPErrorObject.from_dict(data["error"]),
        )


@dataclass
class UpdatePayload:
    """Payload for session/update notifications.

    Handles different update kinds:
    - agent_message_chunk: Text output from agent
    - agent_thought_chunk: Internal reasoning (verbose mode)
    - tool_call: Agent requesting tool execution
    - tool_call_update: Status update for tool call
    - plan: Agent's execution plan

    Attributes:
        kind: The update type (see UpdateKind enum for valid values).
        content: Text content (for message/thought chunks).
        tool_name: Name of tool being called.
        tool_call_id: Unique identifier for tool call.
        arguments: Tool call arguments.
        status: Tool call status (see ToolCallStatus enum for valid values).
        result: Tool call result data.
        error: Tool call error message.
    """

    kind: str  # Valid values: UpdateKind enum members
    content: Optional[str] = None
    tool_name: Optional[str] = None
    tool_call_id: Optional[str] = None
    arguments: Optional[dict[str, Any]] = None
    status: Optional[str] = None
    result: Optional[Any] = None
    error: Optional[str] = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "UpdatePayload":
        """Parse UpdatePayload from dict.

        Handles camelCase to snake_case conversion for ACP fields.

        Args:
            data: Dict with kind and kind-specific fields.

        Returns:
            UpdatePayload instance.
        """
        return cls(
            kind=data["kind"],
            content=data.get("content"),
            tool_name=data.get("toolName"),
            tool_call_id=data.get("toolCallId"),
            arguments=data.get("arguments"),
            status=data.get("status"),
            result=data.get("result"),
            error=data.get("error"),
        )


@dataclass
class SessionUpdate:
    """Wrapper for session/update notification.

    Attributes:
        method: Should be "session/update".
        payload: The update payload.
    """

    method: str
    payload: UpdatePayload

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "SessionUpdate":
        """Parse SessionUpdate from notification dict.

        Args:
            data: Dict with method and params.

        Returns:
            SessionUpdate instance.
        """
        return cls(
            method=data["method"],
            payload=UpdatePayload.from_dict(data["params"]),
        )


@dataclass
class ToolCall:
    """Tracks a tool execution within a session.

    Attributes:
        tool_call_id: Unique identifier for this call.
        tool_name: Name of the tool being called.
        arguments: Arguments passed to the tool.
        status: Current status (pending/running/completed/failed).
        result: Result data if completed.
        error: Error message if failed.
    """

    tool_call_id: str
    tool_name: str
    arguments: dict[str, Any]
    status: str = "pending"
    result: Optional[Any] = None
    error: Optional[str] = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ToolCall":
        """Parse ToolCall from dict.

        Args:
            data: Dict with toolCallId, toolName, arguments.

        Returns:
            ToolCall instance.
        """
        return cls(
            tool_call_id=data["toolCallId"],
            tool_name=data["toolName"],
            arguments=data.get("arguments", {}),
            status=data.get("status", "pending"),
            result=data.get("result"),
            error=data.get("error"),
        )


@dataclass
class ACPSession:
    """Accumulates session state during prompt execution.

    Tracks output chunks, thoughts, and tool calls for building
    the final response.

    Attributes:
        session_id: Unique session identifier.
        output: Accumulated agent output text.
        thoughts: Accumulated agent thoughts (verbose).
        tool_calls: List of tool calls in this session.
    """

    session_id: str
    output: str = ""
    thoughts: str = ""
    tool_calls: list[ToolCall] = field(default_factory=list)

    def append_output(self, text: str) -> None:
        """Append text to accumulated output.

        Args:
            text: Text chunk to append.
        """
        self.output += text

    def append_thought(self, text: str) -> None:
        """Append text to accumulated thoughts.

        Args:
            text: Thought chunk to append.
        """
        self.thoughts += text

    def add_tool_call(self, tool_call: ToolCall) -> None:
        """Add a new tool call to track.

        Args:
            tool_call: The ToolCall to track.
        """
        self.tool_calls.append(tool_call)

    def get_tool_call(self, tool_call_id: str) -> Optional[ToolCall]:
        """Find a tool call by ID.

        Args:
            tool_call_id: The ID to look up.

        Returns:
            ToolCall if found, None otherwise.
        """
        for tc in self.tool_calls:
            if tc.tool_call_id == tool_call_id:
                return tc
        return None

    def process_update(self, payload: UpdatePayload) -> None:
        """Process a session update payload.

        Routes update to appropriate handler based on kind.

        Args:
            payload: The update payload to process.
        """
        if payload.kind == "agent_message_chunk":
            if payload.content:
                self.append_output(payload.content)
        elif payload.kind == "agent_thought_chunk":
            if payload.content:
                self.append_thought(payload.content)
        elif payload.kind == "tool_call":
            tool_call = ToolCall(
                tool_call_id=payload.tool_call_id or "",
                tool_name=payload.tool_name or "",
                arguments=payload.arguments or {},
            )
            self.add_tool_call(tool_call)
        elif payload.kind == "tool_call_update":
            if payload.tool_call_id:
                tc = self.get_tool_call(payload.tool_call_id)
                if tc:
                    if payload.status:
                        tc.status = payload.status
                    if payload.result is not None:
                        tc.result = payload.result
                    if payload.error:
                        tc.error = payload.error

    def reset(self) -> None:
        """Reset session state for a new prompt.

        Preserves session_id but clears accumulated data.
        """
        self.output = ""
        self.thoughts = ""
        self.tool_calls = []


@dataclass
class ACPAdapterConfig:
    """Configuration for the ACP adapter.

    Attributes:
        agent_command: Command to spawn the agent (default: gemini).
        agent_args: Additional arguments for agent command.
        timeout: Request timeout in seconds.
        permission_mode: How to handle permission requests.
            - auto_approve: Approve all requests.
            - deny_all: Deny all requests.
            - allowlist: Check against permission_allowlist.
            - interactive: Prompt user for each request.
        permission_allowlist: Patterns to allow in allowlist mode.
    """

    agent_command: str = "gemini"
    agent_args: list[str] = field(default_factory=list)
    timeout: int = 300
    permission_mode: str = "auto_approve"
    permission_allowlist: list[str] = field(default_factory=list)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "ACPAdapterConfig":
        """Parse ACPAdapterConfig from dict.

        Uses defaults for missing keys.

        Args:
            data: Configuration dict.

        Returns:
            ACPAdapterConfig instance.
        """
        return cls(
            agent_command=data.get("agent_command", "gemini"),
            agent_args=data.get("agent_args", []),
            timeout=data.get("timeout", 300),
            permission_mode=data.get("permission_mode", "auto_approve"),
            permission_allowlist=data.get("permission_allowlist", []),
        )

    @classmethod
    def from_adapter_config(cls, adapter_config: "AdapterConfig") -> "ACPAdapterConfig":
        """Create ACPAdapterConfig from AdapterConfig with env var overrides.

        Extracts ACP-specific settings from AdapterConfig.tool_permissions
        and applies environment variable overrides.

        Environment Variables:
            RALPH_ACP_AGENT: Override agent_command
            RALPH_ACP_PERMISSION_MODE: Override permission_mode
            RALPH_ACP_TIMEOUT: Override timeout (integer)

        Args:
            adapter_config: General adapter configuration.

        Returns:
            ACPAdapterConfig with ACP-specific settings.
        """
        # Start with tool_permissions or empty dict
        tool_perms = adapter_config.tool_permissions or {}

        # Get base values from tool_permissions
        agent_command = tool_perms.get("agent_command", "gemini")
        agent_args = tool_perms.get("agent_args", [])
        timeout = tool_perms.get("timeout", adapter_config.timeout)
        permission_mode = tool_perms.get("permission_mode", "auto_approve")
        permission_allowlist = tool_perms.get("permission_allowlist", [])

        # Apply environment variable overrides
        if env_agent := os.environ.get("RALPH_ACP_AGENT"):
            agent_command = env_agent

        if env_mode := os.environ.get("RALPH_ACP_PERMISSION_MODE"):
            valid_modes = {"auto_approve", "deny_all", "allowlist", "interactive"}
            if env_mode in valid_modes:
                permission_mode = env_mode
            else:
                _logger.warning(
                    "Invalid RALPH_ACP_PERMISSION_MODE value '%s'. Valid modes: %s. Using default: %s",
                    env_mode,
                    ", ".join(valid_modes),
                    permission_mode,
                )

        if env_timeout := os.environ.get("RALPH_ACP_TIMEOUT"):
            try:
                timeout = int(env_timeout)
            except ValueError:
                _logger.warning(
                    "Invalid RALPH_ACP_TIMEOUT value '%s' - must be integer. Using default: %d",
                    env_timeout,
                    timeout,
                )

        return cls(
            agent_command=agent_command,
            agent_args=agent_args,
            timeout=timeout,
            permission_mode=permission_mode,
            permission_allowlist=permission_allowlist,
        )
