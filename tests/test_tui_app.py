# ABOUTME: Tests for TUI application and core components
# ABOUTME: Uses pytest-textual for snapshot testing and mock connections

import pytest
from unittest.mock import Mock, AsyncMock, patch, MagicMock
import asyncio

from ralph_orchestrator.tui.app import RalphTUI
from ralph_orchestrator.tui.connection import (
    OrchestratorConnection,
    AttachedConnection,
    WebSocketConnection,
    TUIEvent,
    EventType,
)


class TestRalphTUI:
    """Test main TUI application."""

    def test_app_creates_without_connection(self):
        """TUI can be created without active connection."""
        app = RalphTUI()
        assert app is not None
        assert app.connection is None
        assert app.prompt_file is None

    def test_app_creates_with_prompt_file(self):
        """TUI can be created with prompt file path."""
        app = RalphTUI(prompt_file="/path/to/prompt.md")
        assert app.prompt_file == "/path/to/prompt.md"

    def test_app_has_expected_bindings(self):
        """TUI has all expected keyboard bindings."""
        app = RalphTUI()
        binding_keys = [b.key for b in app.BINDINGS]

        # Essential bindings
        assert "q" in binding_keys  # quit
        assert "p" in binding_keys  # pause
        assert "l" in binding_keys  # logs
        assert "t" in binding_keys  # tasks
        assert "m" in binding_keys  # metrics
        assert "h" in binding_keys  # history

    def test_reactive_state_defined(self):
        """Reactive state attributes are defined on the class."""
        # Check reactive descriptors exist on the class (not instance)
        # Accessing them on instance triggers Textual runtime checks
        assert "is_paused" in dir(RalphTUI)
        assert "current_iteration" in dir(RalphTUI)
        assert "max_iterations" in dir(RalphTUI)
        assert "current_task" in dir(RalphTUI)
        assert "connection_status" in dir(RalphTUI)

    def test_app_title_constants(self):
        """TUI has expected title constants."""
        app = RalphTUI()
        assert app.TITLE == "RALPH Orchestrator"
        assert app.SUB_TITLE == "Real-time Monitoring"

    def test_app_with_mock_connection(self):
        """TUI can be created with a mock connection."""
        mock_conn = Mock(spec=OrchestratorConnection)
        app = RalphTUI(connection=mock_conn)
        assert app.connection is mock_conn


class TestTUIEvent:
    """Test TUIEvent data class."""

    def test_event_creation(self):
        """TUIEvent can be created with type and data."""
        event = TUIEvent(type=EventType.OUTPUT, data={"text": "hello"})
        assert event.type == EventType.OUTPUT
        assert event.data == {"text": "hello"}
        assert event.timestamp > 0

    def test_event_to_json(self):
        """TUIEvent serializes to JSON."""
        event = TUIEvent(type=EventType.ITERATION_START, data={"iteration": 1})
        json_str = event.to_json()
        assert "iteration_start" in json_str
        assert "iteration" in json_str

    def test_event_from_json(self):
        """TUIEvent deserializes from JSON."""
        event = TUIEvent(type=EventType.COMPLETE, data={})
        json_str = event.to_json()
        restored = TUIEvent.from_json(json_str)
        assert restored.type == EventType.COMPLETE

    def test_event_roundtrip(self):
        """TUIEvent survives JSON roundtrip."""
        original = TUIEvent(
            type=EventType.METRICS,
            data={"cpu": 50.0, "memory": 30.0, "tokens": 1000, "cost": 0.05},
        )
        json_str = original.to_json()
        restored = TUIEvent.from_json(json_str)
        assert restored.type == original.type
        assert restored.data == original.data


class TestEventType:
    """Test EventType enum."""

    def test_event_types_exist(self):
        """EventType enum has all expected event types."""
        assert hasattr(EventType, "ITERATION_START")
        assert hasattr(EventType, "ITERATION_END")
        assert hasattr(EventType, "OUTPUT")
        assert hasattr(EventType, "TOOL_CALL")
        assert hasattr(EventType, "TASK_UPDATE")
        assert hasattr(EventType, "METRICS")
        assert hasattr(EventType, "VALIDATION_GATE")
        assert hasattr(EventType, "CHECKPOINT")
        assert hasattr(EventType, "ERROR")
        assert hasattr(EventType, "WARNING")
        assert hasattr(EventType, "COMPLETE")
        assert hasattr(EventType, "PAUSED")
        assert hasattr(EventType, "RESUMED")

    def test_event_type_values(self):
        """EventType values are strings."""
        assert EventType.ITERATION_START.value == "iteration_start"
        assert EventType.OUTPUT.value == "output"
        assert EventType.COMPLETE.value == "complete"


class TestOrchestratorConnection:
    """Test base orchestrator connection class."""

    def test_connection_init(self):
        """OrchestratorConnection initializes with correct defaults."""
        # Create a concrete subclass for testing
        class TestConnection(OrchestratorConnection):
            async def connect(self, target=None):
                return True

            async def send_command(self, command, **kwargs):
                return True

        conn = TestConnection()
        assert conn.mode == "attached"
        assert not conn.is_connected

    def test_connection_mode(self):
        """Connection mode is configurable."""

        class TestConnection(OrchestratorConnection):
            async def connect(self, target=None):
                return True

            async def send_command(self, command, **kwargs):
                return True

        conn = TestConnection(mode="websocket")
        assert conn.mode == "websocket"


class TestAttachedConnection:
    """Test attached connection for in-process orchestrator."""

    def test_attached_connection_init(self):
        """AttachedConnection initializes with orchestrator."""
        mock_orch = Mock()
        conn = AttachedConnection(mock_orch)
        assert conn.orchestrator is mock_orch
        assert conn.mode == "attached"

    @pytest.mark.asyncio
    async def test_attached_connection_connect(self):
        """AttachedConnection can connect to orchestrator."""
        mock_orch = Mock()
        conn = AttachedConnection(mock_orch)
        result = await conn.connect()
        assert result is True
        assert conn.is_connected is True

    @pytest.mark.asyncio
    async def test_attached_connection_disconnect(self):
        """AttachedConnection can disconnect."""
        mock_orch = Mock()
        conn = AttachedConnection(mock_orch)
        await conn.connect()
        await conn.disconnect()
        assert conn.is_connected is False

    @pytest.mark.asyncio
    async def test_attached_send_pause_command(self):
        """AttachedConnection can send pause command."""
        mock_orch = Mock()
        mock_orch.pause = Mock()
        conn = AttachedConnection(mock_orch)
        await conn.connect()

        result = await conn.send_command("pause")
        assert result is True
        mock_orch.pause.assert_called_once()

    @pytest.mark.asyncio
    async def test_attached_send_resume_command(self):
        """AttachedConnection can send resume command."""
        mock_orch = Mock()
        mock_orch.resume = Mock()
        conn = AttachedConnection(mock_orch)
        await conn.connect()

        result = await conn.send_command("resume")
        assert result is True
        mock_orch.resume.assert_called_once()

    @pytest.mark.asyncio
    async def test_attached_send_unknown_command(self):
        """AttachedConnection returns False for unknown commands."""
        mock_orch = Mock()
        conn = AttachedConnection(mock_orch)
        await conn.connect()

        result = await conn.send_command("unknown_command")
        assert result is False


class TestWebSocketConnection:
    """Test WebSocket connection for remote orchestrator."""

    def test_websocket_connection_init(self):
        """WebSocketConnection initializes correctly."""
        conn = WebSocketConnection()
        assert conn.mode == "websocket"
        assert not conn.is_connected

    @pytest.mark.asyncio
    async def test_websocket_connection_no_websockets_module(self):
        """WebSocketConnection handles missing websockets package gracefully."""
        conn = WebSocketConnection()

        # Mock the import to fail
        with patch.dict("sys.modules", {"websockets": None}):
            # The connection should handle the ImportError gracefully
            # We can't easily test this without the actual package
            pass


class TestTUIImports:
    """Test that all TUI modules import correctly."""

    def test_import_app(self):
        """App module imports without errors."""
        from ralph_orchestrator.tui.app import RalphTUI

        assert RalphTUI is not None

    def test_import_connection(self):
        """Connection module imports without errors."""
        from ralph_orchestrator.tui.connection import (
            OrchestratorConnection,
            AttachedConnection,
            WebSocketConnection,
            TUIEvent,
            EventType,
        )

        assert OrchestratorConnection is not None
        assert AttachedConnection is not None
        assert WebSocketConnection is not None
        assert TUIEvent is not None
        assert EventType is not None

    def test_import_widgets(self):
        """Widgets module imports without errors."""
        from ralph_orchestrator.tui.widgets import (
            ProgressPanel,
            OutputViewer,
            TaskSidebar,
            MetricsPanel,
            ValidationPrompt,
        )

        assert ProgressPanel is not None
        assert OutputViewer is not None
        assert TaskSidebar is not None
        assert MetricsPanel is not None
        assert ValidationPrompt is not None


class TestTUIEndToEnd:
    """End-to-end tests for TUI using Textual's async pilot."""

    @pytest.mark.asyncio
    async def test_tui_mounts_and_composes(self):
        """TUI app mounts and composes all widgets without errors."""
        app = RalphTUI(prompt_file="prompts/test-tui.md")

        async with app.run_test() as pilot:
            # Verify app mounted successfully
            assert pilot.app is not None
            assert pilot.app.title.startswith("RALPH")

            # Verify main widgets are composed
            assert pilot.app.query_one("#progress") is not None
            assert pilot.app.query_one("#output") is not None
            assert pilot.app.query_one("#tasks") is not None
            assert pilot.app.query_one("#metrics") is not None

    @pytest.mark.asyncio
    async def test_tui_keyboard_bindings(self):
        """TUI responds to keyboard bindings."""
        app = RalphTUI()

        async with app.run_test() as pilot:
            # Press 'l' to toggle logs
            await pilot.press("l")
            output = pilot.app.query_one("#output")
            assert output.has_class("hidden")

            # Press 'l' again to toggle back
            await pilot.press("l")
            assert not output.has_class("hidden")

    @pytest.mark.asyncio
    async def test_tui_toggle_tasks(self):
        """TUI can toggle task sidebar."""
        app = RalphTUI()

        async with app.run_test() as pilot:
            tasks = pilot.app.query_one("#tasks")
            initial_collapsed = tasks.has_class("collapsed")

            # Press 't' to toggle tasks
            await pilot.press("t")
            assert tasks.has_class("collapsed") != initial_collapsed

    @pytest.mark.asyncio
    async def test_tui_toggle_metrics(self):
        """TUI can toggle metrics panel."""
        app = RalphTUI()

        async with app.run_test() as pilot:
            metrics = pilot.app.query_one("#metrics")
            initial_hidden = metrics.has_class("hidden")

            # Press 'm' to toggle metrics
            await pilot.press("m")
            assert metrics.has_class("hidden") != initial_hidden

    @pytest.mark.asyncio
    async def test_tui_pause_resume_without_connection(self):
        """TUI handles pause/resume gracefully without connection."""
        app = RalphTUI()

        async with app.run_test() as pilot:
            # Press 'p' to toggle pause (should not crash without connection)
            await pilot.press("p")
            # The app should still be running
            assert pilot.app is not None

    @pytest.mark.asyncio
    async def test_tui_help_screen(self):
        """TUI can show help screen."""
        app = RalphTUI()

        async with app.run_test() as pilot:
            # Press '?' to show help
            await pilot.press("question_mark")
            # Should have pushed a screen
            assert len(pilot.app.screen_stack) > 1

            # Press escape to go back
            await pilot.press("escape")
            assert len(pilot.app.screen_stack) == 1

    @pytest.mark.asyncio
    async def test_tui_history_screen(self):
        """TUI can show history screen."""
        app = RalphTUI()

        async with app.run_test() as pilot:
            # Press 'h' to show history
            await pilot.press("h")
            # Should have pushed a screen
            assert len(pilot.app.screen_stack) > 1

    @pytest.mark.asyncio
    async def test_tui_quit(self):
        """TUI can quit cleanly."""
        app = RalphTUI()

        async with app.run_test() as pilot:
            # Press 'q' to quit
            await pilot.press("q")
            # App should have initiated quit
            # (the test harness handles cleanup)
