"""Main TUI application using Textual framework."""

from textual.app import App, ComposeResult
from textual.containers import Container, Horizontal, Vertical, ScrollableContainer
from textual.widgets import Header, Footer, Static, RichLog, ProgressBar, DataTable
from textual.binding import Binding
from textual.reactive import reactive
from textual import on
from rich.syntax import Syntax
from rich.panel import Panel
from rich.text import Text
from typing import Optional
import asyncio

from .widgets import ProgressPanel, OutputViewer, TaskSidebar, MetricsPanel, ValidationPrompt
from .connection import OrchestratorConnection, TUIEvent


class RalphTUI(App):
    """Real-time Terminal UI for RALPH Orchestrator.

    Provides live visibility into orchestration with:
    - Real-time output streaming
    - Task queue management
    - Metrics visualization
    - Validation gate controls
    - History browser
    """

    CSS_PATH = "ralph.tcss"
    TITLE = "RALPH Orchestrator"
    SUB_TITLE = "Real-time Monitoring"

    # Reactive state
    is_paused: reactive[bool] = reactive(False)
    current_iteration: reactive[int] = reactive(0)
    max_iterations: reactive[int] = reactive(100)
    current_task: reactive[str] = reactive("")
    connection_status: reactive[str] = reactive("disconnected")

    BINDINGS = [
        Binding("q", "quit", "Quit", show=True),
        Binding("p", "pause_resume", "Pause", show=True),
        Binding("l", "toggle_logs", "Logs", show=True),
        Binding("t", "toggle_tasks", "Tasks", show=True),
        Binding("m", "toggle_metrics", "Metrics", show=True),
        Binding("h", "show_history", "History", show=True),
        Binding("d", "toggle_details", "Details", show=False),
        Binding("c", "checkpoint", "Checkpoint", show=False),
        Binding("y", "approve_validation", "Approve", show=False),
        Binding("n", "reject_validation", "Reject", show=False),
        Binding("s", "skip_validation", "Skip", show=False),
        Binding("f", "toggle_follow", "Follow", show=False),
        Binding("slash", "search", "Search", show=False),
        Binding("question_mark", "show_help", "Help", show=True),
        Binding("escape", "back", "Back", show=False),
    ]

    def __init__(
        self,
        connection: Optional[OrchestratorConnection] = None,
        prompt_file: Optional[str] = None,
    ):
        super().__init__()
        self.connection = connection
        self.prompt_file = prompt_file
        self._event_task: Optional[asyncio.Task] = None
        self._pending_validation: Optional[dict] = None

    def compose(self) -> ComposeResult:
        """Compose the TUI layout."""
        yield Header(show_clock=True)

        with Horizontal(id="main-container"):
            yield TaskSidebar(id="tasks")

            with Vertical(id="content"):
                yield ProgressPanel(id="progress")
                yield OutputViewer(id="output")
                yield MetricsPanel(id="metrics")

        yield Footer()

    async def on_mount(self) -> None:
        """Initialize when app mounts."""
        self.title = f"RALPH - {self.prompt_file or 'Watching'}"

        if self.connection:
            self._event_task = asyncio.create_task(self._process_events())
            self.connection_status = "connecting"

    async def _process_events(self) -> None:
        """Process events from orchestrator connection."""
        if not self.connection:
            return

        try:
            connected = await self.connection.connect()
            if connected:
                self.connection_status = "connected"
                self.notify("Connected to orchestrator", severity="information")
            else:
                self.connection_status = "failed"
                self.notify("Connection failed", severity="error")
                return

            async for event in self.connection.events():
                await self._handle_event(event)

        except asyncio.CancelledError:
            pass
        except Exception as e:
            self.connection_status = "error"
            self.notify(f"Connection error: {e}", severity="error")

    async def _handle_event(self, event: TUIEvent) -> None:
        """Handle incoming orchestrator event."""
        output = self.query_one("#output", OutputViewer)
        progress = self.query_one("#progress", ProgressPanel)
        tasks = self.query_one("#tasks", TaskSidebar)
        metrics = self.query_one("#metrics", MetricsPanel)

        match event.type:
            case "iteration_start":
                self.current_iteration = event.data.get("iteration", 0)
                self.max_iterations = event.data.get("max_iterations", 100)
                self.current_task = event.data.get("task", "")
                progress.update_iteration(self.current_iteration, self.max_iterations)
                output.append_iteration_marker(self.current_iteration)

            case "output":
                output.append_agent_output(event.data.get("text", ""))

            case "tool_call":
                output.append_tool_call(
                    event.data.get("name", "unknown"),
                    event.data.get("input", {}),
                    event.data.get("result", ""),
                    event.data.get("status", "success"),
                )

            case "task_update":
                tasks.update_tasks(
                    event.data.get("pending", []),
                    event.data.get("current"),
                    event.data.get("completed", []),
                )

            case "metrics":
                metrics.update_metrics(
                    cpu=event.data.get("cpu", 0),
                    memory=event.data.get("memory", 0),
                    tokens=event.data.get("tokens", 0),
                    cost=event.data.get("cost", 0.0),
                )

            case "validation_gate":
                self._pending_validation = event.data
                await self._show_validation_prompt(event.data)

            case "error":
                output.append_error(event.data.get("message", "Unknown error"))
                self.notify(event.data.get("message", "Error"), severity="error")

            case "complete":
                progress.mark_complete()
                self.notify("Orchestration complete!", severity="information")
                self.connection_status = "complete"

    async def _show_validation_prompt(self, data: dict) -> None:
        """Show validation gate prompt."""
        prompt = ValidationPrompt(
            gate_name=data.get("name", "Validation Gate"),
            description=data.get("description", ""),
            evidence=data.get("evidence", []),
        )
        await self.push_screen(prompt)

    def action_pause_resume(self) -> None:
        """Toggle pause state."""
        self.is_paused = not self.is_paused
        if self.connection:
            asyncio.create_task(
                self.connection.send_command("pause" if self.is_paused else "resume")
            )
        status = "Paused" if self.is_paused else "Resumed"
        self.notify(status, severity="warning" if self.is_paused else "information")

    def action_toggle_logs(self) -> None:
        """Toggle log panel visibility."""
        output = self.query_one("#output")
        output.toggle_class("hidden")

    def action_toggle_tasks(self) -> None:
        """Toggle task sidebar visibility."""
        tasks = self.query_one("#tasks")
        tasks.toggle_class("collapsed")

    def action_toggle_metrics(self) -> None:
        """Toggle metrics panel visibility."""
        metrics = self.query_one("#metrics")
        metrics.toggle_class("hidden")

    def action_checkpoint(self) -> None:
        """Force a checkpoint."""
        if self.connection:
            asyncio.create_task(self.connection.send_command("checkpoint"))
            self.notify("Checkpoint created", severity="information")

    def action_approve_validation(self) -> None:
        """Approve current validation gate."""
        if self._pending_validation and self.connection:
            asyncio.create_task(
                self.connection.send_command("validation_response", approved=True)
            )
            self._pending_validation = None
            self.pop_screen()

    def action_reject_validation(self) -> None:
        """Reject current validation gate."""
        if self._pending_validation and self.connection:
            asyncio.create_task(
                self.connection.send_command("validation_response", approved=False)
            )
            self._pending_validation = None
            self.pop_screen()

    def action_skip_validation(self) -> None:
        """Skip current validation gate."""
        if self._pending_validation and self.connection:
            asyncio.create_task(
                self.connection.send_command("validation_response", skipped=True)
            )
            self._pending_validation = None
            self.pop_screen()

    def action_toggle_follow(self) -> None:
        """Toggle auto-scroll in output."""
        output = self.query_one("#output", OutputViewer)
        output.toggle_auto_scroll()

    def action_show_history(self) -> None:
        """Show iteration history browser."""
        from .screens import HistoryScreen
        self.push_screen(HistoryScreen())

    def action_show_help(self) -> None:
        """Show help overlay."""
        from .screens import HelpScreen
        self.push_screen(HelpScreen())

    def action_back(self) -> None:
        """Go back from current screen."""
        if len(self.screen_stack) > 1:
            self.pop_screen()

    def watch_is_paused(self, paused: bool) -> None:
        """React to pause state changes."""
        progress = self.query_one("#progress", ProgressPanel)
        progress.set_paused(paused)

        # Update binding label
        for binding in self.BINDINGS:
            if binding.key == "p":
                # Note: In Textual, binding.description is readonly after init
                # This would need a different approach in practice
                break

    def watch_connection_status(self, status: str) -> None:
        """React to connection status changes."""
        progress = self.query_one("#progress", ProgressPanel)
        progress.set_connection_status(status)
