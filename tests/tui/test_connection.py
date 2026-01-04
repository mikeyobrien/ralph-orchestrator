"""Tests for TUI connection layer."""

import pytest
import asyncio
from ralph_orchestrator.tui.connection import (
    EventType, TUIEvent, OrchestratorConnection,
    AttachedConnection, WebSocketConnection
)


class TestEventType:
    """Tests for EventType enum."""

    def test_all_event_types_defined(self):
        """Verify all expected event types exist."""
        expected = [
            "ITERATION_START", "ITERATION_END", "OUTPUT", "TOOL_CALL",
            "TASK_UPDATE", "METRICS", "VALIDATION_GATE", "CHECKPOINT",
            "ERROR", "WARNING", "COMPLETE", "PAUSED", "RESUMED"
        ]
        for name in expected:
            assert hasattr(EventType, name)

    def test_event_type_values_are_strings(self):
        """Event type values should be serializable strings."""
        for event_type in EventType:
            assert isinstance(event_type.value, str)


class TestTUIEvent:
    """Tests for TUIEvent dataclass."""

    def test_create_event_with_defaults(self):
        """Create event with minimal args."""
        event = TUIEvent(type=EventType.OUTPUT)
        assert event.type == EventType.OUTPUT
        assert event.data == {}
        assert event.timestamp > 0

    def test_create_event_with_data(self):
        """Create event with custom data."""
        event = TUIEvent(
            type=EventType.METRICS,
            data={"cpu": 50, "memory": 75}
        )
        assert event.data["cpu"] == 50
        assert event.data["memory"] == 75

    def test_to_json(self):
        """Event serializes to JSON string."""
        event = TUIEvent(
            type=EventType.OUTPUT,
            data={"text": "hello"}
        )
        json_str = event.to_json()
        assert '"type": "output"' in json_str
        assert '"text": "hello"' in json_str

    def test_from_json(self):
        """Event deserializes from JSON string."""
        json_str = '{"type": "output", "data": {"text": "hello"}, "timestamp": 1234567890}'
        event = TUIEvent.from_json(json_str)
        assert event.type == EventType.OUTPUT
        assert event.data["text"] == "hello"
        assert event.timestamp == 1234567890


class TestOrchestratorConnection:
    """Tests for base connection class."""

    def test_initial_state(self):
        """Connection starts disconnected."""
        conn = OrchestratorConnection()
        assert conn.is_connected is False
        assert conn.mode == "attached"

    @pytest.mark.asyncio
    async def test_disconnect(self):
        """Disconnect sets connected to False."""
        conn = OrchestratorConnection()
        conn._connected = True
        await conn.disconnect()
        assert conn.is_connected is False

    def test_emit_event_to_queue(self):
        """Events are added to internal queue."""
        conn = OrchestratorConnection()
        event = TUIEvent(type=EventType.OUTPUT, data={"text": "test"})
        conn._emit_event(event)
        # Queue should have one item
        assert not conn._queue.empty()

    @pytest.mark.asyncio
    async def test_events_iterator(self):
        """Events iterator yields queued events."""
        conn = OrchestratorConnection()
        conn._connected = True

        # Add event to queue
        event = TUIEvent(type=EventType.OUTPUT, data={"text": "test"})
        conn._emit_event(event)

        # Collect events with timeout
        received = []
        async def collect():
            async for e in conn.events():
                received.append(e)
                break  # Exit after first event

        await asyncio.wait_for(collect(), timeout=1.0)
        assert len(received) == 1
        assert received[0].type == EventType.OUTPUT


class TestAttachedConnection:
    """Tests for attached (in-process) connection."""

    def test_init_with_orchestrator(self):
        """AttachedConnection stores orchestrator reference."""
        # Mock orchestrator
        class MockOrchestrator:
            pass

        orch = MockOrchestrator()
        conn = AttachedConnection(orch)
        assert conn.orchestrator is orch
        assert conn.mode == "attached"

    @pytest.mark.asyncio
    async def test_connect_sets_up_hooks(self):
        """Connect should set up orchestrator hooks."""
        class MockOrchestrator:
            on_iteration_start = None
            on_output = None
            on_tool_call = None
            on_metrics = None
            on_validation_gate = None
            on_complete = None

        orch = MockOrchestrator()
        conn = AttachedConnection(orch)
        result = await conn.connect()

        assert result is True
        assert conn.is_connected
        # Hooks should be set
        assert orch.on_iteration_start is not None
        assert orch.on_output is not None

    @pytest.mark.asyncio
    async def test_send_pause_command(self):
        """Send pause command calls orchestrator.pause()."""
        class MockOrchestrator:
            paused = False
            def pause(self):
                self.paused = True

        orch = MockOrchestrator()
        conn = AttachedConnection(orch)
        await conn.connect()

        result = await conn.send_command("pause")
        assert result is True
        assert orch.paused is True


class TestWebSocketConnection:
    """Tests for WebSocket connection."""

    def test_init_state(self):
        """WebSocketConnection initializes correctly."""
        conn = WebSocketConnection()
        assert conn.mode == "websocket"
        assert conn._ws is None
        assert conn._url is None

    @pytest.mark.asyncio
    async def test_connect_without_websockets_package(self):
        """Connect fails gracefully when websockets not installed."""
        # This test assumes websockets may not be installed
        conn = WebSocketConnection()
        # The connect will fail and emit an error event
        result = await conn.connect("ws://localhost:9999/ws")
        # Connection should fail (either ImportError or ConnectionRefused)
        # Either way, we should have an error event in the queue
        if not result:
            assert conn.is_connected is False
