"""Output viewer widget for streaming agent output."""

from textual.widgets import RichLog, Static, Collapsible
from textual.containers import Vertical
from textual.reactive import reactive
from rich.syntax import Syntax
from rich.panel import Panel
from rich.text import Text
from rich.markdown import Markdown
from rich.console import Group
from typing import Optional
import json


class OutputViewer(RichLog):
    """Syntax-highlighted, scrollable output display.

    Features:
    - Real-time streaming of agent output
    - Syntax-highlighted code blocks
    - Collapsible tool call cards
    - Error highlighting
    - Auto-scroll with toggle
    - Search capability
    """

    DEFAULT_CSS = """
    OutputViewer {
        height: 1fr;
        background: $surface;
        padding: 1;
        border: solid $primary-darken-3;
    }

    OutputViewer.hidden {
        display: none;
    }

    OutputViewer .tool-call {
        background: $surface-darken-2;
        border: solid $accent;
        margin: 1 0;
        padding: 1;
    }

    OutputViewer .tool-success {
        border: solid $success;
    }

    OutputViewer .tool-error {
        border: solid $error;
    }

    OutputViewer .iteration-marker {
        background: $primary-darken-2;
        color: $text;
        text-style: bold;
        padding: 0 2;
        margin: 1 0;
    }
    """

    auto_scroll: reactive[bool] = reactive(True)

    def __init__(self, **kwargs):
        super().__init__(
            highlight=True,
            markup=True,
            wrap=True,
            **kwargs
        )
        self._tool_call_count = 0

    def append_agent_output(self, text: str) -> None:
        """Append agent output with markdown rendering."""
        if not text.strip():
            return

        # Check if it looks like code
        if text.startswith("```"):
            # Extract language and code
            lines = text.split("\n")
            lang = lines[0].replace("```", "").strip() or "text"
            code = "\n".join(lines[1:-1]) if lines[-1] == "```" else "\n".join(lines[1:])
            self.write(Syntax(code, lang, theme="monokai", line_numbers=True))
        else:
            # Render as markdown
            try:
                self.write(Markdown(text))
            except Exception:
                self.write(text)

        if self.auto_scroll:
            self.scroll_end(animate=False)

    def append_tool_call(
        self,
        tool_name: str,
        tool_input: dict,
        result: str,
        status: str = "success"
    ) -> None:
        """Display tool call with collapsible details."""
        self._tool_call_count += 1

        # Status indicator
        if status == "success":
            icon = ""
            style = "green"
        elif status == "error":
            icon = ""
            style = "red"
        else:
            icon = ""
            style = "yellow"

        # Header
        header = Text()
        header.append(f" TOOL #{self._tool_call_count}: ", style="bold")
        header.append(tool_name, style=f"bold {style}")
        header.append(f" {icon}", style=style)

        # Input summary
        input_preview = json.dumps(tool_input, indent=2)[:200]
        if len(json.dumps(tool_input)) > 200:
            input_preview += "..."

        # Result summary
        result_preview = str(result)[:300]
        if len(str(result)) > 300:
            result_preview += "..."

        # Build panel content
        content = Text()
        content.append("Input: ", style="dim")
        content.append(input_preview + "\n", style="cyan")
        content.append("Result: ", style="dim")
        content.append(result_preview, style="green" if status == "success" else "red")

        panel = Panel(
            content,
            title=str(header),
            border_style=style,
            expand=True,
        )

        self.write(panel)

        if self.auto_scroll:
            self.scroll_end(animate=False)

    def append_iteration_marker(self, iteration: int) -> None:
        """Add a visible iteration marker."""
        marker = Text()
        marker.append(f"  Iteration {iteration} ", style="bold white on blue")
        marker.append(" " + "" * 40, style="blue")

        self.write("")  # Blank line
        self.write(marker)
        self.write("")  # Blank line

        if self.auto_scroll:
            self.scroll_end(animate=False)

    def append_error(self, error: str) -> None:
        """Display error with red highlighting."""
        panel = Panel(
            Text(error, style="bold red"),
            title=" Error",
            border_style="red",
            expand=True,
        )
        self.write(panel)

        if self.auto_scroll:
            self.scroll_end(animate=False)

    def append_validation_gate(self, name: str, status: str, evidence: list) -> None:
        """Display validation gate result."""
        if status == "passed":
            icon = ""
            style = "green"
        elif status == "failed":
            icon = ""
            style = "red"
        elif status == "skipped":
            icon = ""
            style = "yellow"
        else:
            icon = ""
            style = "blue"

        content = Text()
        content.append(f"{icon} {name}\n", style=f"bold {style}")

        if evidence:
            content.append("Evidence:\n", style="dim")
            for e in evidence[:5]:  # Limit to 5
                content.append(f"   {e}\n", style="cyan")

        panel = Panel(
            content,
            title=" Validation Gate",
            border_style=style,
            expand=True,
        )
        self.write(panel)

    def toggle_auto_scroll(self) -> None:
        """Toggle auto-scroll behavior."""
        self.auto_scroll = not self.auto_scroll
        self.notify(
            f"Auto-scroll {'enabled' if self.auto_scroll else 'disabled'}",
            severity="information"
        )

    def clear_output(self) -> None:
        """Clear all output."""
        self.clear()
        self._tool_call_count = 0
