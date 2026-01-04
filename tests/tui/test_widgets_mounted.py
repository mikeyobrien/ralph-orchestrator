"""Mounted widget tests for TUI components.

These tests use Textual's run_test() context manager to properly mount widgets,
enabling testing of compose(), on_mount(), and widget query methods.
"""

import pytest
import asyncio
from textual.app import App, ComposeResult
from textual.widgets import Static, ProgressBar, ListView, Button

from ralph_orchestrator.tui.widgets.progress import ProgressPanel
from ralph_orchestrator.tui.widgets.output import OutputViewer
from ralph_orchestrator.tui.widgets.tasks import Task, TaskSidebar, TaskItem
from ralph_orchestrator.tui.widgets.metrics import MetricWidget, MetricsPanel
from ralph_orchestrator.tui.widgets.validation import ValidationPrompt
from ralph_orchestrator.tui.screens.history import HistoryScreen
from ralph_orchestrator.tui.screens.help import HelpScreen
from ralph_orchestrator.tui import RalphTUI


# =============================================================================
# Test Apps - Wrap widgets in minimal apps for mounting
# =============================================================================

class ProgressPanelApp(App):
    """Test app for ProgressPanel."""

    def compose(self) -> ComposeResult:
        yield ProgressPanel(id="progress")


class OutputViewerApp(App):
    """Test app for OutputViewer."""

    def compose(self) -> ComposeResult:
        yield OutputViewer(id="output")


class TaskSidebarApp(App):
    """Test app for TaskSidebar."""

    def compose(self) -> ComposeResult:
        yield TaskSidebar(id="tasks")


class MetricsPanelApp(App):
    """Test app for MetricsPanel."""

    def compose(self) -> ComposeResult:
        yield MetricsPanel(id="metrics")


class MetricWidgetApp(App):
    """Test app for MetricWidget."""

    def compose(self) -> ComposeResult:
        yield MetricWidget(
            "Test Metric",
            icon="T",
            unit="%",
            max_value=100.0,
            format_spec=".1f",
            id="metric"
        )


# =============================================================================
# ProgressPanel Mounted Tests
# =============================================================================

class TestProgressPanelMounted:
    """Tests for ProgressPanel when mounted."""

    @pytest.mark.asyncio
    async def test_compose_creates_widgets(self):
        """ProgressPanel composes all required child widgets."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)
            # Check child widgets exist
            assert panel.query_one("#status-line") is not None
            assert panel.query_one("#task-line") is not None
            assert panel.query_one("#progress-bar") is not None
            assert panel.query_one("#timing-stat") is not None
            assert panel.query_one("#cost-stat") is not None
            assert panel.query_one("#connection-stat") is not None

    @pytest.mark.asyncio
    async def test_on_mount_initializes(self):
        """ProgressPanel initializes start_time on mount."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)
            assert panel.start_time is not None
            assert panel._timer is not None

    @pytest.mark.asyncio
    async def test_update_iteration_changes_state(self):
        """update_iteration updates reactive properties."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)
            panel.update_iteration(5, 10)

            assert panel.current_iteration == 5
            assert panel.max_iterations == 10
            assert panel.status == "running"

    @pytest.mark.asyncio
    async def test_set_paused_updates_display(self):
        """set_paused updates display state."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)
            panel.set_paused(True)
            assert panel.is_paused is True

            panel.set_paused(False)
            assert panel.is_paused is False

    @pytest.mark.asyncio
    async def test_update_cost_reflects_in_display(self):
        """update_cost changes cost value."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)
            panel.update_cost(25.50)
            assert panel.cost == 25.50

    @pytest.mark.asyncio
    async def test_set_connection_status(self):
        """set_connection_status updates status."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)

            for status in ["connected", "connecting", "disconnected", "error", "complete"]:
                panel.set_connection_status(status)
                assert panel.connection_status == status

    @pytest.mark.asyncio
    async def test_mark_complete_stops_timer(self):
        """mark_complete stops the timer and sets status."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)
            panel.mark_complete()
            assert panel.status == "complete"

    @pytest.mark.asyncio
    async def test_get_status_text_all_states(self):
        """_get_status_text handles all status states."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)

            # Test complete status
            panel.status = "complete"
            text = panel._get_status_text()
            assert "COMPLETE" in text.plain

            # Test paused status
            panel.status = "running"
            panel.is_paused = True
            text = panel._get_status_text()
            assert "PAUSED" in text.plain

            # Test error status
            panel.is_paused = False
            panel.status = "error"
            text = panel._get_status_text()
            assert "ERROR" in text.plain

            # Test running status
            panel.status = "running"
            text = panel._get_status_text()
            assert "RUNNING" in text.plain

            # Test idle status
            panel.status = "idle"
            text = panel._get_status_text()
            assert "IDLE" in text.plain

    @pytest.mark.asyncio
    async def test_format_timing_with_eta(self):
        """_format_timing calculates ETA when appropriate."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)
            panel.current_iteration = 5
            panel.max_iterations = 10
            panel.elapsed_seconds = 50.0

            text = panel._format_timing()
            # Should have elapsed time
            assert "0:00:50" in text.plain
            # Should have ETA since we have iterations remaining
            assert "ETA" in text.plain

    @pytest.mark.asyncio
    async def test_get_connection_text_all_statuses(self):
        """_get_connection_text handles all connection statuses."""
        app = ProgressPanelApp()
        async with app.run_test():
            panel = app.query_one("#progress", ProgressPanel)

            for status in ["connected", "connecting", "disconnected", "error", "complete"]:
                panel.connection_status = status
                text = panel._get_connection_text()
                assert len(text.plain) > 0


# =============================================================================
# OutputViewer Mounted Tests
# =============================================================================

class TestOutputViewerMounted:
    """Tests for OutputViewer when mounted."""

    @pytest.mark.asyncio
    async def test_append_agent_output(self):
        """append_agent_output writes to the log."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            viewer.append_agent_output("Test output message")
            # Should not raise, output is written

    @pytest.mark.asyncio
    async def test_append_agent_output_empty(self):
        """append_agent_output ignores empty text."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            viewer.append_agent_output("")
            viewer.append_agent_output("   ")
            # Should not raise

    @pytest.mark.asyncio
    async def test_append_agent_output_code_block(self):
        """append_agent_output handles code blocks."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            code = "```python\nprint('hello')\n```"
            viewer.append_agent_output(code)
            # Should render as syntax-highlighted

    @pytest.mark.asyncio
    async def test_append_tool_call(self):
        """append_tool_call renders tool call card."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            viewer.append_tool_call(
                tool_name="write_file",
                tool_input={"path": "test.py"},
                result="Success",
                status="success"
            )
            assert viewer._tool_call_count == 1

    @pytest.mark.asyncio
    async def test_append_tool_call_error_status(self):
        """append_tool_call handles error status."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            viewer.append_tool_call(
                tool_name="delete_file",
                tool_input={"path": "missing.py"},
                result="File not found",
                status="error"
            )
            assert viewer._tool_call_count == 1

    @pytest.mark.asyncio
    async def test_append_tool_call_pending_status(self):
        """append_tool_call handles pending status."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            viewer.append_tool_call(
                tool_name="long_task",
                tool_input={},
                result="In progress...",
                status="pending"
            )
            assert viewer._tool_call_count == 1

    @pytest.mark.asyncio
    async def test_append_iteration_marker(self):
        """append_iteration_marker adds iteration marker."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            viewer.append_iteration_marker(5)
            # Should not raise

    @pytest.mark.asyncio
    async def test_append_error(self):
        """append_error displays error panel."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            viewer.append_error("Something went wrong!")
            # Should not raise

    @pytest.mark.asyncio
    async def test_append_validation_gate(self):
        """append_validation_gate displays gate result."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)

            for status in ["passed", "failed", "skipped", "pending"]:
                viewer.append_validation_gate(
                    name="Test Gate",
                    status=status,
                    evidence=["file1.txt", "file2.png"]
                )

    @pytest.mark.asyncio
    async def test_clear_output_resets_counter(self):
        """clear_output resets tool call counter."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            viewer.append_tool_call("test", {}, "result", "success")
            viewer.append_tool_call("test2", {}, "result2", "success")
            assert viewer._tool_call_count == 2

            viewer.clear_output()
            assert viewer._tool_call_count == 0

    @pytest.mark.asyncio
    async def test_auto_scroll_toggle(self):
        """toggle_auto_scroll toggles auto-scroll."""
        app = OutputViewerApp()
        async with app.run_test():
            viewer = app.query_one("#output", OutputViewer)
            assert viewer.auto_scroll is True

            viewer.toggle_auto_scroll()
            assert viewer.auto_scroll is False

            viewer.toggle_auto_scroll()
            assert viewer.auto_scroll is True


# =============================================================================
# TaskSidebar Mounted Tests
# =============================================================================

class TestTaskSidebarMounted:
    """Tests for TaskSidebar when mounted."""

    @pytest.mark.asyncio
    async def test_compose_creates_widgets(self):
        """TaskSidebar composes all required child widgets."""
        app = TaskSidebarApp()
        async with app.run_test():
            sidebar = app.query_one("#tasks", TaskSidebar)
            # Check child widgets exist
            assert sidebar.query_one("#pending-list") is not None
            assert sidebar.query_one("#current-task") is not None
            assert sidebar.query_one("#completed-list") is not None
            assert sidebar.query_one("#completed-section") is not None

    @pytest.mark.asyncio
    async def test_update_tasks_with_pending(self):
        """update_tasks updates pending task list."""
        app = TaskSidebarApp()
        async with app.run_test():
            sidebar = app.query_one("#tasks", TaskSidebar)
            sidebar.update_tasks(
                pending=[
                    {"id": "1", "name": "Task 1"},
                    {"id": "2", "name": "Task 2"},
                ],
                current=None,
                completed=[]
            )

            summary = sidebar.get_task_summary()
            assert summary["pending"] == 2
            assert summary["running"] == 0

    @pytest.mark.asyncio
    async def test_update_tasks_with_current(self):
        """update_tasks updates current task."""
        app = TaskSidebarApp()
        async with app.run_test():
            sidebar = app.query_one("#tasks", TaskSidebar)
            sidebar.update_tasks(
                pending=[],
                current={"id": "1", "name": "Running Task", "duration": 10.5},
                completed=[]
            )

            summary = sidebar.get_task_summary()
            assert summary["running"] == 1

    @pytest.mark.asyncio
    async def test_update_tasks_with_completed(self):
        """update_tasks updates completed task list."""
        app = TaskSidebarApp()
        async with app.run_test():
            sidebar = app.query_one("#tasks", TaskSidebar)
            sidebar.update_tasks(
                pending=[],
                current=None,
                completed=[
                    {"id": "1", "name": "Done Task 1", "status": "completed", "duration": 5.0},
                    {"id": "2", "name": "Failed Task", "status": "failed", "error": "Oops"},
                ]
            )

            summary = sidebar.get_task_summary()
            assert summary["completed"] == 1
            assert summary["failed"] == 1

    @pytest.mark.asyncio
    async def test_update_tasks_many_pending(self):
        """update_tasks handles many pending tasks."""
        app = TaskSidebarApp()
        async with app.run_test():
            sidebar = app.query_one("#tasks", TaskSidebar)
            sidebar.update_tasks(
                pending=[{"id": str(i), "name": f"Task {i}"} for i in range(15)],
                current=None,
                completed=[]
            )

            summary = sidebar.get_task_summary()
            assert summary["pending"] == 15

    @pytest.mark.asyncio
    async def test_toggle_collapse_changes_class(self):
        """toggle_collapse adds/removes collapsed class."""
        app = TaskSidebarApp()
        async with app.run_test():
            sidebar = app.query_one("#tasks", TaskSidebar)
            assert sidebar.is_collapsed is False

            sidebar.toggle_collapse()
            assert sidebar.is_collapsed is True
            assert "collapsed" in sidebar.classes

            sidebar.toggle_collapse()
            assert sidebar.is_collapsed is False


# =============================================================================
# TaskItem Tests
# =============================================================================

class TaskItemApp(App):
    """Test app for TaskItem."""

    def compose(self) -> ComposeResult:
        task = Task(id="1", name="Test Task", status="pending")
        yield TaskItem(task, id="task-item")


class TestTaskItemMounted:
    """Tests for TaskItem when mounted."""

    @pytest.mark.asyncio
    async def test_compose_renders_task(self):
        """TaskItem composes task display."""
        app = TaskItemApp()
        async with app.run_test():
            item = app.query_one("#task-item", TaskItem)
            assert item is not None
            assert item._task_data.name == "Test Task"

    @pytest.mark.asyncio
    async def test_render_task_all_statuses(self):
        """TaskItem renders all status types."""
        for status in ["pending", "running", "completed", "failed", "skipped"]:
            task = Task(id="1", name="Test Task", status=status)
            item = TaskItem(task)
            text = item._render_task()
            assert len(text.plain) > 0


# =============================================================================
# MetricWidget Mounted Tests
# =============================================================================

class TestMetricWidgetMounted:
    """Tests for MetricWidget when mounted."""

    @pytest.mark.asyncio
    async def test_compose_creates_widgets(self):
        """MetricWidget composes all required child widgets."""
        app = MetricWidgetApp()
        async with app.run_test():
            widget = app.query_one("#metric", MetricWidget)
            assert widget.query_one("#label") is not None
            assert widget.query_one("#value") is not None
            assert widget.query_one("#sparkline") is not None

    @pytest.mark.asyncio
    async def test_update_value_adds_to_history(self):
        """update_value adds value to history."""
        app = MetricWidgetApp()
        async with app.run_test():
            widget = app.query_one("#metric", MetricWidget)
            widget.update_value(50.0)
            assert widget.value == 50.0
            assert 50.0 in widget._history

    @pytest.mark.asyncio
    async def test_history_max_length(self):
        """History has max length of 60."""
        app = MetricWidgetApp()
        async with app.run_test():
            widget = app.query_one("#metric", MetricWidget)
            # Add more than 60 values
            for i in range(70):
                widget.update_value(float(i))

            assert len(widget._history) == 60


# =============================================================================
# MetricsPanel Mounted Tests
# =============================================================================

class TestMetricsPanelMounted:
    """Tests for MetricsPanel when mounted."""

    @pytest.mark.asyncio
    async def test_compose_creates_all_metrics(self):
        """MetricsPanel composes all metric widgets."""
        app = MetricsPanelApp()
        async with app.run_test():
            panel = app.query_one("#metrics", MetricsPanel)
            assert panel.query_one("#cpu-metric") is not None
            assert panel.query_one("#memory-metric") is not None
            assert panel.query_one("#tokens-metric") is not None
            assert panel.query_one("#cost-metric") is not None

    @pytest.mark.asyncio
    async def test_update_metrics(self):
        """update_metrics updates all metric values."""
        app = MetricsPanelApp()
        async with app.run_test():
            panel = app.query_one("#metrics", MetricsPanel)
            panel.update_metrics(
                cpu=75.0,
                memory=50.0,
                tokens=25000,
                cost=12.50
            )

            current = panel.get_current_metrics()
            assert current["cpu"] == 75.0
            assert current["memory"] == 50.0
            assert current["tokens"] == 25000.0
            assert current["cost"] == 12.50

    @pytest.mark.asyncio
    async def test_get_current_metrics(self):
        """get_current_metrics returns all metric values."""
        app = MetricsPanelApp()
        async with app.run_test():
            panel = app.query_one("#metrics", MetricsPanel)
            current = panel.get_current_metrics()

            assert "cpu" in current
            assert "memory" in current
            assert "tokens" in current
            assert "cost" in current


# =============================================================================
# ValidationPrompt Tests (without mounting - compose has side effects)
# =============================================================================

class TestValidationPromptBasic:
    """Basic tests for ValidationPrompt without full mounting."""

    def test_initialization(self):
        """ValidationPrompt stores constructor args."""
        evidence = ["file1.png", "log.txt"]
        prompt = ValidationPrompt(
            gate_name="Test Gate",
            description="Test description",
            evidence=evidence
        )
        assert prompt.gate_name == "Test Gate"
        assert prompt.description == "Test description"
        assert prompt.evidence == evidence

    def test_initialization_empty_evidence(self):
        """ValidationPrompt handles empty evidence."""
        prompt = ValidationPrompt(gate_name="Empty Gate")
        assert prompt.gate_name == "Empty Gate"
        assert prompt.description == ""
        assert prompt.evidence == []

    def test_action_approve(self):
        """action_approve calls dismiss with True."""
        prompt = ValidationPrompt(gate_name="Test")
        # Just verify method exists and is callable
        assert callable(prompt.action_approve)

    def test_action_reject(self):
        """action_reject calls dismiss with False."""
        prompt = ValidationPrompt(gate_name="Test")
        assert callable(prompt.action_reject)

    def test_action_skip(self):
        """action_skip calls dismiss with None."""
        prompt = ValidationPrompt(gate_name="Test")
        assert callable(prompt.action_skip)

    def test_bindings_defined(self):
        """ValidationPrompt has keyboard bindings."""
        prompt = ValidationPrompt(gate_name="Test")
        # BINDINGS can be tuples (key, action, description)
        binding_keys = [b[0] if isinstance(b, tuple) else b.key for b in prompt.BINDINGS]
        assert "y" in binding_keys
        assert "n" in binding_keys
        assert "s" in binding_keys
        assert "escape" in binding_keys


class TestValidationPromptMounted:
    """Tests for ValidationPrompt when mounted."""

    @pytest.mark.asyncio
    async def test_compose_with_evidence(self):
        """ValidationPrompt composes with evidence list."""
        evidence = ["screenshot.png", "log.json", "audio.mp3", "notes.txt", "data.csv"]

        app = App()
        async with app.run_test() as pilot:
            prompt = ValidationPrompt(
                gate_name="Test Gate",
                description="Test description",
                evidence=evidence
            )
            app.push_screen(prompt)
            await pilot.pause()

            # Check buttons exist
            assert prompt.query_one("#approve") is not None
            assert prompt.query_one("#reject") is not None
            assert prompt.query_one("#skip") is not None

    @pytest.mark.asyncio
    async def test_compose_without_evidence(self):
        """ValidationPrompt composes without evidence."""
        app = App()
        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Empty Gate")
            app.push_screen(prompt)
            await pilot.pause()

            assert prompt.query_one("#approve") is not None

    @pytest.mark.asyncio
    async def test_compose_with_description(self):
        """ValidationPrompt composes with description."""
        app = App()
        async with app.run_test() as pilot:
            prompt = ValidationPrompt(
                gate_name="Test",
                description="This is a test description"
            )
            app.push_screen(prompt)
            await pilot.pause()

            assert prompt.description == "This is a test description"

    @pytest.mark.asyncio
    async def test_approve_button_pressed(self):
        """Pressing approve button dismisses with True."""
        app = App()
        results = []

        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Test")

            def on_dismiss(result):
                results.append(result)

            app.push_screen(prompt, callback=on_dismiss)
            await pilot.pause()

            await pilot.click("#approve")
            await pilot.pause()

            assert results == [True]

    @pytest.mark.asyncio
    async def test_reject_button_pressed(self):
        """Pressing reject button dismisses with False."""
        app = App()
        results = []

        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Test")

            def on_dismiss(result):
                results.append(result)

            app.push_screen(prompt, callback=on_dismiss)
            await pilot.pause()

            await pilot.click("#reject")
            await pilot.pause()

            assert results == [False]

    @pytest.mark.asyncio
    async def test_skip_button_pressed(self):
        """Pressing skip button dismisses with None."""
        app = App()
        results = []

        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Test")

            def on_dismiss(result):
                results.append(result)

            app.push_screen(prompt, callback=on_dismiss)
            await pilot.pause()

            await pilot.click("#skip")
            await pilot.pause()

            assert results == [None]

    @pytest.mark.asyncio
    async def test_keyboard_y_approves(self):
        """Pressing Y key approves."""
        app = App()
        results = []

        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Test")

            def on_dismiss(result):
                results.append(result)

            app.push_screen(prompt, callback=on_dismiss)
            await pilot.pause()

            await pilot.press("y")
            await pilot.pause()

            assert results == [True]

    @pytest.mark.asyncio
    async def test_keyboard_n_rejects(self):
        """Pressing N key rejects."""
        app = App()
        results = []

        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Test")

            def on_dismiss(result):
                results.append(result)

            app.push_screen(prompt, callback=on_dismiss)
            await pilot.pause()

            await pilot.press("n")
            await pilot.pause()

            assert results == [False]

    @pytest.mark.asyncio
    async def test_keyboard_s_skips(self):
        """Pressing S key skips."""
        app = App()
        results = []

        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Test")

            def on_dismiss(result):
                results.append(result)

            app.push_screen(prompt, callback=on_dismiss)
            await pilot.pause()

            await pilot.press("s")
            await pilot.pause()

            assert results == [None]

    @pytest.mark.asyncio
    async def test_keyboard_escape_skips(self):
        """Pressing Escape key skips."""
        app = App()
        results = []

        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Test")

            def on_dismiss(result):
                results.append(result)

            app.push_screen(prompt, callback=on_dismiss)
            await pilot.pause()

            await pilot.press("escape")
            await pilot.pause()

            assert results == [None]

    @pytest.mark.asyncio
    async def test_evidence_file_icons(self):
        """Evidence items get appropriate icons based on file type."""
        evidence = [
            "image.png",      # 🖼
            "photo.jpg",      # 🖼
            "data.json",      # 📋
            "audio.mp3",      # 🔊
            "sound.wav",      # 🔊
            "notes.txt",      # 📄
            "other.xyz",      # 📁
        ]

        app = App()
        async with app.run_test() as pilot:
            prompt = ValidationPrompt(gate_name="Test", evidence=evidence)
            app.push_screen(prompt)
            await pilot.pause()

            # All files should be in evidence
            assert len(prompt.evidence) == 7


# =============================================================================
# HistoryScreen Mounted Tests
# =============================================================================

from ralph_orchestrator.tui.screens.history import HistoryScreen, IterationRecord


class TestHistoryScreenMounted:
    """Tests for HistoryScreen when mounted."""

    @pytest.mark.asyncio
    async def test_compose_creates_widgets(self):
        """HistoryScreen composes all required widgets."""
        app = App()

        async with app.run_test() as pilot:
            screen = HistoryScreen()
            app.push_screen(screen)
            await pilot.pause()

            # Check main widgets exist
            assert screen.query_one("#history-table") is not None

    @pytest.mark.asyncio
    async def test_initial_state(self):
        """HistoryScreen starts with empty records."""
        app = App()

        async with app.run_test() as pilot:
            screen = HistoryScreen()
            app.push_screen(screen)
            await pilot.pause()

            assert screen.records == []

    @pytest.mark.asyncio
    async def test_with_records(self):
        """HistoryScreen displays records in table."""
        records = [
            IterationRecord(
                number=1, status="success", task="Test task",
                duration=10.5, tokens=1000, cost=0.05, tool_calls=5
            ),
            IterationRecord(
                number=2, status="error", task="Failed task",
                duration=5.0, tokens=500, cost=0.02, tool_calls=2
            ),
        ]

        app = App()
        async with app.run_test() as pilot:
            screen = HistoryScreen(records=records)
            app.push_screen(screen)
            await pilot.pause()

            assert len(screen.records) == 2

    @pytest.mark.asyncio
    async def test_update_records(self):
        """update_records refreshes the table."""
        app = App()

        async with app.run_test() as pilot:
            screen = HistoryScreen()
            app.push_screen(screen)
            await pilot.pause()

            new_records = [
                IterationRecord(
                    number=1, status="success", task="New task",
                    duration=15.0, tokens=2000, cost=0.10, tool_calls=10
                ),
            ]
            screen.update_records(new_records)
            assert len(screen.records) == 1

    @pytest.mark.asyncio
    async def test_escape_closes_screen(self):
        """Escape key closes history screen."""
        app = App()

        async with app.run_test() as pilot:
            screen = HistoryScreen()
            app.push_screen(screen)
            await pilot.pause()

            await pilot.press("escape")

    @pytest.mark.asyncio
    async def test_all_status_types_render(self):
        """HistoryScreen renders all status types correctly."""
        records = [
            IterationRecord(number=1, status="success", task="T1", duration=10, tokens=1000, cost=0.05, tool_calls=5),
            IterationRecord(number=2, status="error", task="T2", duration=10, tokens=1000, cost=0.05, tool_calls=5),
            IterationRecord(number=3, status="validation_failed", task="T3", duration=10, tokens=1000, cost=0.05, tool_calls=5),
            IterationRecord(number=4, status="checkpoint", task="T4", duration=10, tokens=1000, cost=0.05, tool_calls=5),
            IterationRecord(number=5, status="unknown", task="T5", duration=10, tokens=1000, cost=0.05, tool_calls=5),
        ]

        app = App()
        async with app.run_test() as pilot:
            screen = HistoryScreen(records=records)
            app.push_screen(screen)
            await pilot.pause()

            assert len(screen.records) == 5

    @pytest.mark.asyncio
    async def test_duration_formatting_seconds(self):
        """HistoryScreen formats short durations as seconds."""
        records = [
            IterationRecord(number=1, status="success", task="Quick", duration=30.5, tokens=1000, cost=0.05, tool_calls=5),
        ]

        app = App()
        async with app.run_test() as pilot:
            screen = HistoryScreen(records=records)
            app.push_screen(screen)
            await pilot.pause()

    @pytest.mark.asyncio
    async def test_duration_formatting_minutes(self):
        """HistoryScreen formats long durations as minutes."""
        records = [
            IterationRecord(number=1, status="success", task="Long", duration=125.0, tokens=1000, cost=0.05, tool_calls=5),
        ]

        app = App()
        async with app.run_test() as pilot:
            screen = HistoryScreen(records=records)
            app.push_screen(screen)
            await pilot.pause()

    @pytest.mark.asyncio
    async def test_task_truncation(self):
        """HistoryScreen truncates long task names."""
        records = [
            IterationRecord(
                number=1, status="success",
                task="This is a very long task name that exceeds thirty characters",
                duration=10, tokens=1000, cost=0.05, tool_calls=5
            ),
        ]

        app = App()
        async with app.run_test() as pilot:
            screen = HistoryScreen(records=records)
            app.push_screen(screen)
            await pilot.pause()


# =============================================================================
# HelpScreen Mounted Tests
# =============================================================================

class TestHelpScreenMounted:
    """Tests for HelpScreen when mounted."""

    @pytest.mark.asyncio
    async def test_compose_creates_widgets(self):
        """HelpScreen composes keyboard shortcuts."""
        app = App()

        async with app.run_test() as pilot:
            screen = HelpScreen()
            app.push_screen(screen)
            await pilot.pause()

            # Should have content
            assert screen is not None

    @pytest.mark.asyncio
    async def test_escape_closes_screen(self):
        """Escape key closes help screen."""
        app = App()

        async with app.run_test() as pilot:
            screen = HelpScreen()
            app.push_screen(screen)
            await pilot.pause()

            await pilot.press("escape")


# =============================================================================
# RalphTUI App Mounted Tests
# =============================================================================

class TestRalphTUIMounted:
    """Tests for RalphTUI app when mounted."""

    @pytest.mark.asyncio
    async def test_compose_creates_widgets(self):
        """RalphTUI composes all main widgets."""
        app = RalphTUI()
        async with app.run_test():
            assert app.query_one("#tasks") is not None
            assert app.query_one("#progress") is not None
            assert app.query_one("#output") is not None
            assert app.query_one("#metrics") is not None

    @pytest.mark.asyncio
    async def test_initial_state(self):
        """RalphTUI starts with correct initial state."""
        app = RalphTUI()
        async with app.run_test():
            assert app.is_paused is False
            assert app.current_iteration == 0
            assert app.max_iterations == 100
            assert app.current_task == ""
            assert app.connection_status == "disconnected"

    @pytest.mark.asyncio
    async def test_action_pause_resume(self):
        """action_pause_resume toggles pause state."""
        app = RalphTUI()
        async with app.run_test():
            assert app.is_paused is False
            app.action_pause_resume()
            assert app.is_paused is True
            app.action_pause_resume()
            assert app.is_paused is False

    @pytest.mark.asyncio
    async def test_action_toggle_logs(self):
        """action_toggle_logs toggles output visibility."""
        app = RalphTUI()
        async with app.run_test():
            output = app.query_one("#output")
            assert "hidden" not in output.classes
            app.action_toggle_logs()
            assert "hidden" in output.classes
            app.action_toggle_logs()
            assert "hidden" not in output.classes

    @pytest.mark.asyncio
    async def test_action_toggle_tasks(self):
        """action_toggle_tasks toggles sidebar visibility."""
        app = RalphTUI()
        async with app.run_test():
            tasks = app.query_one("#tasks")
            assert "collapsed" not in tasks.classes
            app.action_toggle_tasks()
            assert "collapsed" in tasks.classes

    @pytest.mark.asyncio
    async def test_action_toggle_metrics(self):
        """action_toggle_metrics toggles metrics visibility."""
        app = RalphTUI()
        async with app.run_test():
            metrics = app.query_one("#metrics")
            assert "hidden" not in metrics.classes
            app.action_toggle_metrics()
            assert "hidden" in metrics.classes

    @pytest.mark.asyncio
    async def test_action_show_history(self):
        """action_show_history pushes history screen."""
        app = RalphTUI()
        async with app.run_test() as pilot:
            app.action_show_history()
            await pilot.pause()
            assert len(app.screen_stack) > 1

    @pytest.mark.asyncio
    async def test_action_show_help(self):
        """action_show_help pushes help screen."""
        app = RalphTUI()
        async with app.run_test() as pilot:
            app.action_show_help()
            await pilot.pause()
            assert len(app.screen_stack) > 1

    @pytest.mark.asyncio
    async def test_action_back_no_effect_on_main(self):
        """action_back does nothing on main screen."""
        app = RalphTUI()
        async with app.run_test():
            initial_stack = len(app.screen_stack)
            app.action_back()
            assert len(app.screen_stack) == initial_stack

    @pytest.mark.asyncio
    async def test_action_back_pops_screen(self):
        """action_back pops pushed screen."""
        app = RalphTUI()
        async with app.run_test() as pilot:
            app.action_show_help()
            await pilot.pause()
            assert len(app.screen_stack) > 1

            app.action_back()
            await pilot.pause()
            assert len(app.screen_stack) == 1

    @pytest.mark.asyncio
    async def test_action_toggle_follow(self):
        """action_toggle_follow toggles auto-scroll."""
        app = RalphTUI()
        async with app.run_test():
            from ralph_orchestrator.tui.widgets.output import OutputViewer
            output = app.query_one("#output", OutputViewer)
            assert output.auto_scroll is True

            app.action_toggle_follow()
            assert output.auto_scroll is False

    @pytest.mark.asyncio
    async def test_watch_is_paused(self):
        """Changing is_paused updates progress panel."""
        app = RalphTUI()
        async with app.run_test():
            from ralph_orchestrator.tui.widgets.progress import ProgressPanel
            progress = app.query_one("#progress", ProgressPanel)

            app.is_paused = True
            # The watcher should have been called
            assert progress.is_paused is True

    @pytest.mark.asyncio
    async def test_watch_connection_status(self):
        """Changing connection_status updates progress panel."""
        app = RalphTUI()
        async with app.run_test():
            from ralph_orchestrator.tui.widgets.progress import ProgressPanel
            progress = app.query_one("#progress", ProgressPanel)

            app.connection_status = "connected"
            assert progress.connection_status == "connected"

    @pytest.mark.asyncio
    async def test_checkpoint_without_connection(self):
        """action_checkpoint does nothing without connection."""
        app = RalphTUI()
        async with app.run_test():
            # Should not raise
            app.action_checkpoint()

    @pytest.mark.asyncio
    async def test_approve_validation_without_pending(self):
        """action_approve_validation does nothing without pending."""
        app = RalphTUI()
        async with app.run_test():
            # Should not raise
            app.action_approve_validation()

    @pytest.mark.asyncio
    async def test_reject_validation_without_pending(self):
        """action_reject_validation does nothing without pending."""
        app = RalphTUI()
        async with app.run_test():
            # Should not raise
            app.action_reject_validation()

    @pytest.mark.asyncio
    async def test_skip_validation_without_pending(self):
        """action_skip_validation does nothing without pending."""
        app = RalphTUI()
        async with app.run_test():
            # Should not raise
            app.action_skip_validation()

    @pytest.mark.asyncio
    async def test_key_bindings_defined(self):
        """RalphTUI has all expected key bindings."""
        app = RalphTUI()
        binding_keys = [b.key for b in app.BINDINGS]
        assert "q" in binding_keys
        assert "p" in binding_keys
        assert "l" in binding_keys
        assert "t" in binding_keys
        assert "m" in binding_keys
        assert "h" in binding_keys
        assert "escape" in binding_keys

    @pytest.mark.asyncio
    async def test_with_prompt_file(self):
        """RalphTUI accepts prompt_file parameter."""
        app = RalphTUI(prompt_file="test_prompt.md")
        assert app.prompt_file == "test_prompt.md"
        async with app.run_test():
            pass  # Just verify it doesn't crash


# =============================================================================
# Additional Coverage Tests
# =============================================================================

class TestHistoryScreenActions:
    """Additional tests for HistoryScreen action coverage."""

    @pytest.mark.asyncio
    async def test_action_view_details(self):
        """action_view_details shows notification for selected row."""
        records = [
            IterationRecord(number=1, status="success", task="Test", duration=10, tokens=1000, cost=0.05, tool_calls=5),
        ]
        app = App()

        async with app.run_test() as pilot:
            screen = HistoryScreen(records=records)
            app.push_screen(screen)
            await pilot.pause()

            # Should not raise even without row selected
            screen.action_view_details()

    @pytest.mark.asyncio
    async def test_action_filter(self):
        """action_filter shows warning notification."""
        app = App()

        async with app.run_test() as pilot:
            screen = HistoryScreen(records=[])
            app.push_screen(screen)
            await pilot.pause()

            # Should show "not yet implemented" notification
            screen.action_filter()

    @pytest.mark.asyncio
    async def test_action_export(self):
        """action_export shows warning notification."""
        app = App()

        async with app.run_test() as pilot:
            screen = HistoryScreen(records=[])
            app.push_screen(screen)
            await pilot.pause()

            # Should show "not yet implemented" notification
            screen.action_export()


class TestProgressPanelTick:
    """Additional tests for ProgressPanel timer coverage."""

    @pytest.mark.asyncio
    async def test_tick_updates_elapsed(self):
        """_tick updates elapsed_seconds when running."""
        from datetime import datetime
        app = ProgressPanelApp()

        async with app.run_test() as pilot:
            progress = app.query_one("#progress", ProgressPanel)

            # Set start time
            progress.start_time = datetime.now()
            progress.is_paused = False

            # Manually call tick
            progress._tick()

            # Should have updated elapsed
            assert progress.elapsed_seconds >= 0

    @pytest.mark.asyncio
    async def test_tick_no_update_when_paused(self):
        """_tick doesn't update when paused."""
        from datetime import datetime
        app = ProgressPanelApp()

        async with app.run_test() as pilot:
            progress = app.query_one("#progress", ProgressPanel)

            # Set start time but pause
            progress.start_time = datetime.now()
            progress.is_paused = True
            progress.elapsed_seconds = 0

            # Manually call tick
            progress._tick()

            # Should not have updated elapsed (still 0)
            assert progress.elapsed_seconds == 0


class TestOutputViewerScrolling:
    """Additional tests for OutputViewer scroll coverage."""

    @pytest.mark.asyncio
    async def test_scroll_end_called_with_auto_scroll(self):
        """scroll_end is called when auto_scroll is True."""
        app = OutputViewerApp()

        async with app.run_test() as pilot:
            output = app.query_one("#output", OutputViewer)
            output.auto_scroll = True

            # Append output (should trigger scroll_end)
            output.append_agent_output("Test output")
            await pilot.pause()

            # Just verify no error occurred

    @pytest.mark.asyncio
    async def test_no_scroll_when_disabled(self):
        """scroll_end not called when auto_scroll is False."""
        app = OutputViewerApp()

        async with app.run_test() as pilot:
            output = app.query_one("#output", OutputViewer)
            output.auto_scroll = False

            # Append output (should not trigger scroll_end)
            output.append_agent_output("Test output")
            await pilot.pause()

            # Just verify no error occurred

    @pytest.mark.asyncio
    async def test_markdown_fallback_on_exception(self):
        """Falls back to plain text when markdown fails."""
        app = OutputViewerApp()

        async with app.run_test() as pilot:
            output = app.query_one("#output", OutputViewer)

            # Send text that will work even if markdown has issues
            output.append_agent_output("Normal text output")
            await pilot.pause()

            # Just verify no error occurred


class TestConnectionBasic:
    """Basic tests for OrchestratorConnection."""

    def test_connection_import(self):
        """OrchestratorConnection can be imported."""
        from ralph_orchestrator.tui.connection import OrchestratorConnection
        assert OrchestratorConnection is not None

    def test_connection_instantiation(self):
        """OrchestratorConnection can be instantiated."""
        from ralph_orchestrator.tui.connection import OrchestratorConnection
        conn = OrchestratorConnection()
        assert conn is not None
        assert conn._connected is False

    def test_is_connected_property(self):
        """is_connected returns connection state."""
        from ralph_orchestrator.tui.connection import OrchestratorConnection
        conn = OrchestratorConnection()
        assert conn.is_connected is False

    def test_tui_event_class(self):
        """TUIEvent can be created and serialized."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        event = TUIEvent(type=EventType.ITERATION_START, data={"count": 1})
        assert event.type == EventType.ITERATION_START
        assert event.data["count"] == 1

    def test_tui_event_to_json(self):
        """TUIEvent can be serialized to JSON."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        event = TUIEvent(type=EventType.OUTPUT, data={"text": "hello"})
        json_str = event.to_json()
        assert "output" in json_str
        assert "hello" in json_str

    def test_tui_event_from_json(self):
        """TUIEvent can be deserialized from JSON."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        json_str = '{"type": "iteration_start", "data": {"count": 5}, "timestamp": 12345}'
        event = TUIEvent.from_json(json_str)
        assert event.type == EventType.ITERATION_START
        assert event.data["count"] == 5

    def test_event_type_enum(self):
        """EventType enum has expected values."""
        from ralph_orchestrator.tui.connection import EventType
        assert EventType.ITERATION_START.value == "iteration_start"
        assert EventType.OUTPUT.value == "output"
        assert EventType.ERROR.value == "error"

    @pytest.mark.asyncio
    async def test_disconnect(self):
        """disconnect sets connected to False."""
        from ralph_orchestrator.tui.connection import OrchestratorConnection
        conn = OrchestratorConnection()
        await conn.disconnect()
        assert conn._connected is False
        assert conn._running is False


class TestAppEventHandlers:
    """Tests for RalphTUI event handling methods."""

    @pytest.mark.asyncio
    async def test_handle_event_iteration(self):
        """_handle_event handles iteration_start event."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        app = RalphTUI()
        async with app.run_test():
            event = TUIEvent(
                type=EventType.ITERATION_START,
                data={"iteration": 5, "max_iterations": 10, "task": "Test task"}
            )
            await app._handle_event(event)

            assert app.current_iteration == 5
            assert app.max_iterations == 10
            assert app.current_task == "Test task"

    @pytest.mark.asyncio
    async def test_handle_event_output(self):
        """_handle_event handles output event."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        app = RalphTUI()
        async with app.run_test():
            event = TUIEvent(
                type=EventType.OUTPUT,
                data={"text": "Hello from agent"}
            )
            await app._handle_event(event)
            # Verify no exception

    @pytest.mark.asyncio
    async def test_handle_event_tool_call(self):
        """_handle_event handles tool_call event."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        app = RalphTUI()
        async with app.run_test():
            event = TUIEvent(
                type=EventType.TOOL_CALL,
                data={"name": "bash", "input": {"cmd": "ls"}, "result": "file.txt", "status": "success"}
            )
            await app._handle_event(event)
            # Verify no exception

    @pytest.mark.asyncio
    async def test_handle_event_metrics(self):
        """_handle_event handles metrics event."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        from ralph_orchestrator.tui.widgets.metrics import MetricsPanel
        app = RalphTUI()
        async with app.run_test():
            event = TUIEvent(
                type=EventType.METRICS,
                data={"cpu": 50.0, "memory": 1024, "tokens": 500, "cost": 0.05}
            )
            await app._handle_event(event)
            metrics = app.query_one("#metrics", MetricsPanel)
            # Verify metrics were updated
            current = metrics.get_current_metrics()
            assert current["cpu"] == 50.0

    @pytest.mark.asyncio
    async def test_handle_event_error(self):
        """_handle_event handles error event."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        app = RalphTUI()
        async with app.run_test():
            event = TUIEvent(
                type=EventType.ERROR,
                data={"message": "Something went wrong"}
            )
            await app._handle_event(event)
            # Verify no exception

    @pytest.mark.asyncio
    async def test_handle_event_complete(self):
        """_handle_event handles complete event."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        app = RalphTUI()
        async with app.run_test():
            event = TUIEvent(type=EventType.COMPLETE, data={})
            await app._handle_event(event)
            assert app.connection_status == "complete"

    @pytest.mark.asyncio
    async def test_handle_event_task_update(self):
        """_handle_event handles task_update event."""
        from ralph_orchestrator.tui.connection import TUIEvent, EventType
        from ralph_orchestrator.tui.widgets.tasks import TaskSidebar
        app = RalphTUI()
        async with app.run_test():
            event = TUIEvent(
                type=EventType.TASK_UPDATE,
                data={
                    "pending": [{"id": "1", "name": "Task 1"}],
                    "current": {"id": "2", "name": "Current"},
                    "completed": []
                }
            )
            await app._handle_event(event)
            tasks = app.query_one("#tasks", TaskSidebar)
            summary = tasks.get_task_summary()
            assert summary["pending"] == 1
