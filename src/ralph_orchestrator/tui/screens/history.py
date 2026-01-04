"""History screen showing past iterations."""

from textual.screen import Screen
from textual.widgets import DataTable, Static, Header, Footer
from textual.containers import Vertical
from textual.binding import Binding
from rich.text import Text
from dataclasses import dataclass
from typing import List, Optional
from datetime import datetime


@dataclass
class IterationRecord:
    """Record of a single iteration."""
    number: int
    status: str  # success, error, validation_failed, checkpoint
    task: str
    duration: float
    tokens: int
    cost: float
    tool_calls: int
    timestamp: float = 0.0


class HistoryScreen(Screen):
    """Screen showing iteration history with details.

    Displays a table of past iterations with:
    - Iteration number
    - Status (success/error/checkpoint)
    - Duration
    - Task summary
    - Tokens used
    - Cost
    """

    DEFAULT_CSS = """
    HistoryScreen {
        align: center middle;
    }

    HistoryScreen Vertical {
        width: 100%;
        height: 100%;
    }

    HistoryScreen DataTable {
        height: 1fr;
        width: 100%;
    }

    HistoryScreen .title {
        text-style: bold;
        color: $primary;
        text-align: center;
        padding: 1;
    }

    HistoryScreen .summary {
        background: $surface-darken-1;
        padding: 1;
        text-align: center;
        color: $text-muted;
    }
    """

    BINDINGS = [
        Binding("escape", "back", "Back"),
        Binding("q", "back", "Back"),
        Binding("enter", "view_details", "View Details"),
        Binding("f", "filter", "Filter"),
        Binding("e", "export", "Export"),
    ]

    def __init__(self, records: List[IterationRecord] = None, **kwargs):
        super().__init__(**kwargs)
        self.records = records or []

    def compose(self):
        """Compose history screen layout."""
        yield Header()
        with Vertical():
            yield Static("📜 Iteration History", classes="title")

            # Summary line
            total = len(self.records)
            success = sum(1 for r in self.records if r.status == "success")
            errors = sum(1 for r in self.records if r.status == "error")
            total_cost = sum(r.cost for r in self.records)
            yield Static(
                f"Total: {total} | Success: {success} | Errors: {errors} | Cost: ${total_cost:.2f}",
                classes="summary"
            )

            # Data table
            table = DataTable(id="history-table")
            table.add_columns(
                "#", "Status", "Duration", "Task", "Cost", "Tokens", "Tools"
            )
            yield table

        yield Footer()

    def on_mount(self) -> None:
        """Populate table with records on mount."""
        self._populate_table()

    def _populate_table(self) -> None:
        """Fill table with iteration records."""
        table = self.query_one("#history-table", DataTable)
        table.clear()

        for record in self.records:
            # Status with icon
            match record.status:
                case "success":
                    status = Text("✓", style="green")
                case "error":
                    status = Text("✗", style="red")
                case "validation_failed":
                    status = Text("⚠", style="yellow")
                case "checkpoint":
                    status = Text("📌", style="blue")
                case _:
                    status = Text("?", style="dim")

            # Duration formatting
            if record.duration < 60:
                duration = f"{record.duration:.1f}s"
            else:
                mins = int(record.duration // 60)
                secs = int(record.duration % 60)
                duration = f"{mins}m {secs}s"

            # Task name (truncated)
            task = record.task[:30] + "..." if len(record.task) > 30 else record.task

            table.add_row(
                str(record.number),
                status,
                duration,
                task,
                f"${record.cost:.2f}",
                f"{record.tokens:,}",
                str(record.tool_calls),
            )

    def update_records(self, records: List[IterationRecord]) -> None:
        """Update the records and refresh table."""
        self.records = records
        self._populate_table()

    def action_back(self) -> None:
        """Return to main screen."""
        self.app.pop_screen()

    def action_view_details(self) -> None:
        """View details of selected iteration."""
        table = self.query_one("#history-table", DataTable)
        if table.cursor_row is not None:
            # Get iteration number from first column
            row_key = table.get_row_at(table.cursor_row)
            # TODO: Show detail modal
            self.notify(f"Details for iteration {row_key[0]}")

    def action_filter(self) -> None:
        """Show filter options."""
        self.notify("Filter not yet implemented", severity="warning")

    def action_export(self) -> None:
        """Export history to file."""
        self.notify("Export not yet implemented", severity="warning")
