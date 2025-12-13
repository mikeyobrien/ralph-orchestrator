# ABOUTME: ACP Adapter for Agent Client Protocol integration
# ABOUTME: Provides subprocess-based communication with ACP-compliant agents like Gemini CLI

"""ACP (Agent Client Protocol) adapter for Ralph Orchestrator.

This adapter enables Ralph to use any ACP-compliant agent (like Gemini CLI)
as a backend for task execution. It manages the subprocess lifecycle,
handles the initialization handshake, and routes session messages.
"""

import asyncio
import shutil
import signal
import threading
from typing import Optional

from .base import ToolAdapter, ToolResponse
from .acp_client import ACPClient, ACPClientError
from .acp_models import ACPAdapterConfig, ACPSession, UpdatePayload
from .acp_handlers import ACPHandlers


# ACP Protocol version this adapter supports
ACP_PROTOCOL_VERSION = "2024-01"


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
    ) -> None:
        """Initialize ACPAdapter.

        Args:
            agent_command: Command to spawn the agent (default: gemini).
            agent_args: Additional command-line arguments.
            timeout: Request timeout in seconds (default: 300).
            permission_mode: Permission handling mode (default: auto_approve).
            permission_allowlist: Patterns for allowlist mode.
        """
        self.agent_command = agent_command
        self.agent_args = agent_args or []
        self.timeout = timeout
        self.permission_mode = permission_mode
        self.permission_allowlist = permission_allowlist or []

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
        except ValueError:
            # Signal handlers can only be set in main thread
            pass

    def _restore_signal_handlers(self) -> None:
        """Restore original signal handlers."""
        try:
            if self._original_sigint is not None:
                signal.signal(signal.SIGINT, self._original_sigint)
            if self._original_sigterm is not None:
                signal.signal(signal.SIGTERM, self._original_sigterm)
        except (ValueError, TypeError):
            pass

    def _signal_handler(self, signum: int, frame) -> None:
        """Handle shutdown signals.

        Terminates running subprocess synchronously (signal-safe).

        Args:
            signum: Signal number.
            frame: Current stack frame.
        """
        with self._lock:
            self._shutdown_requested = True

        # Kill subprocess synchronously (signal-safe)
        self.kill_subprocess_sync()

    def kill_subprocess_sync(self) -> None:
        """Synchronously kill the agent subprocess (signal-safe).

        This method is safe to call from signal handlers.
        """
        if self._client and self._client._process:
            try:
                process = self._client._process
                if process.returncode is None:
                    process.terminate()
                    try:
                        process.wait(timeout=3)
                    except Exception:
                        try:
                            process.kill()
                        except Exception:
                            pass
            except Exception:
                pass

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

        # Create and start client
        self._client = ACPClient(
            command=self.agent_command,
            args=self.agent_args,
            timeout=self.timeout,
        )

        await self._client.start()

        # Register notification handler for session updates
        self._client.on_notification(self._handle_notification)

        # Register request handler for permission requests
        self._client.on_request(self._handle_request)

        try:
            # Send initialize request
            init_future = self._client.send_request(
                "initialize",
                {
                    "protocolVersion": ACP_PROTOCOL_VERSION,
                    "capabilities": {
                        "fs": True,
                        "terminal": True,
                    },
                },
            )
            init_response = await asyncio.wait_for(init_future, timeout=self.timeout)

            # Validate response
            if "protocolVersion" not in init_response:
                raise ACPClientError("Invalid initialize response: missing protocolVersion")

            # Create new session
            session_future = self._client.send_request(
                "session/new",
                {},
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
            payload = UpdatePayload.from_dict(params)
            self._session.process_update(payload)

    def _handle_request(self, method: str, params: dict) -> dict:
        """Handle requests from agent.

        Currently handles permission requests based on permission_mode.

        Args:
            method: Request method name.
            params: Request parameters.

        Returns:
            Response result dict.
        """
        if method == "session/request_permission":
            return self._handle_permission_request(params)

        # Unknown request - return empty result
        return {}

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
        # TODO: Integrate with Ralph's logging system
        pass

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
            **kwargs: Additional arguments.

        Returns:
            ToolResponse with execution result.
        """
        # Reset session state for new prompt (preserve session_id)
        if self._session:
            self._session.reset()

        # Build messages array for ACP protocol
        messages = [{"role": "user", "content": prompt}]

        # Send session/prompt request
        try:
            prompt_future = self._client.send_request(
                "session/prompt",
                {
                    "sessionId": self._session_id,
                    "messages": messages,
                },
            )

            # Wait for response with timeout
            response = await asyncio.wait_for(prompt_future, timeout=self.timeout)

            # Check for error stop reason
            stop_reason = response.get("stopReason", "unknown")
            if stop_reason == "error":
                error_obj = response.get("error", {})
                error_msg = error_obj.get("message", "Unknown error from agent")
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
            except Exception:
                pass
