# ABOUTME: ACPClient manages subprocess lifecycle for ACP agents
# ABOUTME: Handles async message routing, request tracking, and graceful shutdown

"""ACPClient subprocess manager for ACP (Agent Client Protocol)."""

import asyncio
import logging
import os
from typing import Any, Callable, Optional

from .acp_protocol import ACPProtocol, MessageType

logger = logging.getLogger(__name__)

# Default asyncio StreamReader line limit (in bytes) for ACP agent stdout/stderr.
# Some ACP agents can emit large single-line JSON-RPC frames (e.g., big tool payloads).
# The asyncio default (~64KiB) can raise LimitOverrunError and break the ACP session.
DEFAULT_ACP_STREAM_LIMIT = 8 * 1024 * 1024  # 8 MiB


class ACPClientError(Exception):
    """Exception raised by ACPClient operations."""

    pass


class ACPClient:
    """Manages subprocess lifecycle and async message routing for ACP agents.

    Spawns an agent subprocess, handles JSON-RPC message serialization,
    routes responses to pending requests, and invokes callbacks for
    notifications and incoming requests.

    Attributes:
        command: The command to spawn the agent.
        args: Additional command-line arguments.
        timeout: Request timeout in seconds.
    """

    def __init__(
        self,
        command: str,
        args: Optional[list[str]] = None,
        timeout: int = 300,
        stream_limit: Optional[int] = None,
    ) -> None:
        """Initialize ACPClient.

        Args:
            command: The command to spawn the agent (e.g., "gemini").
            args: Additional command-line arguments.
            timeout: Request timeout in seconds (default: 300).
        """
        self.command = command
        self.args = args or []
        self.timeout = timeout
        # Limit used by asyncio for readline()/readuntil(); must be > max ACP frame line length.
        env_limit = os.environ.get("RALPH_ACP_STREAM_LIMIT")
        if stream_limit is not None:
            self.stream_limit = stream_limit
        elif env_limit:
            try:
                self.stream_limit = int(env_limit)
            except ValueError:
                logger.warning(
                    "Invalid RALPH_ACP_STREAM_LIMIT=%r; using default %d",
                    env_limit,
                    DEFAULT_ACP_STREAM_LIMIT,
                )
                self.stream_limit = DEFAULT_ACP_STREAM_LIMIT
        else:
            self.stream_limit = DEFAULT_ACP_STREAM_LIMIT

        self._protocol = ACPProtocol()
        self._process: Optional[asyncio.subprocess.Process] = None
        self._read_task: Optional[asyncio.Task] = None
        self._write_lock = asyncio.Lock()

        # Pending requests: id -> Future
        self._pending_requests: dict[int, asyncio.Future] = {}

        # Notification handlers
        self._notification_handlers: list[Callable[[str, dict], None]] = []

        # Request handlers (for incoming requests from agent)
        self._request_handlers: list[Callable[[str, dict], Any]] = []

    @property
    def is_running(self) -> bool:
        """Check if subprocess is running.

        Returns:
            True if subprocess is running, False otherwise.
        """
        return self._process is not None and self._process.returncode is None

    async def start(self) -> None:
        """Start the agent subprocess.

        Spawns the subprocess with stdin/stdout/stderr pipes and starts
        the read loop task.

        Raises:
            RuntimeError: If already running.
            FileNotFoundError: If command not found.
        """
        if self.is_running:
            raise RuntimeError("ACPClient is already running")

        cmd = [self.command] + self.args

        self._process = await asyncio.create_subprocess_exec(
            *cmd,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            limit=self.stream_limit,
        )

        # Start the read loop
        self._read_task = asyncio.create_task(self._read_loop())

    async def stop(self) -> None:
        """Stop the agent subprocess.

        Terminates the subprocess gracefully with 2 second timeout, then kills if necessary.
        Cancels the read loop task and all pending requests.
        """
        if not self.is_running:
            return

        # Cancel read loop first
        if self._read_task and not self._read_task.done():
            self._read_task.cancel()
            try:
                await asyncio.wait_for(self._read_task, timeout=0.5)
            except asyncio.CancelledError:
                logger.debug("Read task cancelled during shutdown")
            except asyncio.TimeoutError:
                logger.warning("Read task cancellation timed out")

        # Terminate subprocess with 2 second timeout
        if self._process:
            try:
                self._process.terminate()
                try:
                    await asyncio.wait_for(self._process.wait(), timeout=2.0)
                except asyncio.TimeoutError:
                    # Force kill if graceful termination fails
                    logger.warning("Process did not terminate gracefully, killing")
                    self._process.kill()
                    try:
                        await asyncio.wait_for(self._process.wait(), timeout=0.5)
                    except asyncio.TimeoutError:
                        logger.error("Process did not die after kill signal")
            except ProcessLookupError:
                logger.debug("Process already terminated")

        self._process = None
        self._read_task = None

        # Cancel all pending requests
        for future in self._pending_requests.values():
            if not future.done():
                future.cancel()
        self._pending_requests.clear()

    async def _read_loop(self) -> None:
        """Continuously read stdout and route messages.

        Reads newline-delimited JSON-RPC messages from subprocess stdout.
        """
        if not self._process or not self._process.stdout:
            return

        try:
            while self.is_running:
                line = await self._process.stdout.readline()
                if not line:
                    break

                message_str = line.decode().strip()
                if message_str:
                    await self._handle_message(message_str)
        except asyncio.CancelledError:
            pass  # Expected during shutdown
        except Exception as e:
            logger.error("ACP read loop failed: %s", e, exc_info=True)
        finally:
            # Cancel all pending requests when read loop exits (subprocess died or cancelled)
            for future in self._pending_requests.values():
                if not future.done():
                    future.set_exception(ACPClientError("Agent subprocess terminated"))
            self._pending_requests.clear()

    async def _handle_message(self, message_str: str) -> None:
        """Handle a received JSON-RPC message.

        Routes message to appropriate handler based on type.

        Args:
            message_str: Raw JSON string.
        """
        parsed = self._protocol.parse_message(message_str)
        msg_type = parsed.get("type")

        if msg_type == MessageType.RESPONSE:
            # Route to pending request
            request_id = parsed.get("id")
            if request_id in self._pending_requests:
                future = self._pending_requests.pop(request_id)
                if not future.done():
                    future.set_result(parsed.get("result"))

        elif msg_type == MessageType.ERROR:
            # Route error to pending request
            request_id = parsed.get("id")
            if request_id in self._pending_requests:
                future = self._pending_requests.pop(request_id)
                if not future.done():
                    error = parsed.get("error", {})
                    error_msg = error.get("message", "Unknown error")
                    future.set_exception(ACPClientError(error_msg))

        elif msg_type == MessageType.NOTIFICATION:
            # Invoke notification handlers
            method = parsed.get("method", "")
            params = parsed.get("params", {})
            for handler in self._notification_handlers:
                try:
                    handler(method, params)
                except Exception as e:
                    logger.error("Notification handler failed for method=%s: %s", method, e, exc_info=True)

        elif msg_type == MessageType.REQUEST:
            # Invoke request handlers and send response
            request_id = parsed.get("id")
            method = parsed.get("method", "")
            params = parsed.get("params", {})

            for handler in self._request_handlers:
                try:
                    if asyncio.iscoroutinefunction(handler):
                        result = await handler(method, params)
                    else:
                        result = handler(method, params)

                    # Check if handler returned an error (dict with "error" key)
                    if isinstance(result, dict) and "error" in result:
                        error_info = result["error"]
                        error_code = error_info.get("code", -32603) if isinstance(error_info, dict) else -32603
                        error_msg = error_info.get("message", str(error_info)) if isinstance(error_info, dict) else str(error_info)
                        response = self._protocol.create_error_response(request_id, error_code, error_msg)
                    else:
                        response = self._protocol.create_response(request_id, result)
                    await self._write_message(response)
                    break  # Only first handler responds
                except Exception as e:
                    # Send error response
                    error_response = self._protocol.create_error_response(
                        request_id, -32603, str(e)
                    )
                    await self._write_message(error_response)
                    break

    async def _write_message(self, message: str) -> None:
        """Write a JSON-RPC message to subprocess stdin.

        Args:
            message: JSON string to write.

        Raises:
            RuntimeError: If not running.
        """
        if not self.is_running or not self._process or not self._process.stdin:
            raise RuntimeError("ACPClient is not running")

        async with self._write_lock:
            self._process.stdin.write((message + "\n").encode())
            await self._process.stdin.drain()

    async def _do_send(self, request_id: int, message: str) -> None:
        """Helper to send message and handle write errors.

        Args:
            request_id: The request ID.
            message: The JSON-RPC message to send.
        """
        try:
            await self._write_message(message)
        except Exception as e:
            future = self._pending_requests.pop(request_id, None)
            if future and not future.done():
                future.set_exception(ACPClientError(f"Failed to send request: {e}"))

    def send_request(
        self, method: str, params: dict[str, Any]
    ) -> asyncio.Future[Any]:
        """Send a JSON-RPC request and return Future for response.

        Args:
            method: The RPC method name.
            params: The request parameters.

        Returns:
            Future that resolves with the response result.
        """
        request_id, message = self._protocol.create_request(method, params)

        # Create future for response
        loop = asyncio.get_running_loop()
        future: asyncio.Future[Any] = loop.create_future()
        self._pending_requests[request_id] = future

        # Schedule write with error handling
        asyncio.create_task(self._do_send(request_id, message))

        return future

    async def send_notification(
        self, method: str, params: dict[str, Any]
    ) -> None:
        """Send a JSON-RPC notification (no response expected).

        Args:
            method: The notification method name.
            params: The notification parameters.
        """
        message = self._protocol.create_notification(method, params)
        await self._write_message(message)

    def on_notification(
        self, handler: Callable[[str, dict], None]
    ) -> None:
        """Register a notification handler.

        Args:
            handler: Callback invoked with (method, params) for each notification.
        """
        self._notification_handlers.append(handler)

    def on_request(
        self, handler: Callable[[str, dict], Any]
    ) -> None:
        """Register a request handler for incoming requests from agent.

        Handler should return the response result. Can be sync or async.

        Args:
            handler: Callback invoked with (method, params), returns result.
        """
        self._request_handlers.append(handler)
