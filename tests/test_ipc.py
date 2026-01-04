# ABOUTME: Tests for IPC mechanism (Unix socket with HTTP fallback)
# ABOUTME: TDD tests for Plan 02-03: IPC Mechanism

import unittest
import tempfile
import os
import socket
import json
import threading
import time
from pathlib import Path
from typing import Optional


class TestIPCServer(unittest.TestCase):
    """Tests for IPCServer - handles incoming commands from CLI."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.socket_path = Path(self.temp_dir) / "ralph.sock"

    def tearDown(self):
        """Clean up test fixtures."""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_import_ipc_server(self):
        """Test that IPCServer can be imported."""
        from ralph_orchestrator.daemon.ipc import IPCServer
        self.assertIsNotNone(IPCServer)

    def test_init_with_socket_path(self):
        """Test that IPCServer accepts socket path."""
        from ralph_orchestrator.daemon.ipc import IPCServer
        server = IPCServer(socket_path=self.socket_path)
        self.assertEqual(server.socket_path, self.socket_path)

    def test_default_socket_path(self):
        """Test that IPCServer has default socket path."""
        from ralph_orchestrator.daemon.ipc import IPCServer
        server = IPCServer()
        expected = Path.home() / ".ralph" / "ralph.sock"
        self.assertEqual(server.socket_path, expected)

    def test_start_creates_socket(self):
        """Test that start() creates Unix socket file."""
        from ralph_orchestrator.daemon.ipc import IPCServer
        server = IPCServer(socket_path=self.socket_path)
        server.start()
        try:
            self.assertTrue(self.socket_path.exists())
        finally:
            server.stop()

    def test_stop_removes_socket(self):
        """Test that stop() removes Unix socket file."""
        from ralph_orchestrator.daemon.ipc import IPCServer
        server = IPCServer(socket_path=self.socket_path)
        server.start()
        server.stop()
        self.assertFalse(self.socket_path.exists())

    def test_server_accepts_connections(self):
        """Test that server accepts client connections."""
        from ralph_orchestrator.daemon.ipc import IPCServer
        server = IPCServer(socket_path=self.socket_path)
        server.start()
        try:
            # Try to connect
            client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            client.connect(str(self.socket_path))
            client.close()
        finally:
            server.stop()

    def test_register_handler(self):
        """Test that handlers can be registered for commands."""
        from ralph_orchestrator.daemon.ipc import IPCServer
        server = IPCServer(socket_path=self.socket_path)

        def handler(params):
            return {"result": "ok"}

        server.register_handler("test_command", handler)
        self.assertIn("test_command", server.handlers)

    def test_handle_command_calls_registered_handler(self):
        """Test that received commands dispatch to handlers."""
        from ralph_orchestrator.daemon.ipc import IPCServer, IPCClient
        server = IPCServer(socket_path=self.socket_path)

        handler_called = []

        def handler(params):
            handler_called.append(params)
            return {"status": "handled"}

        server.register_handler("test_cmd", handler)
        server.start()
        try:
            client = IPCClient(socket_path=self.socket_path)
            response = client.send_command("test_cmd", {"arg": "value"})
            self.assertEqual(len(handler_called), 1)
            self.assertEqual(handler_called[0], {"arg": "value"})
            self.assertEqual(response["status"], "handled")
        finally:
            server.stop()


class TestIPCClient(unittest.TestCase):
    """Tests for IPCClient - sends commands to daemon."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.socket_path = Path(self.temp_dir) / "ralph.sock"

    def tearDown(self):
        """Clean up test fixtures."""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_import_ipc_client(self):
        """Test that IPCClient can be imported."""
        from ralph_orchestrator.daemon.ipc import IPCClient
        self.assertIsNotNone(IPCClient)

    def test_init_with_socket_path(self):
        """Test that IPCClient accepts socket path."""
        from ralph_orchestrator.daemon.ipc import IPCClient
        client = IPCClient(socket_path=self.socket_path)
        self.assertEqual(client.socket_path, self.socket_path)

    def test_default_socket_path(self):
        """Test that IPCClient has default socket path."""
        from ralph_orchestrator.daemon.ipc import IPCClient
        client = IPCClient()
        expected = Path.home() / ".ralph" / "ralph.sock"
        self.assertEqual(client.socket_path, expected)

    def test_send_command_returns_response(self):
        """Test that send_command returns server response."""
        from ralph_orchestrator.daemon.ipc import IPCServer, IPCClient
        server = IPCServer(socket_path=self.socket_path)
        server.register_handler("ping", lambda p: {"pong": True})
        server.start()
        try:
            client = IPCClient(socket_path=self.socket_path)
            response = client.send_command("ping", {})
            self.assertEqual(response, {"pong": True})
        finally:
            server.stop()

    def test_send_command_raises_when_not_connected(self):
        """Test that send_command raises when server not available."""
        from ralph_orchestrator.daemon.ipc import IPCClient, IPCConnectionError
        client = IPCClient(socket_path=self.socket_path)
        with self.assertRaises(IPCConnectionError):
            client.send_command("ping", {})

    def test_is_daemon_running(self):
        """Test that is_daemon_running detects server availability."""
        from ralph_orchestrator.daemon.ipc import IPCServer, IPCClient
        client = IPCClient(socket_path=self.socket_path)

        # Not running initially
        self.assertFalse(client.is_daemon_running())

        # Start server
        server = IPCServer(socket_path=self.socket_path)
        server.start()
        try:
            # Now running
            self.assertTrue(client.is_daemon_running())
        finally:
            server.stop()

        # Not running after stop
        self.assertFalse(client.is_daemon_running())


class TestIPCProtocol(unittest.TestCase):
    """Tests for IPC message protocol."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.socket_path = Path(self.temp_dir) / "ralph.sock"

    def tearDown(self):
        """Clean up test fixtures."""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_json_message_format(self):
        """Test that messages use JSON format."""
        from ralph_orchestrator.daemon.ipc import IPCServer, IPCClient
        server = IPCServer(socket_path=self.socket_path)
        server.register_handler("echo", lambda p: p)
        server.start()
        try:
            client = IPCClient(socket_path=self.socket_path)
            test_data = {"key": "value", "number": 42, "nested": {"a": 1}}
            response = client.send_command("echo", test_data)
            self.assertEqual(response, test_data)
        finally:
            server.stop()

    def test_error_response_for_unknown_command(self):
        """Test that unknown commands return error."""
        from ralph_orchestrator.daemon.ipc import IPCServer, IPCClient
        server = IPCServer(socket_path=self.socket_path)
        server.start()
        try:
            client = IPCClient(socket_path=self.socket_path)
            response = client.send_command("nonexistent_command", {})
            self.assertIn("error", response)
            self.assertIn("unknown command", response["error"].lower())
        finally:
            server.stop()

    def test_error_response_for_handler_exception(self):
        """Test that handler exceptions return error response."""
        from ralph_orchestrator.daemon.ipc import IPCServer, IPCClient
        server = IPCServer(socket_path=self.socket_path)

        def failing_handler(params):
            raise ValueError("Test error")

        server.register_handler("fail", failing_handler)
        server.start()
        try:
            client = IPCClient(socket_path=self.socket_path)
            response = client.send_command("fail", {})
            self.assertIn("error", response)
            self.assertIn("Test error", response["error"])
        finally:
            server.stop()


class TestIPCHTTPFallback(unittest.TestCase):
    """Tests for HTTP fallback when Unix socket not available."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.socket_path = Path(self.temp_dir) / "ralph.sock"

    def tearDown(self):
        """Clean up test fixtures."""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_client_http_fallback_option(self):
        """Test that IPCClient can be configured for HTTP fallback."""
        from ralph_orchestrator.daemon.ipc import IPCClient
        client = IPCClient(socket_path=self.socket_path, http_fallback_port=8080)
        self.assertEqual(client.http_fallback_port, 8080)

    def test_server_http_fallback_option(self):
        """Test that IPCServer can be configured for HTTP fallback."""
        from ralph_orchestrator.daemon.ipc import IPCServer
        server = IPCServer(socket_path=self.socket_path, http_fallback_port=8081)
        self.assertEqual(server.http_fallback_port, 8081)


if __name__ == "__main__":
    unittest.main()
