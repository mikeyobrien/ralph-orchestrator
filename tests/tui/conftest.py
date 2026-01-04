"""Pytest fixtures for TUI tests."""

import pytest
from ralph_orchestrator.tui import RalphTUI
from ralph_orchestrator.tui.connection import TUIEvent, EventType, OrchestratorConnection


@pytest.fixture
def app():
    """Create TUI app instance."""
    return RalphTUI()


@pytest.fixture
def mock_events():
    """Generate mock TUI events for testing."""
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
        TUIEvent(
            type=EventType.COMPLETE,
            data={}
        ),
    ]


class MockConnection(OrchestratorConnection):
    """Mock connection for testing."""

    def __init__(self, events=None):
        super().__init__(mode="mock")
        self._mock_events = events or []

    async def connect(self, target=None):
        self._connected = True
        # Queue mock events
        for event in self._mock_events:
            self._emit_event(event)
        return True

    async def send_command(self, command, **kwargs):
        return True


@pytest.fixture
def mock_connection(mock_events):
    """Create mock connection with test events."""
    return MockConnection(mock_events)
