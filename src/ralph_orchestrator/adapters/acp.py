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
                        "version": "1.2.2",
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
                content_obj = update.get("content", {})
                # Extract text content if it's an object
                if isinstance(content_obj, dict):
                    content = content_obj.get("text", "")
                else:
                    content = str(content_obj) if content_obj else ""
                flat_params = {"kind": kind, "content": content}
                # Copy other fields if present
                for key in ["toolName", "toolCallId", "arguments", "status", "result", "error"]:
                    if key in update:
                        flat_params[key] = update[key]
                payload = UpdatePayload.from_dict(flat_params)
            else:
                # Flat format
                payload = UpdatePayload.from_dict(params)

            # Stream to console if verbose (use per-request flag)
            if self._current_verbose:
                self._stream_update(payload)

            self._session.process_update(payload)

    def _stream_update(self, payload: UpdatePayload) -> None:
        """Stream session update to console.

        Args:
            payload: The update payload to stream.
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
            tool_name = payload.tool_name or "unknown"
            tool_id = payload.tool_call_id or "unknown"
            self._console.print_separator()
            self._console.print_status(f"TOOL CALL: {tool_name}", style="cyan bold")
            self._console.print_info(f"ID: {tool_id[:12]}...")
            if payload.arguments:
                self._console.print_info("Arguments:")
                for key, value in payload.arguments.items():
                    value_str = str(value)
                    if len(value_str) > 100:
                        value_str = value_str[:97] + "..."
                    self._console.print_info(f"  - {key}: {value_str}")

        elif kind == "tool_call_update":
            # Show tool call status update
            tool_id = payload.tool_call_id or "unknown"
            status = payload.status or "unknown"

            if status == "completed":
                self._console.print_success(f"Tool {tool_id[:12]}... completed")
                if payload.result:
                    result_str = str(payload.result)
                    if len(result_str) > 200:
                        result_str = result_str[:197] + "..."
                    self._console.print_info(f"Result: {result_str}")
            elif status == "failed":
                self._console.print_error(f"Tool {tool_id[:12]}... failed")
                if payload.error:
                    self._console.print_error(f"Error: {payload.error}")
            elif status == "running":
                self._console.print_status(f"Tool {tool_id[:12]}... running", style="yellow")

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
