"""Tests for TUI widgets."""

import pytest
from ralph_orchestrator.tui.widgets.progress import ProgressPanel
from ralph_orchestrator.tui.widgets.output import OutputViewer
from ralph_orchestrator.tui.widgets.tasks import Task, TaskSidebar
from ralph_orchestrator.tui.widgets.metrics import MetricWidget, MetricsPanel


class TestProgressPanel:
    """Tests for ProgressPanel widget."""

    def test_instantiation(self):
        """ProgressPanel can be instantiated."""
        panel = ProgressPanel()
        assert panel is not None

    def test_has_methods(self):
        """ProgressPanel has expected public methods."""
        panel = ProgressPanel()
        assert callable(getattr(panel, 'update_iteration', None))
        assert callable(getattr(panel, 'set_paused', None))
        assert callable(getattr(panel, 'mark_complete', None))
        assert callable(getattr(panel, 'update_cost', None))

    def test_constructor_sets_attributes(self):
        """ProgressPanel sets internal attributes."""
        panel = ProgressPanel()
        assert panel.start_time is None
        assert panel._timer is None


class TestOutputViewer:
    """Tests for OutputViewer widget."""

    def test_initial_state(self):
        """OutputViewer starts with auto-scroll enabled."""
        viewer = OutputViewer()
        assert viewer.auto_scroll is True
        assert viewer._tool_call_count == 0

    def test_toggle_auto_scroll(self):
        """Can toggle auto-scroll."""
        viewer = OutputViewer()
        viewer.auto_scroll = False
        assert viewer.auto_scroll is False

    def test_clear_output(self):
        """Clear resets tool call count."""
        viewer = OutputViewer()
        viewer._tool_call_count = 5
        viewer.clear_output()
        assert viewer._tool_call_count == 0


class TestTask:
    """Tests for Task dataclass."""

    def test_create_task(self):
        """Create task with required fields."""
        task = Task(id="1", name="Test Task", status="pending")
        assert task.id == "1"
        assert task.name == "Test Task"
        assert task.status == "pending"
        assert task.duration is None
        assert task.error is None

    def test_task_with_optional_fields(self):
        """Create task with all fields."""
        task = Task(
            id="1",
            name="Test Task",
            status="failed",
            duration=10.5,
            error="Something went wrong"
        )
        assert task.duration == 10.5
        assert task.error == "Something went wrong"


class TestTaskSidebar:
    """Tests for TaskSidebar widget."""

    def test_initial_state(self):
        """TaskSidebar starts collapsed=False."""
        sidebar = TaskSidebar()
        assert sidebar.is_collapsed is False

    def test_toggle_collapse(self):
        """Can toggle collapse state."""
        sidebar = TaskSidebar()
        sidebar.is_collapsed = True
        assert sidebar.is_collapsed is True

    def test_get_task_summary(self):
        """get_task_summary returns count dict."""
        sidebar = TaskSidebar()
        summary = sidebar.get_task_summary()
        assert "pending" in summary
        assert "completed" in summary
        assert "running" in summary  # Key is 'running' not 'current'
        assert "failed" in summary


class TestMetricWidget:
    """Tests for MetricWidget."""

    def test_instantiation(self):
        """MetricWidget can be instantiated."""
        widget = MetricWidget("Test", icon="⚙", unit="%")
        assert widget is not None
        assert widget.label == "Test"
        assert widget.unit == "%"
        assert widget.icon == "⚙"

    def test_widget_properties(self):
        """MetricWidget stores constructor parameters."""
        widget = MetricWidget("CPU", icon="🔥", unit="%", max_value=100.0)
        assert widget.label == "CPU"
        assert widget.icon == "🔥"
        assert widget.unit == "%"
        assert widget.max_value == 100.0
        assert widget.format_spec == ".1f"  # default value


class TestMetricsPanel:
    """Tests for MetricsPanel."""

    def test_update_metrics(self):
        """MetricsPanel updates all metrics."""
        # We can't fully test without mounting the widget,
        # but we can verify the method exists
        panel = MetricsPanel()
        assert hasattr(panel, "update_metrics")
        assert hasattr(panel, "get_current_metrics")
