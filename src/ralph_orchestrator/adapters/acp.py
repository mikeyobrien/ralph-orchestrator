# ABOUTME: ACP Adapter for Agent Client Protocol integration
# ABOUTME: Provides subprocess-based communication with ACP-compliant agents like Gemini CLI

"""ACP (Agent Client Protocol) adapter for Ralph Orchestrator.

This adapter enables Ralph to use any ACP-compliant agent (like Gemini CLI)
as a backend for task execution. It manages the subprocess lifecycle,
handles the initialization handshake, and routes session messages.
"""

import asyncio
import logging
import os
import shutil
import signal
import threading
from typing import Optional

from .base import ToolAdapter, ToolResponse
from .acp_client import ACPClient, ACPClientError
from .acp_models import ACPAdapterConfig, ACPSession, UpdatePayload
from .acp_handlers import ACPHandlers
from ..output.console import RalphConsole

logger = logging.getLogger(__name__)


# ACP Protocol version this adapter supports (integer per spec)
ACP_PROTOCOL_VERSION = 1


class ACPAdapter(ToolAdapter):
    """Adapter for ACP-compliant agents like Gemini CLI.

    Manages subprocess lifecycle, initialization handshake, and session
    message routing for Agent Client Protocol communication.

    Attributes:
        agent_command: Command to spawn the agent (default: gemini).
        agent_args: Additional arguments for agent command.
        timeout: Request timeout in seconds.
        permission_mode: How to handle permission requests.
    """

    _TOOL_FIELD_ALIASES = {
        "toolName": ("toolName", "tool_name", "name", "tool"),
        "toolCallId": ("toolCallId", "tool_call_id", "id"),
        "arguments": ("arguments", "args", "parameters", "params", "input"),
        "status": ("status",),
        "result": ("result",),
        "error": ("error",),
    }

    def __init__(
        self,
        agent_command: str = "gemini",
        agent_args: Optional[list[str]] = None,
        timeout: int = 300,
        permission_mode: str = "auto_approve",
        permission_allowlist: Optional[list[str]] = None,
        verbose: bool = False,
    ) -> None:
        """Initialize ACPAdapter.

        Args:
            agent_command: Command to spawn the agent (default: gemini).
            agent_args: Additional command-line arguments.
            timeout: Request timeout in seconds (default: 300).
            permission_mode: Permission handling mode (default: auto_approve).
            permission_allowlist: Patterns for allowlist mode.
            verbose: Enable verbose streaming output (default: False).
        """
        self.agent_command = agent_command
        self.agent_args = agent_args or []
        self.timeout = timeout
        self.permission_mode = permission_mode
        self.permission_allowlist = permission_allowlist or []
        self.verbose = verbose
        self._current_verbose = verbose  # Per-request verbose flag

        # Console for verbose output
        self._console = RalphConsole()

        # State
        self._client: Optional[ACPClient] = None
        self._session_id: Optional[str] = None
        self._initialized = False
        self._session: Optional[ACPSession] = None

        # Create permission handlers
        self._handlers = ACPHandlers(
            permission_mode=permission_mode,
            permission_allowlist=self.permission_allowlist,
            on_permission_log=self._log_permission,
        )

        # Thread synchronization
        self._lock = threading.Lock()
        self._shutdown_requested = False

        # Signal handlers
        self._original_sigint = None
        self._original_sigterm = None

        # Call parent init - this will call check_availability()
        super().__init__("acp")

        # Register signal handlers
        self._register_signal_handlers()

    @classmethod
    def from_config(cls, config: ACPAdapterConfig) -> "ACPAdapter":
        """Create ACPAdapter from configuration object.

        Args:
            config: ACPAdapterConfig with adapter settings.

        Returns:
            Configured ACPAdapter instance.
        """
        return cls(
            agent_command=config.agent_command,
            agent_args=config.agent_args,
            timeout=config.timeout,
            permission_mode=config.permission_mode,
            permission_allowlist=config.permission_allowlist,
        )

    def check_availability(self) -> bool:
        """Check if the agent command is available.

        Returns:
            True if agent command exists in PATH, False otherwise.
        """
        return shutil.which(self.agent_command) is not None

    def _register_signal_handlers(self) -> None:
        """Register signal handlers for graceful shutdown."""
        try:
            self._original_sigint = signal.signal(signal.SIGINT, self._signal_handler)
            self._original_sigterm = signal.signal(signal.SIGTERM, self._signal_handler)
        except ValueError as e:
            logger.warning("Cannot register signal handlers (not in main thread): %s. Graceful shutdown via Ctrl+C will not work.", e)

    def _restore_signal_handlers(self) -> None:
        """Restore original signal handlers."""
        try:
            if self._original_sigint is not None:
                signal.signal(signal.SIGINT, self._original_sigint)
            if self._original_sigterm is not None:
                signal.signal(signal.SIGTERM, self._original_sigterm)
        except (ValueError, TypeError) as e:
            logger.warning("Failed to restore signal handlers: %s", e)

    def _signal_handler(self, signum: int, frame) -> None:
        """Handle shutdown signals.

        Terminates running subprocess synchronously (signal-safe),
        then propagates to original handler (orchestrator).

        Args:
            signum: Signal number.
            frame: Current stack frame.
        """
        with self._lock:
            self._shutdown_requested = True

        # Kill subprocess synchronously (signal-safe)
        self.kill_subprocess_sync()

        # Propagate signal to original handler (orchestrator's handler)
        original = self._original_sigint if signum == signal.SIGINT else self._original_sigterm
        if original and callable(original):
            original(signum, frame)

    def kill_subprocess_sync(self) -> None:
        """Synchronously kill the agent subprocess (signal-safe).

        This method is safe to call from signal handlers.
        Uses non-blocking approach with immediate force kill after 2 seconds.
        """
        if self._client and self._client._process:
            try:
                process = self._client._process
                if process.returncode is None:
                    # Try graceful termination first
                    process.terminate()

                    # Non-blocking poll with timeout
                    import time
                    start = time.time()
                    timeout = 2.0

                    while time.time() - start < timeout:
                        if process.poll() is not None:
                            # Process terminated successfully
                            return
                        time.sleep(0.01)  # Brief sleep to avoid busy-wait

                    # Timeout reached, force kill
                    try:
                        process.kill()
                        # Brief wait to ensure kill completes
                        time.sleep(0.1)
                        process.poll()
                    except Exception as e:
                        logger.debug("Exception during subprocess kill: %s", e)
            except Exception as e:
                logger.debug("Exception during subprocess kill: %s", e)

    async def _initialize(self) -> None:
        """Initialize ACP connection with agent.

        Performs the ACP initialization handshake:
        1. Start ACPClient subprocess
        2. Send initialize request with protocol version
        3. Receive and validate initialize response
        4. Send session/new request
        5. Store session_id

        Raises:
            ACPClientError: If initialization fails.
        """
        if self._initialized:
            return

        # Build effective args, auto-adding ACP flags for known agents
        effective_args = list(self.agent_args)

        # Gemini CLI requires --experimental-acp flag to enter ACP mode
        # Also add --yolo to auto-approve internal tool executions
        # And --allowed-tools to enable native Gemini tools
        agent_basename = os.path.basename(self.agent_command)
        if agent_basename == "gemini":
            if "--experimental-acp" not in effective_args:
                logger.info("Auto-adding --experimental-acp flag for Gemini CLI")
                effective_args.append("--experimental-acp")
            if "--yolo" not in effective_args:
                logger.info("Auto-adding --yolo flag for Gemini CLI tool execution")
                effective_args.append("--yolo")
            # Enable native Gemini tools for ACP mode
            # Note: Excluding write_file and run_shell_command - they have bugs in ACP mode
            # Gemini should fall back to ACP's fs/write_text_file and terminal/create
            if "--allowed-tools" not in effective_args:
                logger.info("Auto-adding --allowed-tools for Gemini CLI native tools")
                effective_args.extend([
                    "--allowed-tools",
                    "list_directory",
                    "read_many_files",
                    "read_file",
                    "web_fetch",
                    "google_web_search",
                ])

        # Create and start client
        self._client = ACPClient(
            command=self.agent_command,
            args=effective_args,
            timeout=self.timeout,
        )

        await self._client.start()

        # Register notification handler for session updates
        self._client.on_notification(self._handle_notification)

        # Register request handler for permission requests
        self._client.on_request(self._handle_request)

        try:
            # Send initialize request (per ACP spec)
            init_future = self._client.send_request(
                "initialize",
                {
                    "protocolVersion": ACP_PROTOCOL_VERSION,
                    "clientCapabilities": {
                        "fs": {
                            "readTextFile": True,
                            "writeTextFile": True,
                        },
                        "terminal": True,
                    },
                    "clientInfo": {
                        "name": "ralph-orchestrator",
                        "title": "Ralph Orchestrator",
                        "version": "1.2.3",
                    },
                },
            )
            init_response = await asyncio.wait_for(init_future, timeout=self.timeout)

            # Validate response
            if "protocolVersion" not in init_response:
                raise ACPClientError("Invalid initialize response: missing protocolVersion")

            # Create new session (cwd and mcpServers are required per ACP spec)
            session_future = self._client.send_request(
                "session/new",
                {
                    "cwd": os.getcwd(),
                    "mcpServers": [],  # No MCP servers by default
                },
            )
            session_response = await asyncio.wait_for(session_future, timeout=self.timeout)

            # Store session ID
            self._session_id = session_response.get("sessionId")
            if not self._session_id:
                raise ACPClientError("Invalid session/new response: missing sessionId")

            # Create session state tracker
            self._session = ACPSession(session_id=self._session_id)

            self._initialized = True

        except asyncio.TimeoutError:
            await self._client.stop()
            raise ACPClientError("Initialization timed out")
        except Exception:
            await self._client.stop()
            raise

    def _handle_notification(self, method: str, params: dict) -> None:
        """Handle notifications from agent.

        Args:
            method: Notification method name.
            params: Notification parameters.
        """
        if method == "session/update" and self._session:
            # Handle both notification formats:
            # Format 1 (flat): {"kind": "agent_message_chunk", "content": "..."}
            # Format 2 (nested): {"update": {"sessionUpdate": "agent_message_chunk", "content": {...}}}
            if "update" in params:
                # Nested format (Gemini)
                update = params["update"]
                kind = update.get("sessionUpdate", "")
                content_obj = update.get("content")
                content = None
                flat_params = {"kind": kind, "content": content}
                allow_name_id = kind in ("tool_call", "tool_call_update")
                # Extract text content if it's an object
                if isinstance(content_obj, dict):
                    if "text" in content_obj:
                        content = content_obj.get("text", "")
                        flat_params["content"] = content
                elif isinstance(content_obj, list):
                    for entry in content_obj:
                        if not isinstance(entry, dict):
                            continue
                        self._merge_tool_fields(flat_params, entry, allow_name_id=allow_name_id)
                        nested_tool_call = entry.get("toolCall") or entry.get("tool_call")
                        if isinstance(nested_tool_call, dict):
                            self._merge_tool_fields(
                                flat_params,
                                nested_tool_call,
                                allow_name_id=allow_name_id,
                            )
                        nested_tool = entry.get("tool")
                        if isinstance(nested_tool, dict):
                            self._merge_tool_fields(
                                flat_params,
                                nested_tool,
                                allow_name_id=allow_name_id,
                            )
                else:
                    content = str(content_obj) if content_obj else ""
                    flat_params["content"] = content
                self._merge_tool_fields(flat_params, update, allow_name_id=allow_name_id)
                if isinstance(content_obj, dict):
                    self._merge_tool_fields(flat_params, content_obj, allow_name_id=allow_name_id)
                    nested_tool_call = content_obj.get("toolCall") or content_obj.get("tool_call")
                    if isinstance(nested_tool_call, dict):
                        self._merge_tool_fields(
                            flat_params,
                            nested_tool_call,
                            allow_name_id=allow_name_id,
                        )
                    nested_tool = content_obj.get("tool")
                    if isinstance(nested_tool, dict):
                        self._merge_tool_fields(
                            flat_params,
                            nested_tool,
                            allow_name_id=allow_name_id,
                        )
                nested_tool_call = update.get("toolCall") or update.get("tool_call")
                if isinstance(nested_tool_call, dict):
                    self._merge_tool_fields(
                        flat_params,
                        nested_tool_call,
                        allow_name_id=allow_name_id,
                    )
                nested_tool = update.get("tool")
                if isinstance(nested_tool, dict):
                    self._merge_tool_fields(
                        flat_params,
                        nested_tool,
                        allow_name_id=allow_name_id,
                    )
                payload = UpdatePayload.from_dict(flat_params)
                payload._raw = update
                payload._raw_flat = flat_params
            else:
                # Flat format
                payload = UpdatePayload.from_dict(params)
                payload._raw = params

            # Stream to console if verbose; always show tool calls
            if self._current_verbose:
                self._stream_update(payload, show_details=True)
            elif payload.kind == "tool_call":
                self._stream_update(payload, show_details=False)

            self._session.process_update(payload)

    def _merge_tool_fields(
        self,
        target: dict,
        source: dict,
        *,
        allow_name_id: bool = False,
    ) -> None:
        """Merge tool call fields from source into target with alias support."""
        for canonical, aliases in self._TOOL_FIELD_ALIASES.items():
            if not allow_name_id and canonical in ("toolName", "toolCallId"):
                aliases = tuple(
                    alias for alias in aliases if alias not in ("name", "id")
                )
            if canonical in target and target[canonical] not in (None, ""):
                continue
            for key in aliases:
                if key in source:
                    value = source[key]
                    if key == "tool" and canonical == "toolName":
                        if isinstance(value, dict):
                            value = value.get("name") or value.get("toolName") or value.get("tool_name")
                        elif not isinstance(value, str):
                            value = None
                    if value is None or value == "":
                        continue
                    target[canonical] = value
                    break

    def _format_agent_label(self) -> str:
        """Return the ACP agent command with arguments for display."""
        if not self.agent_args:
            return self.agent_command
        return " ".join([self.agent_command, *self.agent_args])

    def _format_payload_value(self, value: object, limit: int = 200) -> str:
        """Format payload values for console output."""
        if value is None:
            return ""
        value_str = str(value)
        if len(value_str) > limit:
            return value_str[: limit - 3] + "..."
        return value_str

    def _format_payload_error(self, error: object) -> str:
        """Extract a readable error string from ACP error payloads."""
        if error is None:
            return ""
        if isinstance(error, dict):
            message = error.get("message") or error.get("error") or error.get("detail")
            code = error.get("code")
            data = error.get("data")
            parts = []
            if message:
                parts.append(message)
            if code is not None:
                parts.append(f"code={code}")
            if data and not message:
                parts.append(str(data))
            if parts:
                return self._format_payload_value(" ".join(parts), limit=200)
        return self._format_payload_value(error, limit=200)

    def _get_raw_payload(self, payload: UpdatePayload) -> dict | None:
        raw = getattr(payload, "_raw", None)
        return raw if isinstance(raw, dict) else None

    def _extract_tool_field(self, raw: dict | None, key: str) -> object:
        if not isinstance(raw, dict):
            return None
        value = raw.get(key)
        if value not in (None, ""):
            return value
        for nested_key in ("toolCall", "tool_call", "tool"):
            nested = raw.get(nested_key)
            if isinstance(nested, dict) and key in nested:
                nested_value = nested.get(key)
                if nested_value not in (None, ""):
                    return nested_value
        return None

    def _extract_tool_name_from_meta(self, raw: dict | None) -> str | None:
        if not isinstance(raw, dict):
            return None
        meta = raw.get("_meta")
        if not isinstance(meta, dict):
            return None
        for key in ("codex", "claudeCode", "agent", "acp"):
            entry = meta.get(key)
            if isinstance(entry, dict):
                for name_key in ("toolName", "tool_name", "name", "tool"):
                    value = entry.get(name_key)
                    if isinstance(value, str) and value:
                        return value
        for name_key in ("toolName", "tool_name", "name", "tool"):
            value = meta.get(name_key)
            if isinstance(value, str) and value:
                return value
        return None

    def _extract_tool_response(self, raw: dict | None) -> object:
        if not isinstance(raw, dict):
            return None
        meta = raw.get("_meta")
        if not isinstance(meta, dict):
            return None
        for key in ("codex", "claudeCode", "agent", "acp"):
            entry = meta.get(key)
            if isinstance(entry, dict) and "toolResponse" in entry:
                return entry.get("toolResponse")
        if "toolResponse" in meta:
            return meta.get("toolResponse")
        return None

    def _stream_update(self, payload: UpdatePayload, show_details: bool = True) -> None:
        """Stream session update to console.

        Args:
            payload: The update payload to stream.
            show_details: Include detailed info (arguments, results, progress).
        """
        kind = payload.kind

        if kind == "agent_message_chunk":
            # Stream agent output text
            if payload.content:
                self._console.print_message(payload.content)

        elif kind == "agent_thought_chunk":
            # Stream agent internal reasoning (dimmed)
            if payload.content:
                if self._console.console:
                    self._console.console.print(
                        f"[dim italic]{payload.content}[/dim italic]",
                        end="",
                    )
                else:
                    print(payload.content, end="")

        elif kind == "tool_call":
            # Show tool call start
            tool_name = payload.tool_name
            raw_update = self._get_raw_payload(payload)
            meta_tool_name = self._extract_tool_name_from_meta(raw_update)
            title = self._extract_tool_field(raw_update, "title")
            kind = self._extract_tool_field(raw_update, "kind")
            raw_input = self._extract_tool_field(raw_update, "rawInput")
            if raw_input is None:
                raw_input = self._extract_tool_field(raw_update, "input")
            tool_name = tool_name or meta_tool_name or title or kind or "unknown"
            tool_id = payload.tool_call_id or "unknown"
            self._console.print_separator()
            self._console.print_status(f"TOOL CALL: {tool_name}", style="cyan bold")
            self._console.print_info(f"ID: {tool_id[:12]}...")
            self._console.print_info(f"Agent: {self._format_agent_label()}")
            if show_details:
                if title and title != tool_name:
                    self._console.print_info(f"Title: {title}")
                if kind:
                    self._console.print_info(f"Kind: {kind}")
                if tool_name == "unknown":
                    raw_str = self._format_payload_value(raw_update, limit=300)
                    if raw_str:
                        self._console.print_info(f"Update: {raw_str}")
                if payload.arguments or raw_input:
                    input_value = payload.arguments or raw_input
                    if isinstance(input_value, dict):
                        self._console.print_info("Arguments:")
                        for key, value in input_value.items():
                            value_str = str(value)
                            if len(value_str) > 100:
                                value_str = value_str[:97] + "..."
                            self._console.print_info(f"  - {key}: {value_str}")
                    else:
                        input_str = self._format_payload_value(input_value, limit=300)
                        if input_str:
                            self._console.print_info(f"Input: {input_str}")

        elif kind == "tool_call_update":
            if not show_details:
                return
            # Show tool call status update
            tool_id = payload.tool_call_id or "unknown"
            status = payload.status or "unknown"
            tool_name = payload.tool_name
            tool_args = None
            tool_call = None
            if self._session and payload.tool_call_id:
                tool_call = self._session.get_tool_call(payload.tool_call_id)
            if tool_call:
                tool_name = tool_name or tool_call.tool_name
                tool_args = tool_call.arguments or None
            raw_update = self._get_raw_payload(payload)
            meta_tool_name = self._extract_tool_name_from_meta(raw_update)
            title = self._extract_tool_field(raw_update, "title")
            kind = self._extract_tool_field(raw_update, "kind")
            raw_input = self._extract_tool_field(raw_update, "rawInput")
            raw_output = self._extract_tool_field(raw_update, "rawOutput")
            tool_name = tool_name or meta_tool_name or title
            display_name = tool_name or kind or "unknown"
            if display_name == "unknown":
                status_label = f"Tool call {tool_id[:12]}..."
            else:
                status_label = f"Tool {display_name} ({tool_id[:12]}...)"

            if status == "completed":
                self._console.print_success(
                    f"{status_label} completed"
                )
                result_value = payload.result
                if result_value is None and tool_call:
                    result_value = tool_call.result
                if result_value is None:
                    result_value = raw_output
                if result_value is None:
                    result_value = self._extract_tool_response(raw_update)
                result_str = self._format_payload_value(result_value)
                if result_str:
                    self._console.print_info(f"Result: {result_str}")
            elif status == "failed":
                self._console.print_error(
                    f"{status_label} failed"
                )
                if display_name == "unknown":
                    raw_str = self._format_payload_value(raw_update, limit=300)
                    if raw_str:
                        self._console.print_info(f"Update: {raw_str}")
                error_str = self._format_payload_error(payload.error)
                if not error_str and payload.result is not None:
                    error_str = self._format_payload_value(payload.result)
                if not error_str and raw_output is not None:
                    error_str = self._format_payload_value(raw_output)
                if not error_str:
                    error_str = self._format_payload_value(
                        self._extract_tool_response(raw_update)
                    )
                if error_str:
                    self._console.print_error(f"Error: {error_str}")
                if tool_args or raw_input:
                    if tool_args is None:
                        tool_args = raw_input
                    self._console.print_info("Arguments:")
                    if isinstance(tool_args, dict):
                        for key, value in tool_args.items():
                            value_str = str(value)
                            if len(value_str) > 100:
                                value_str = value_str[:97] + "..."
                            self._console.print_info(f"  - {key}: {value_str}")
                    else:
                        arg_str = self._format_payload_value(tool_args, limit=300)
                        if arg_str:
                            self._console.print_info(f"  - {arg_str}")
            elif status == "running":
                self._console.print_status(
                    f"{status_label} running",
                    style="yellow",
                )
                progress_value = payload.result or payload.content
                if progress_value is None:
                    progress_value = raw_output
                progress_str = self._format_payload_value(progress_value, limit=200)
                if progress_str:
                    self._console.print_info(f"Progress: {progress_str}")
            else:
                self._console.print_status(
                    f"{status_label} {status}",
                    style="yellow",
                )
            if title and title != display_name:
                self._console.print_info(f"Title: {title}")
            if kind:
                self._console.print_info(f"Kind: {kind}")

    def _handle_request(self, method: str, params: dict) -> dict:
        """Handle requests from agent.

        Routes requests to appropriate handlers:
        - session/request_permission: Permission checks
        - fs/read_text_file: File read operations
        - fs/write_text_file: File write operations
        - terminal/*: Terminal operations

        Args:
            method: Request method name.
            params: Request parameters.

        Returns:
            Response result dict.
        """
        logger.info("ACP REQUEST: method=%s", method)
        if method == "session/request_permission":
            # Permission handler already returns ACP-compliant format
            return self._handle_permission_request(params)

        # File operations - return raw result (client wraps in JSON-RPC)
        if method == "fs/read_text_file":
            return self._handlers.handle_read_file(params)
        if method == "fs/write_text_file":
            return self._handlers.handle_write_file(params)

        # Terminal operations - return raw result (client wraps in JSON-RPC)
        if method == "terminal/create":
            return self._handlers.handle_terminal_create(params)
        if method == "terminal/output":
            return self._handlers.handle_terminal_output(params)
        if method == "terminal/wait_for_exit":
            return self._handlers.handle_terminal_wait_for_exit(params)
        if method == "terminal/kill":
            return self._handlers.handle_terminal_kill(params)
        if method == "terminal/release":
            return self._handlers.handle_terminal_release(params)

        # Unknown request - log and return error
        logger.warning("Unknown ACP request method: %s with params: %s", method, params)
        return {"error": {"code": -32601, "message": f"Method not found: {method}"}}

    def _handle_permission_request(self, params: dict) -> dict:
        """Handle permission request from agent.

        Delegates to ACPHandlers which supports multiple modes:
        - auto_approve: Always approve
        - deny_all: Always deny
        - allowlist: Check against configured patterns
        - interactive: Prompt user (if terminal available)

        Args:
            params: Permission request parameters.

        Returns:
            Response with approved: True/False.
        """
        return self._handlers.handle_request_permission(params)

    def _log_permission(self, message: str) -> None:
        """Log permission decision.

        Args:
            message: Permission decision message.
        """
        logger.info(message)

    def get_permission_history(self) -> list:
        """Get permission decision history.

        Returns:
            List of (request, result) tuples.
        """
        return self._handlers.get_history()

    def get_permission_stats(self) -> dict:
        """Get permission decision statistics.

        Returns:
            Dict with approved_count and denied_count.
        """
        return {
            "approved_count": self._handlers.get_approved_count(),
            "denied_count": self._handlers.get_denied_count(),
        }

    async def _execute_prompt(self, prompt: str, **kwargs) -> ToolResponse:
        """Execute a prompt through the ACP agent.

        Sends session/prompt request with messages array and waits for response.
        Session updates (streaming output, thoughts, tool calls) are processed
        through _handle_notification during the request.

        Args:
            prompt: The prompt to execute.
            **kwargs: Additional arguments (verbose: bool).

        Returns:
            ToolResponse with execution result.
        """
        # Get verbose from kwargs (per-call override) without mutating instance state
        verbose = kwargs.get("verbose", self.verbose)
        # Store for use in _handle_notification during this request
        self._current_verbose = verbose

        # Reset session state for new prompt (preserve session_id)
        if self._session:
            self._session.reset()

        # Print header if verbose
        if verbose:
            self._console.print_header(f"ACP AGENT ({self.agent_command})")
            self._console.print_status("Processing prompt...")

        # Build prompt array per ACP spec (ContentBlock format)
        prompt_blocks = [{"type": "text", "text": prompt}]

        # Send session/prompt request
        try:
            prompt_future = self._client.send_request(
                "session/prompt",
                {
                    "sessionId": self._session_id,
                    "prompt": prompt_blocks,
                },
            )

            # Wait for response with timeout
            response = await asyncio.wait_for(prompt_future, timeout=self.timeout)

            # Check for error stop reason
            stop_reason = response.get("stopReason", "unknown")
            if stop_reason == "error":
                error_obj = response.get("error", {})
                error_msg = error_obj.get("message", "Unknown error from agent")
                if verbose:
                    self._console.print_separator()
                    self._console.print_error(f"Agent error: {error_msg}")
                return ToolResponse(
                    success=False,
                    output=self._session.output if self._session else "",
                    error=error_msg,
                    metadata={
                        "tool": "acp",
                        "agent": self.agent_command,
                        "session_id": self._session_id,
                        "stop_reason": stop_reason,
                    },
                )

            # Build successful response
            output = self._session.output if self._session else ""
            if verbose:
                self._console.print_separator()
                tool_count = len(self._session.tool_calls) if self._session else 0
                self._console.print_success(f"Agent completed (tools: {tool_count})")
            return ToolResponse(
                success=True,
                output=output,
                metadata={
                    "tool": "acp",
                    "agent": self.agent_command,
                    "session_id": self._session_id,
                    "stop_reason": stop_reason,
                    "tool_calls_count": len(self._session.tool_calls) if self._session else 0,
                    "has_thoughts": bool(self._session.thoughts) if self._session else False,
                },
            )

        except asyncio.TimeoutError:
            if verbose:
                self._console.print_separator()
                self._console.print_error(f"Timeout after {self.timeout}s")
            return ToolResponse(
                success=False,
                output=self._session.output if self._session else "",
                error=f"Prompt execution timed out after {self.timeout} seconds",
                metadata={
                    "tool": "acp",
                    "agent": self.agent_command,
                    "session_id": self._session_id,
                },
            )

    async def _shutdown(self) -> None:
        """Shutdown the ACP connection.

        Stops the client and cleans up state.
        """
        # Kill all running terminals first
        if self._handlers:
            for terminal_id in list(self._handlers._terminals.keys()):
                try:
                    self._handlers.handle_terminal_kill({"terminalId": terminal_id})
                except Exception as e:
                    logger.warning("Failed to kill terminal %s: %s", terminal_id, e)

        if self._client:
            await self._client.stop()
            self._client = None

        self._initialized = False
        self._session_id = None
        self._session = None

    def execute(self, prompt: str, **kwargs) -> ToolResponse:
        """Execute the prompt synchronously.

        Args:
            prompt: The prompt to execute.
            **kwargs: Additional arguments.

        Returns:
            ToolResponse with execution result.
        """
        if not self.available:
            return ToolResponse(
                success=False,
                output="",
                error=f"ACP adapter not available: {self.agent_command} not found",
            )

        # Run async method in new event loop
        try:
            return asyncio.run(self.aexecute(prompt, **kwargs))
        except Exception as e:
            return ToolResponse(
                success=False,
                output="",
                error=str(e),
            )

    async def aexecute(self, prompt: str, **kwargs) -> ToolResponse:
        """Execute the prompt asynchronously.

        Args:
            prompt: The prompt to execute.
            **kwargs: Additional arguments.

        Returns:
            ToolResponse with execution result.
        """
        if not self.available:
            return ToolResponse(
                success=False,
                output="",
                error=f"ACP adapter not available: {self.agent_command} not found",
            )

        try:
            # Initialize if needed
            if not self._initialized:
                await self._initialize()

            # Enhance prompt with orchestration instructions
            enhanced_prompt = self._enhance_prompt_with_instructions(prompt)

            # Execute prompt
            return await self._execute_prompt(enhanced_prompt, **kwargs)

        except ACPClientError as e:
            return ToolResponse(
                success=False,
                output="",
                error=f"ACP error: {e}",
            )
        except Exception as e:
            return ToolResponse(
                success=False,
                output="",
                error=str(e),
            )

    def estimate_cost(self, prompt: str) -> float:
        """Estimate execution cost.

        ACP doesn't provide billing information, so returns 0.

        Args:
            prompt: The prompt to estimate.

        Returns:
            Always 0.0 (no billing info from ACP).
        """
        return 0.0

    def __del__(self) -> None:
        """Cleanup on deletion."""
        self._restore_signal_handlers()

        # Best-effort cleanup
        if self._client:
            try:
                self.kill_subprocess_sync()
            except Exception as e:
                logger.debug("Exception during cleanup in __del__: %s", e)
