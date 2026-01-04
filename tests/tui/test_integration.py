"""Integration tests for TUI with mock orchestrator."""

import pytest
import asyncio
from ralph_orchestrator.tui import RalphTUI
from ralph_orchestrator.tui.connection import OrchestratorConnection, TUIEvent, EventType


class MockConnection(OrchestratorConnection):
    """Mock connection for testing."""

    def __init__(self, events=None):
        super().__init__(mode="mock")
        self._mock_events = events or []
        self.commands_sent = []

    async def connect(self, target=None):
        self._connected = True
        # Queue mock events
        for event in self._mock_events:
            self._emit_event(event)
        return True

    async def send_command(self, command, **kwargs):
        self.commands_sent.append((command, kwargs))
        return True


@pytest.fixture
def integration_events():
    """Events for integration testing."""
    return [
        TUIEvent(
            type=EventType.ITERATION_START,
            data={"iteration": 1, "max_iterations": 10, "task": "Test task"}
        ),
        TUIEvent(
            type=EventType.OUTPUT,
            data={"text": "Processing..."}
        ),
        TUIEvent(
            type=EventType.TOOL_CALL,
            data={
                "name": "write_file",
                "input": {"path": "test.py", "content": "print('hello')"},
                "result": "File written successfully",
                "status": "success"
            }
        ),
        TUIEvent(
            type=EventType.METRICS,
            data={"cpu": 45, "memory": 60, "tokens": 15000, "cost": 1.25}
        ),
    ]


class TestTUIIntegration:
    """Integration tests for full TUI flow."""

    @pytest.mark.asyncio
    async def test_app_creates_without_error(self):
        """TUI app can be instantiated."""
        connection = MockConnection()
        app = RalphTUI(connection=connection)
        assert app is not None

    @pytest.mark.asyncio
    async def test_app_with_prompt_file(self):
        """TUI app accepts prompt_file parameter."""
        connection = MockConnection()
        app = RalphTUI(connection=connection, prompt_file="test.md")
        assert app is not None

    @pytest.mark.asyncio
    async def test_connection_receives_events(self, integration_events):
        """Connection queues events correctly."""
        connection = MockConnection(integration_events)
        await connection.connect()

        # Events should be in queue
        assert connection.is_connected
        assert not connection._queue.empty()

    @pytest.mark.asyncio
    async def test_send_pause_command(self):
        """TUI can send pause command through connection."""
        connection = MockConnection()
        await connection.connect()
        await connection.send_command("pause")

        assert ("pause", {}) in connection.commands_sent

    @pytest.mark.asyncio
    async def test_send_checkpoint_command(self):
        """TUI can send checkpoint command through connection."""
        connection = MockConnection()
        await connection.connect()
        await connection.send_command("checkpoint")

        assert ("checkpoint", {}) in connection.commands_sent

    @pytest.mark.asyncio
    async def test_send_validation_response(self):
        """TUI can send validation response through connection."""
        connection = MockConnection()
        await connection.connect()
        await connection.send_command(
            "validation_response",
            approved=True,
            skipped=False
        )

        assert len(connection.commands_sent) == 1
        cmd, kwargs = connection.commands_sent[0]
        assert cmd == "validation_response"
        assert kwargs["approved"] is True


class TestTUIEventFlow:
    """Tests for event processing flow."""

    @pytest.mark.asyncio
    async def test_events_stream_correctly(self, integration_events):
        """Events stream in order."""
        connection = MockConnection(integration_events)
        await connection.connect()

        received = []
        async def collect():
            async for event in connection.events():
                received.append(event)
                if len(received) >= len(integration_events):
                    break

        # Collect with timeout
        try:
            await asyncio.wait_for(collect(), timeout=2.0)
        except asyncio.TimeoutError:
            pass

        assert len(received) == len(integration_events)
        assert received[0].type == EventType.ITERATION_START
        assert received[1].type == EventType.OUTPUT

    @pytest.mark.asyncio
    async def test_iteration_event_data(self, integration_events):
        """Iteration events contain expected data."""
        connection = MockConnection(integration_events)
        await connection.connect()

        # Get first event
        event = await connection._queue.get()
        assert event.type == EventType.ITERATION_START
        assert event.data["iteration"] == 1
        assert event.data["max_iterations"] == 10
        assert event.data["task"] == "Test task"

    @pytest.mark.asyncio
    async def test_metrics_event_data(self, integration_events):
        """Metrics events contain expected data."""
        connection = MockConnection(integration_events)
        await connection.connect()

        # Find metrics event
        metrics_event = None
        for _ in range(len(integration_events)):
            event = await connection._queue.get()
            if event.type == EventType.METRICS:
                metrics_event = event
                break

        assert metrics_event is not None
        assert metrics_event.data["cpu"] == 45
        assert metrics_event.data["memory"] == 60
        assert metrics_event.data["tokens"] == 15000
        assert metrics_event.data["cost"] == 1.25
