"""Task sidebar widget for queue management."""

from textual.widgets import Static, ListView, ListItem, Collapsible
from textual.containers import Vertical, ScrollableContainer
from textual.reactive import reactive
from rich.text import Text
from rich.panel import Panel
from typing import List, Optional
from dataclasses import dataclass


@dataclass
class Task:
    """Represents a task in the queue."""
    id: str
    name: str
    status: str  # pending, running, completed, failed, skipped
    duration: Optional[float] = None
    error: Optional[str] = None


class TaskItem(ListItem):
    """Single task item in the list."""

    def __init__(self, task: Task, **kwargs):
        super().__init__(**kwargs)
        self._task_data = task

    def compose(self):
        """Compose task item display."""
        yield Static(self._render_task())

    def _render_task(self) -> Text:
        """Render task as Rich Text."""
        text = Text()

        # Status icon
        match self._task_data.status:
            case "pending":
                text.append("○", style="grey50")
            case "running":
                text.append("●", style="cyan bold")
            case "completed":
                text.append("✓", style="green")
            case "failed":
                text.append("✗", style="red")
            case "skipped":
                text.append("⊘", style="yellow")

        text.append(f" {self._task_data.name[:25]}", style="bold" if self._task_data.status == "running" else "")

        if self._task_data.duration:
            text.append(f" ({self._task_data.duration:.1f}s)", style="dim")

        return text


class TaskSidebar(Static):
    """Collapsible sidebar showing task queue.

    Displays:
    - Pending tasks (queued)
    - Current task (highlighted)
    - Completed tasks (collapsible)
    - Failed tasks (highlighted in red)
    """

    DEFAULT_CSS = """
    TaskSidebar {
        width: 32;
        background: $surface-darken-2;
        border-right: solid $primary-darken-3;
        padding: 1;
    }

    TaskSidebar.collapsed {
        width: 3;
    }

    TaskSidebar.collapsed .sidebar-content {
        display: none;
    }

    TaskSidebar .sidebar-title {
        text-style: bold;
        color: $primary;
        padding-bottom: 1;
    }

    TaskSidebar .section-title {
        text-style: bold;
        color: $text-muted;
        padding: 1 0 0 0;
    }

    TaskSidebar ListView {
        height: auto;
        max-height: 15;
        background: transparent;
    }

    TaskSidebar ListItem {
        padding: 0 1;
    }

    TaskSidebar ListItem:hover {
        background: $surface-darken-1;
    }

    TaskSidebar .current-task {
        background: $primary-darken-3;
        border: solid $primary;
        padding: 1;
        margin: 1 0;
    }
    """

    is_collapsed: reactive[bool] = reactive(False)

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._pending: List[Task] = []
        self._current: Optional[Task] = None
        self._completed: List[Task] = []

    def compose(self):
        """Compose sidebar layout."""
        with Vertical(classes="sidebar-content"):
            yield Static("📋 Tasks", classes="sidebar-title")

            yield Static("Pending", classes="section-title")
            yield ListView(id="pending-list")

            yield Static("Current", classes="section-title")
            yield Static(id="current-task", classes="current-task")

            with Collapsible(title="Completed", collapsed=True, id="completed-section"):
                yield ListView(id="completed-list")

    def update_tasks(
        self,
        pending: List[dict],
        current: Optional[dict],
        completed: List[dict]
    ) -> None:
        """Update task lists from orchestrator data."""
        # Convert dicts to Task objects
        self._pending = [
            Task(id=t.get("id", ""), name=t.get("name", "Unknown"), status="pending")
            for t in pending
        ]

        if current:
            self._current = Task(
                id=current.get("id", ""),
                name=current.get("name", "Unknown"),
                status="running",
                duration=current.get("duration"),
            )
        else:
            self._current = None

        self._completed = [
            Task(
                id=t.get("id", ""),
                name=t.get("name", "Unknown"),
                status=t.get("status", "completed"),
                duration=t.get("duration"),
                error=t.get("error"),
            )
            for t in completed
        ]

        self._refresh_display()

    def _refresh_display(self) -> None:
        """Refresh the sidebar display."""
        # Update pending list
        pending_list = self.query_one("#pending-list", ListView)
        pending_list.clear()
        for task in self._pending[:10]:  # Limit display
            pending_list.append(TaskItem(task))

        if len(self._pending) > 10:
            pending_list.append(ListItem(Static(f"... and {len(self._pending) - 10} more")))

        # Update current task
        current_widget = self.query_one("#current-task", Static)
        if self._current:
            text = Text()
            text.append("▶ ", style="cyan bold")
            text.append(f" {self._current.name}", style="bold")
            if self._current.duration:
                text.append(f"\n   {self._current.duration:.1f}s", style="dim")
            current_widget.update(text)
        else:
            current_widget.update(Text("No active task", style="dim"))

        # Update completed list
        completed_list = self.query_one("#completed-list", ListView)
        completed_list.clear()
        for task in reversed(self._completed[-20:]):  # Last 20, newest first
            completed_list.append(TaskItem(task))

        # Update completed count in collapsible title
        completed_section = self.query_one("#completed-section", Collapsible)
        completed_section.title = f"Completed ({len(self._completed)})"

    def toggle_collapse(self) -> None:
        """Toggle sidebar collapse state."""
        self.is_collapsed = not self.is_collapsed
        self.toggle_class("collapsed")

    def get_task_summary(self) -> dict:
        """Get summary of task states."""
        return {
            "pending": len(self._pending),
            "running": 1 if self._current else 0,
            "completed": len([t for t in self._completed if t.status == "completed"]),
            "failed": len([t for t in self._completed if t.status == "failed"]),
        }
