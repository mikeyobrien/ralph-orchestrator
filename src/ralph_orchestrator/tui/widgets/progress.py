"""Progress panel widget showing orchestration status."""

from textual.widgets import Static, ProgressBar
from textual.containers import Horizontal, Vertical
from textual.reactive import reactive
from textual import on
from rich.text import Text
from rich.panel import Panel
from rich.table import Table
from datetime import datetime, timedelta
from typing import Optional


class ProgressPanel(Static):
    """Real-time progress display with iteration tracking.

    Shows:
    - Status badge (Running/Paused/Complete/Error)
    - Current task description
    - Progress bar with percentage
    - Timing info (elapsed, ETA)
    - Cost tracker
    - Connection status
    """

    DEFAULT_CSS = """
    ProgressPanel {
        height: 8;
        padding: 1;
        background: $surface-darken-1;
        border-bottom: solid $primary-darken-2;
    }

    ProgressPanel .status-running {
        color: $success;
        text-style: bold;
    }

    ProgressPanel .status-paused {
        color: $warning;
        text-style: italic;
    }

    ProgressPanel .status-complete {
        color: $primary;
        text-style: bold;
    }

    ProgressPanel .status-error {
        color: $error;
        text-style: bold;
    }

    ProgressPanel #progress-bar {
        width: 100%;
        margin: 1 0;
    }

    ProgressPanel #stats-row {
        height: 1;
    }
    """

    # Reactive properties
    current_iteration: reactive[int] = reactive(0)
    max_iterations: reactive[int] = reactive(100)
    current_task: reactive[str] = reactive("Initializing...")
    status: reactive[str] = reactive("idle")
    is_paused: reactive[bool] = reactive(False)
    elapsed_seconds: reactive[float] = reactive(0.0)
    cost: reactive[float] = reactive(0.0)
    max_cost: reactive[float] = reactive(50.0)
    connection_status: reactive[str] = reactive("disconnected")

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.start_time: Optional[datetime] = None
        self._timer = None

    def compose(self):
        """Compose the progress panel layout."""
        yield Static(id="status-line")
        yield Static(id="task-line")
        yield ProgressBar(id="progress-bar", total=100, show_eta=False)
        with Horizontal(id="stats-row"):
            yield Static(id="timing-stat")
            yield Static(id="cost-stat")
            yield Static(id="connection-stat")

    def on_mount(self) -> None:
        """Start the timer when mounted."""
        self.start_time = datetime.now()
        self._timer = self.set_interval(1.0, self._tick)
        self._update_display()

    def _tick(self) -> None:
        """Update elapsed time every second."""
        if self.start_time and not self.is_paused:
            self.elapsed_seconds = (datetime.now() - self.start_time).total_seconds()

    def update_iteration(self, current: int, max_iter: int) -> None:
        """Update iteration progress."""
        self.current_iteration = current
        self.max_iterations = max_iter
        self.status = "running"
        self._update_display()

    def set_paused(self, paused: bool) -> None:
        """Set paused state."""
        self.is_paused = paused
        self._update_display()

    def set_connection_status(self, status: str) -> None:
        """Update connection status indicator."""
        self.connection_status = status
        self._update_display()

    def update_cost(self, cost: float) -> None:
        """Update cost tracker."""
        self.cost = cost
        self._update_display()

    def mark_complete(self) -> None:
        """Mark orchestration as complete."""
        self.status = "complete"
        if self._timer:
            self._timer.stop()
        self._update_display()

    def _update_display(self) -> None:
        """Refresh all display elements."""
        # Status line
        status_text = self._get_status_text()
        self.query_one("#status-line", Static).update(status_text)

        # Task line
        task_text = Text()
        task_text.append("  ", style="bold cyan")
        task_text.append(self.current_task[:60] or "Waiting for task...")
        self.query_one("#task-line", Static).update(task_text)

        # Progress bar
        progress = self.query_one("#progress-bar", ProgressBar)
        if self.max_iterations > 0:
            pct = (self.current_iteration / self.max_iterations) * 100
            progress.update(progress=pct)

        # Timing stats
        timing = self._format_timing()
        self.query_one("#timing-stat", Static).update(timing)

        # Cost stats
        cost_text = Text()
        cost_text.append(" $", style="bold")
        cost_text.append(f"{self.cost:.2f}", style="green" if self.cost < self.max_cost * 0.8 else "yellow")
        cost_text.append(f" / ${self.max_cost:.2f}")
        self.query_one("#cost-stat", Static).update(cost_text)

        # Connection status
        conn_text = self._get_connection_text()
        self.query_one("#connection-stat", Static).update(conn_text)

    def _get_status_text(self) -> Text:
        """Generate status badge text."""
        text = Text()

        if self.status == "complete":
            text.append(" COMPLETE ", style="bold white on green")
        elif self.is_paused:
            text.append(" PAUSED ", style="bold black on yellow")
        elif self.status == "error":
            text.append(" ERROR ", style="bold white on red")
        elif self.status == "running":
            text.append(" RUNNING ", style="bold white on blue")
        else:
            text.append(" IDLE ", style="bold white on grey50")

        text.append(f"  Iteration {self.current_iteration}/{self.max_iterations}")

        if self.max_iterations > 0:
            pct = (self.current_iteration / self.max_iterations) * 100
            text.append(f" ({pct:.0f}%)", style="dim")

        return text

    def _format_timing(self) -> Text:
        """Format timing information."""
        text = Text()
        text.append(" ", style="bold")

        # Elapsed time
        elapsed = timedelta(seconds=int(self.elapsed_seconds))
        text.append(f"{elapsed}", style="cyan")

        # ETA calculation
        if self.current_iteration > 0 and self.max_iterations > self.current_iteration:
            avg_per_iter = self.elapsed_seconds / self.current_iteration
            remaining = (self.max_iterations - self.current_iteration) * avg_per_iter
            eta = timedelta(seconds=int(remaining))
            text.append(f" | ETA: {eta}", style="dim")

        return text

    def _get_connection_text(self) -> Text:
        """Generate connection status indicator."""
        text = Text()

        match self.connection_status:
            case "connected":
                text.append(" ", style="green")
                text.append("Connected", style="green dim")
            case "connecting":
                text.append(" ", style="yellow")
                text.append("Connecting...", style="yellow dim")
            case "disconnected":
                text.append(" ", style="grey50")
                text.append("Disconnected", style="grey50")
            case "error":
                text.append(" ", style="red")
                text.append("Error", style="red dim")
            case "complete":
                text.append(" ", style="blue")
                text.append("Done", style="blue dim")

        return text

    def watch_current_iteration(self, value: int) -> None:
        """React to iteration changes."""
        self._update_display()

    def watch_is_paused(self, value: bool) -> None:
        """React to pause state changes."""
        self._update_display()

    def watch_elapsed_seconds(self, value: float) -> None:
        """React to time updates."""
        self._update_display()
