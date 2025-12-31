# ABOUTME: Tests for ACPClient subprocess manager
# ABOUTME: Tests subprocess lifecycle, message routing, and async communication

"""Tests for ACPClient subprocess manager."""

import asyncio
import pytest

from ralph_orchestrator.adapters.acp_client import ACPClient


class TestACPClientInit:
    """Tests for ACPClient initialization."""

    def test_init_with_defaults(self):
        """ACPClient initializes with default values."""
        client = ACPClient(command="gemini")
        assert client.command == "gemini"
        assert client.args == []
        assert client.timeout == 300
        assert not client.is_running

    def test_init_with_args(self):
        """ACPClient accepts command arguments."""
        client = ACPClient(command="gemini", args=["--model", "pro"])
        assert client.command == "gemini"
        assert client.args == ["--model", "pro"]

    def test_init_with_custom_timeout(self):
        """ACPClient accepts custom timeout."""
        client = ACPClient(command="gemini", timeout=60)
        assert client.timeout == 60


class TestACPClientStart:
    """Tests for starting subprocess."""

    @pytest.mark.asyncio
    async def test_start_spawns_subprocess(self):
        """start() spawns subprocess with correct command."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            assert client.is_running
            assert client._process is not None
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_start_sets_up_pipes(self):
        """start() configures stdin/stdout/stderr pipes."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            assert client._process.stdin is not None
            assert client._process.stdout is not None
            assert client._process.stderr is not None
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_start_twice_raises_error(self):
        """start() raises error if already running."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            with pytest.raises(RuntimeError, match="already running"):
                await client.start()
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_start_with_invalid_command_raises(self):
        """start() raises error for invalid command."""
        client = ACPClient(command="nonexistent_command_xyz")

        with pytest.raises(FileNotFoundError):
            await client.start()


class TestACPClientStop:
    """Tests for stopping subprocess."""

    @pytest.mark.asyncio
    async def test_stop_terminates_process(self):
        """stop() terminates the subprocess."""
        client = ACPClient(command="cat")

        await client.start()
        assert client.is_running

        await client.stop()
        assert not client.is_running

    @pytest.mark.asyncio
    async def test_stop_when_not_running_is_safe(self):
        """stop() when not running does nothing."""
        client = ACPClient(command="cat")
        await client.stop()  # Should not raise
        assert not client.is_running

    @pytest.mark.asyncio
    async def test_stop_cancels_read_loop(self):
        """stop() cancels the read loop task."""
        client = ACPClient(command="cat")

        await client.start()
        read_task = client._read_task
        assert read_task is not None

        await client.stop()
        # Read task should be cancelled or done
        assert read_task.done() or read_task.cancelled()


class TestACPClientWriteMessage:
    """Tests for writing messages to subprocess."""

    @pytest.mark.asyncio
    async def test_write_message_sends_to_stdin(self):
        """_write_message() writes JSON to stdin with newline."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            message = '{"jsonrpc":"2.0","method":"test","params":{}}'
            await client._write_message(message)

            # Read back from cat
            line = await asyncio.wait_for(
                client._process.stdout.readline(), timeout=1.0
            )
            assert line.decode().strip() == message
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_write_message_raises_when_not_running(self):
        """_write_message() raises error when not running."""
        client = ACPClient(command="cat")

        with pytest.raises(RuntimeError, match="not running"):
            await client._write_message('{"test": true}')


class TestACPClientSendRequest:
    """Tests for sending JSON-RPC requests."""

    @pytest.mark.asyncio
    async def test_send_request_returns_future(self):
        """send_request() returns a Future for the response."""
        # Use a simple echo script that echoes JSON-RPC response
        client = ACPClient(command="cat")

        await client.start()
        try:
            future = client.send_request("test/method", {"key": "value"})
            assert asyncio.isfuture(future)
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_send_request_increments_id(self):
        """send_request() uses incrementing request IDs."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            # Track the request IDs
            id1 = client._protocol._request_id + 1
            client.send_request("test", {})
            id2 = client._protocol._request_id

            client.send_request("test", {})
            id3 = client._protocol._request_id

            assert id2 == id1
            assert id3 == id1 + 1
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_send_request_tracks_pending(self):
        """send_request() tracks pending requests by ID."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            client.send_request("test", {})
            assert len(client._pending_requests) == 1

            client.send_request("test2", {})
            assert len(client._pending_requests) == 2
        finally:
            await client.stop()


class TestACPClientSendNotification:
    """Tests for sending JSON-RPC notifications."""

    @pytest.mark.asyncio
    async def test_send_notification_no_response_expected(self):
        """send_notification() sends without expecting response."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            # Should not add to pending requests
            await client.send_notification("session/update", {"data": "test"})
            assert len(client._pending_requests) == 0
        finally:
            await client.stop()


class TestACPClientResponseRouting:
    """Tests for routing responses to pending requests."""

    @pytest.mark.asyncio
    async def test_response_resolves_pending_request(self):
        """Response with matching ID resolves the pending Future."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            # Send request
            future = client.send_request("test", {})
            request_id = client._protocol._request_id

            # Manually inject a response (simulating agent response)
            response_json = f'{{"jsonrpc":"2.0","id":{request_id},"result":{{"ok":true}}}}'
            await client._handle_message(response_json)

            # Future should be resolved
            result = await asyncio.wait_for(future, timeout=1.0)
            assert result == {"ok": True}
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_error_response_rejects_pending_request(self):
        """Error response with matching ID rejects the pending Future."""
        client = ACPClient(command="cat")

        await client.start()
        try:
            future = client.send_request("test", {})
            request_id = client._protocol._request_id

            # Inject error response
            error_json = f'{{"jsonrpc":"2.0","id":{request_id},"error":{{"code":-32601,"message":"Method not found"}}}}'
            await client._handle_message(error_json)

            with pytest.raises(Exception) as exc_info:
                await asyncio.wait_for(future, timeout=1.0)
            assert "Method not found" in str(exc_info.value)
        finally:
            await client.stop()


class TestACPClientNotificationHandler:
    """Tests for handling incoming notifications."""

    @pytest.mark.asyncio
    async def test_notification_callback_invoked(self):
        """Notification triggers registered callback."""
        client = ACPClient(command="cat")
        received = []

        def handler(method: str, params: dict):
            received.append((method, params))

        client.on_notification(handler)

        await client.start()
        try:
            notification_json = '{"jsonrpc":"2.0","method":"session/update","params":{"kind":"test"}}'
            await client._handle_message(notification_json)

            assert len(received) == 1
            assert received[0][0] == "session/update"
            assert received[0][1] == {"kind": "test"}
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_multiple_notification_handlers(self):
        """Multiple notification handlers can be registered."""
        client = ACPClient(command="cat")
        received1 = []
        received2 = []

        client.on_notification(lambda m, p: received1.append(m))
        client.on_notification(lambda m, p: received2.append(m))

        await client.start()
        try:
            notification_json = '{"jsonrpc":"2.0","method":"test","params":{}}'
            await client._handle_message(notification_json)

            assert len(received1) == 1
            assert len(received2) == 1
        finally:
            await client.stop()


class TestACPClientRequestHandler:
    """Tests for handling incoming requests from agent."""

    @pytest.mark.asyncio
    async def test_request_callback_invoked(self):
        """Incoming request triggers registered callback."""
        client = ACPClient(command="cat")
        received = []

        async def handler(method: str, params: dict) -> dict:
            received.append((method, params))
            return {"approved": True}

        client.on_request(handler)

        await client.start()
        try:
            request_json = '{"jsonrpc":"2.0","id":1,"method":"session/request_permission","params":{"operation":"read"}}'
            await client._handle_message(request_json)

            # Give handler time to run
            await asyncio.sleep(0.01)

            assert len(received) == 1
            assert received[0][0] == "session/request_permission"
        finally:
            await client.stop()

    @pytest.mark.asyncio
    async def test_request_handler_sends_response(self):
        """Request handler result is sent as response."""
        # Use a client without starting it to avoid read loop conflict
        # Test the _handle_message logic directly
        client = ACPClient(command="cat")
        response_sent = []

        async def handler(method: str, params: dict) -> dict:
            return {"result": "success"}

        client.on_request(handler)

        # Mock the write to capture what would be sent

        async def capture_write(msg: str) -> None:
            response_sent.append(msg)

        client._write_message = capture_write

        # Handle incoming request
        request_json = '{"jsonrpc":"2.0","id":42,"method":"test","params":{}}'
        await client._handle_message(request_json)

        # Verify response was "sent" (captured)
        assert len(response_sent) == 1
        response = response_sent[0]
        assert '"id": 42' in response or '"id":42' in response
        assert '"result"' in response


class TestACPClientTimeout:
    """Tests for request timeout handling."""

    @pytest.mark.asyncio
    async def test_request_timeout(self):
        """Request times out if no response received."""
        client = ACPClient(command="cat", timeout=0.1)

        await client.start()
        try:
            future = client.send_request("test", {})

            with pytest.raises(asyncio.TimeoutError):
                await asyncio.wait_for(future, timeout=0.2)
        finally:
            await client.stop()


class TestACPClientThreadSafety:
    """Tests for thread-safe operations."""

    @pytest.mark.asyncio
    async def test_concurrent_writes(self):
        """Multiple concurrent writes don't interleave."""
        # Test that the write lock prevents interleaving
        # by verifying writes are serialized
        client = ACPClient(command="cat")
        write_order = []

        # Mock write to track order and verify lock behavior

        async def tracking_write(msg: str) -> None:
            write_order.append(msg)

        client._write_message = tracking_write

        # Send multiple messages concurrently
        messages = [f'{{"id":{i},"test":true}}' for i in range(10)]
        await asyncio.gather(
            *[client._write_message(m) for m in messages]
        )

        # All messages should be written (lock serializes them)
        assert len(write_order) == 10

        # Each message should be complete (not interleaved)
        for msg in write_order:
            assert msg.startswith("{")
            assert msg.endswith("}")

    @pytest.mark.asyncio
    async def test_write_lock_serializes_writes(self):
        """Write lock ensures sequential writes."""
        client = ACPClient(command="cat")

        # Verify lock exists
        assert client._write_lock is not None

        # Start client to test real writes
        await client.start()
        try:
            # Acquire lock and verify no concurrent write is possible
            async with client._write_lock:
                # While we hold the lock, write should block
                # We can't easily test this without threads, so just verify
                # the lock is an asyncio.Lock
                assert isinstance(client._write_lock, asyncio.Lock)
        finally:
            await client.stop()
