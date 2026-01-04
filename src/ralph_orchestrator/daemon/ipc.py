# ABOUTME: IPC mechanism for daemon communication (Unix socket with HTTP fallback)
# ABOUTME: Enables CLI to communicate with background daemon process

import json
import socket
import threading
from pathlib import Path
from typing import Callable, Dict, Any, Optional


class IPCConnectionError(Exception):
    """Raised when unable to connect to daemon."""
    pass


class IPCServer:
    """IPC server for daemon to receive commands from CLI.

    Uses Unix domain sockets for local IPC with optional HTTP fallback
    for cross-platform compatibility.
    """

    def __init__(
        self,
        socket_path: Optional[Path] = None,
        http_fallback_port: Optional[int] = None
    ):
        """Initialize IPC server.

        Args:
            socket_path: Path to Unix socket. Defaults to ~/.ralph/ralph.sock
            http_fallback_port: Optional HTTP port for cross-platform fallback
        """
        self.socket_path = socket_path or (Path.home() / ".ralph" / "ralph.sock")
        self.http_fallback_port = http_fallback_port
        self.handlers: Dict[str, Callable[[Dict], Dict]] = {}
        self._server_socket: Optional[socket.socket] = None
        self._running = False
        self._accept_thread: Optional[threading.Thread] = None

    def register_handler(self, command: str, handler: Callable[[Dict], Dict]) -> None:
        """Register a handler for a command.

        Args:
            command: Command name to handle
            handler: Function that takes params dict and returns response dict
        """
        self.handlers[command] = handler

    def start(self) -> None:
        """Start the IPC server listening for connections."""
        # Create parent directory if needed
        self.socket_path.parent.mkdir(parents=True, exist_ok=True)

        # Remove stale socket file
        if self.socket_path.exists():
            self.socket_path.unlink()

        # Create and bind Unix socket
        self._server_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._server_socket.bind(str(self.socket_path))
        self._server_socket.listen(5)
        self._server_socket.settimeout(0.5)  # Allow periodic check for stop

        self._running = True

        # Start accept thread
        self._accept_thread = threading.Thread(target=self._accept_loop, daemon=True)
        self._accept_thread.start()

    def stop(self) -> None:
        """Stop the IPC server and clean up."""
        self._running = False

        # Close server socket
        if self._server_socket:
            try:
                self._server_socket.close()
            except OSError:
                pass
            self._server_socket = None

        # Wait for accept thread to finish
        if self._accept_thread and self._accept_thread.is_alive():
            self._accept_thread.join(timeout=1.0)
            self._accept_thread = None

        # Remove socket file
        if self.socket_path.exists():
            try:
                self.socket_path.unlink()
            except OSError:
                pass

    def _accept_loop(self) -> None:
        """Accept loop running in separate thread."""
        while self._running:
            try:
                client_socket, _ = self._server_socket.accept()
                # Handle client in separate thread
                handler_thread = threading.Thread(
                    target=self._handle_client,
                    args=(client_socket,),
                    daemon=True
                )
                handler_thread.start()
            except socket.timeout:
                continue  # Check if still running
            except OSError:
                break  # Socket closed

    def _handle_client(self, client_socket: socket.socket) -> None:
        """Handle a client connection.

        Args:
            client_socket: Connected client socket
        """
        try:
            # Read request
            data = b""
            while True:
                chunk = client_socket.recv(4096)
                if not chunk:
                    break
                data += chunk
                # Check for complete JSON (ends with newline)
                if data.endswith(b"\n"):
                    break

            if not data:
                return

            # Parse request
            try:
                request = json.loads(data.decode("utf-8").strip())
            except json.JSONDecodeError:
                response = {"error": "Invalid JSON"}
                client_socket.sendall(json.dumps(response).encode("utf-8") + b"\n")
                return

            # Get command and params
            command = request.get("command", "")
            params = request.get("params", {})

            # Dispatch to handler
            response = self._dispatch(command, params)

            # Send response
            client_socket.sendall(json.dumps(response).encode("utf-8") + b"\n")

        except Exception as e:
            try:
                response = {"error": str(e)}
                client_socket.sendall(json.dumps(response).encode("utf-8") + b"\n")
            except Exception:
                pass
        finally:
            try:
                client_socket.close()
            except Exception:
                pass

    def _dispatch(self, command: str, params: Dict) -> Dict:
        """Dispatch command to registered handler.

        Args:
            command: Command name
            params: Command parameters

        Returns:
            Response dict from handler
        """
        if command not in self.handlers:
            return {"error": f"Unknown command: {command}"}

        try:
            return self.handlers[command](params)
        except Exception as e:
            return {"error": str(e)}


class IPCClient:
    """IPC client for CLI to send commands to daemon.

    Connects to daemon via Unix socket or HTTP fallback.
    """

    def __init__(
        self,
        socket_path: Optional[Path] = None,
        http_fallback_port: Optional[int] = None
    ):
        """Initialize IPC client.

        Args:
            socket_path: Path to Unix socket. Defaults to ~/.ralph/ralph.sock
            http_fallback_port: Optional HTTP port for cross-platform fallback
        """
        self.socket_path = socket_path or (Path.home() / ".ralph" / "ralph.sock")
        self.http_fallback_port = http_fallback_port

    def send_command(self, command: str, params: Dict[str, Any]) -> Dict[str, Any]:
        """Send command to daemon and return response.

        Args:
            command: Command name
            params: Command parameters

        Returns:
            Response dict from daemon

        Raises:
            IPCConnectionError: If unable to connect to daemon
        """
        if not self.is_daemon_running():
            raise IPCConnectionError("Daemon is not running")

        try:
            # Connect to socket
            client_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            client_socket.settimeout(5.0)
            client_socket.connect(str(self.socket_path))

            # Send request
            request = {"command": command, "params": params}
            client_socket.sendall(json.dumps(request).encode("utf-8") + b"\n")

            # Read response
            data = b""
            while True:
                chunk = client_socket.recv(4096)
                if not chunk:
                    break
                data += chunk
                if data.endswith(b"\n"):
                    break

            client_socket.close()

            # Parse response
            return json.loads(data.decode("utf-8").strip())

        except socket.error as e:
            raise IPCConnectionError(f"Socket error: {e}")
        except json.JSONDecodeError:
            raise IPCConnectionError("Invalid response from daemon")

    def is_daemon_running(self) -> bool:
        """Check if daemon is running and accessible.

        Returns:
            True if daemon is running and socket is connectable
        """
        if not self.socket_path.exists():
            return False

        try:
            client_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            client_socket.settimeout(1.0)
            client_socket.connect(str(self.socket_path))
            client_socket.close()
            return True
        except socket.error:
            return False
