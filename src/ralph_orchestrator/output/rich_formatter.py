# ABOUTME: Rich terminal formatter with colors, panels, and progress indicators
# ABOUTME: Provides visually enhanced output using the Rich library

"""Rich terminal output formatter for Claude adapter."""

from datetime import datetime
from typing import Optional

from .base import (
    FormatContext,
    MessageType,
    OutputFormatter,
    ToolCallInfo,
    VerbosityLevel,
)

# Try to import Rich components with fallback
try:
    from rich.console import Console
    from rich.panel import Panel
    from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn
    from rich.syntax import Syntax
    from rich.table import Table
    from rich.text import Text
    from rich.markup import escape

    RICH_AVAILABLE = True
except ImportError:
    RICH_AVAILABLE = False
    Console = None  # type: ignore
    Panel = None  # type: ignore


class RichTerminalFormatter(OutputFormatter):
    """Rich terminal formatter with colors, panels, and progress indicators.

    Provides visually enhanced output using the Rich library for terminal
    display. Falls back to plain text if Rich is not available.
    """

    # Color scheme
    COLORS = {
        "tool_name": "bold cyan",
        "tool_id": "dim",
        "success": "bold green",
        "error": "bold red",
        "warning": "yellow",
        "info": "blue",
        "timestamp": "dim white",
        "header": "bold magenta",
        "assistant": "white",
        "system": "dim cyan",
        "token_input": "green",
        "token_output": "yellow",
        "cost": "bold yellow",
    }

    # Icons
    ICONS = {
        "tool": "",
        "success": "",
        "error": "",
        "warning": "",
        "info": "",
        "assistant": "",
        "system": "",
        "token": "",
        "clock": "",
        "progress": "",
    }

    def __init__(
        self,
        verbosity: VerbosityLevel = VerbosityLevel.NORMAL,
        console: Optional["Console"] = None,
    ) -> None:
        """Initialize rich terminal formatter.

        Args:
            verbosity: Output verbosity level
            console: Optional Rich console instance (creates new if None)
        """
        super().__init__(verbosity)
        self._rich_available = RICH_AVAILABLE

        if RICH_AVAILABLE:
            self._console = console or Console()
        else:
            self._console = None

    @property
    def console(self) -> Optional["Console"]:
        """Get the Rich console instance."""
        return self._console

    def _timestamp(self) -> str:
        """Get formatted timestamp string with Rich markup."""
        ts = datetime.now().strftime("%H:%M:%S")
        if self._rich_available:
            return f"[{self.COLORS['timestamp']}]{ts}[/]"
        return ts

    def _full_timestamp(self) -> str:
        """Get full timestamp with date."""
        ts = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        if self._rich_available:
            return f"[{self.COLORS['timestamp']}]{ts}[/]"
        return ts

    def format_tool_call(
        self,
        tool_info: ToolCallInfo,
        iteration: int = 0,
    ) -> str:
        """Format a tool call for rich terminal display.

        Args:
            tool_info: Tool call information
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        if not self.should_display(MessageType.TOOL_CALL):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.TOOL_CALL, tool_info, context)

        if not self._rich_available:
            return self._format_tool_call_plain(tool_info)

        # Build rich formatted output
        icon = self.ICONS["tool"]
        name_color = self.COLORS["tool_name"]
        id_color = self.COLORS["tool_id"]

        lines = [
            f"{icon} [{name_color}]TOOL CALL: {tool_info.tool_name}[/]",
            f"   [{id_color}]ID: {tool_info.tool_id[:12]}...[/]",
        ]

        if self._verbosity.value >= VerbosityLevel.VERBOSE.value:
            if tool_info.input_params:
                lines.append(f"   [{self.COLORS['info']}]Input Parameters:[/]")
                for key, value in tool_info.input_params.items():
                    value_str = str(value)
                    if len(value_str) > 100:
                        value_str = value_str[:97] + "..."
                    # Escape Rich markup in values
                    if self._rich_available:
                        value_str = escape(value_str)
                    lines.append(f"     - {key}: {value_str}")

        return "\n".join(lines)

    def _format_tool_call_plain(self, tool_info: ToolCallInfo) -> str:
        """Plain fallback for tool call formatting."""
        lines = [
            f"TOOL CALL: {tool_info.tool_name}",
            f"  ID: {tool_info.tool_id[:12]}...",
        ]
        if tool_info.input_params:
            for key, value in tool_info.input_params.items():
                value_str = str(value)[:100]
                lines.append(f"  {key}: {value_str}")
        return "\n".join(lines)

    def format_tool_result(
        self,
        tool_info: ToolCallInfo,
        iteration: int = 0,
    ) -> str:
        """Format a tool result for rich terminal display.

        Args:
            tool_info: Tool call info with result
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        if not self.should_display(MessageType.TOOL_RESULT):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.TOOL_RESULT, tool_info, context)

        if not self._rich_available:
            return self._format_tool_result_plain(tool_info)

        # Determine status styling
        if tool_info.is_error:
            status_icon = self.ICONS["error"]
            status_color = self.COLORS["error"]
            status_text = "ERROR"
        else:
            status_icon = self.ICONS["success"]
            status_color = self.COLORS["success"]
            status_text = "Success"

        duration = f" ({tool_info.duration_ms}ms)" if tool_info.duration_ms else ""

        lines = [
            f"{status_icon} [{status_color}]TOOL RESULT: {tool_info.tool_name}{duration}[/]",
            f"   [{self.COLORS['tool_id']}]ID: {tool_info.tool_id[:12]}...[/]",
            f"   Status: [{status_color}]{status_text}[/]",
        ]

        if self._verbosity.value >= VerbosityLevel.VERBOSE.value and tool_info.result:
            result_str = str(tool_info.result)
            if len(result_str) > 500:
                result_str = self.summarize_content(result_str, 500)
            # Escape Rich markup in result
            if self._rich_available:
                result_str = escape(result_str)
            lines.append(f"   [{self.COLORS['info']}]Output:[/]")
            for line in result_str.split("\n")[:20]:  # Limit lines
                lines.append(f"     {line}")
            if result_str.count("\n") > 20:
                lines.append(f"     [{self.COLORS['timestamp']}]... ({result_str.count(chr(10)) - 20} more lines)[/]")

        return "\n".join(lines)

    def _format_tool_result_plain(self, tool_info: ToolCallInfo) -> str:
        """Plain fallback for tool result formatting."""
        status = "ERROR" if tool_info.is_error else "Success"
        lines = [
            f"TOOL RESULT: {tool_info.tool_name}",
            f"  Status: {status}",
        ]
        if tool_info.result:
            lines.append(f"  Output: {str(tool_info.result)[:200]}")
        return "\n".join(lines)

    def format_assistant_message(
        self,
        message: str,
        iteration: int = 0,
    ) -> str:
        """Format an assistant message for rich terminal display.

        Args:
            message: Assistant message text
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        if not self.should_display(MessageType.ASSISTANT):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.ASSISTANT, message, context)

        if self._verbosity == VerbosityLevel.QUIET:
            return ""

        # Summarize if needed
        display_message = message
        if self._verbosity == VerbosityLevel.NORMAL and len(message) > 1000:
            display_message = self.summarize_content(message, 1000)

        if not self._rich_available:
            return f"ASSISTANT: {display_message}"

        icon = self.ICONS["assistant"]
        return f"{icon} [{self.COLORS['assistant']}]{display_message}[/]"

    def format_system_message(
        self,
        message: str,
        iteration: int = 0,
    ) -> str:
        """Format a system message for rich terminal display.

        Args:
            message: System message text
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        if not self.should_display(MessageType.SYSTEM):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.SYSTEM, message, context)

        if not self._rich_available:
            return f"SYSTEM: {message}"

        icon = self.ICONS["system"]
        return f"{icon} [{self.COLORS['system']}]SYSTEM: {message}[/]"

    def format_error(
        self,
        error: str,
        exception: Optional[Exception] = None,
        iteration: int = 0,
    ) -> str:
        """Format an error for rich terminal display.

        Args:
            error: Error message
            exception: Optional exception object
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.ERROR, error, context)

        if not self._rich_available:
            return f"ERROR: {error}"

        icon = self.ICONS["error"]
        color = self.COLORS["error"]

        lines = [
            f"\n{icon} [{color}]ERROR (Iteration {iteration})[/]",
            f"   [{color}]{error}[/]",
        ]

        if exception and self._verbosity.value >= VerbosityLevel.VERBOSE.value:
            lines.append(f"   [{self.COLORS['warning']}]Type: {type(exception).__name__}[/]")
            import traceback

            tb = "".join(traceback.format_exception(type(exception), exception, exception.__traceback__))
            lines.append(f"   [{self.COLORS['timestamp']}]Traceback:[/]")
            for line in tb.split("\n")[:15]:  # Limit traceback lines
                if line.strip():
                    lines.append(f"     {escape(line)}" if self._rich_available else f"     {line}")

        return "\n".join(lines)

    def format_progress(
        self,
        message: str,
        current: int = 0,
        total: int = 0,
        iteration: int = 0,
    ) -> str:
        """Format progress information for rich terminal display.

        Args:
            message: Progress message
            current: Current progress value
            total: Total progress value
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        if not self.should_display(MessageType.PROGRESS):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.PROGRESS, message, context)

        if not self._rich_available:
            if total > 0:
                pct = (current / total) * 100
                return f"[{pct:.0f}%] {message}"
            return f"[...] {message}"

        icon = self.ICONS["progress"]
        if total > 0:
            pct = (current / total) * 100
            bar_width = 20
            filled = int(bar_width * current / total)
            bar = "" * filled + "" * (bar_width - filled)
            return f"{icon} [{self.COLORS['info']}][{bar}] {pct:.0f}%[/] {message}"
        return f"{icon} [{self.COLORS['info']}][...][/] {message}"

    def format_token_usage(self, show_session: bool = True) -> str:
        """Format token usage summary for rich terminal display.

        Args:
            show_session: Include session totals

        Returns:
            Formatted string representation
        """
        usage = self._token_usage

        if not self._rich_available:
            lines = [
                f"TOKEN USAGE: {usage.total_tokens:,} (${usage.cost:.4f})",
            ]
            if show_session:
                lines.append(f"  Session: {usage.session_total_tokens:,} (${usage.session_cost:.4f})")
            return "\n".join(lines)

        icon = self.ICONS["token"]
        input_color = self.COLORS["token_input"]
        output_color = self.COLORS["token_output"]
        cost_color = self.COLORS["cost"]

        lines = [
            f"\n{icon} [{self.COLORS['header']}]TOKEN USAGE[/]",
            f"   Current: [{input_color}]{usage.input_tokens:,} in[/] | [{output_color}]{usage.output_tokens:,} out[/] | [{cost_color}]${usage.cost:.4f}[/]",
        ]

        if show_session:
            lines.append(
                f"   Session: [{input_color}]{usage.session_input_tokens:,} in[/] | [{output_color}]{usage.session_output_tokens:,} out[/] | [{cost_color}]${usage.session_cost:.4f}[/]"
            )

        if usage.model:
            lines.append(f"   [{self.COLORS['timestamp']}]Model: {usage.model}[/]")

        return "\n".join(lines)

    def format_section_header(self, title: str, iteration: int = 0) -> str:
        """Format a section header for rich terminal display.

        Args:
            title: Section title
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        if not self._rich_available:
            sep = "=" * 60
            header_title = f"{title} (Iteration {iteration})" if iteration else title
            return f"\n{sep}\n{header_title}\n{sep}"

        header_title = f"{title} (Iteration {iteration})" if iteration else title
        sep = "" * 50
        return f"\n[{self.COLORS['header']}]{sep}\n{header_title}\n{sep}[/]"

    def format_section_footer(self) -> str:
        """Format a section footer for rich terminal display.

        Returns:
            Formatted string representation
        """
        elapsed = self.get_elapsed_time()

        if not self._rich_available:
            return f"\n{'=' * 50}\nElapsed: {elapsed:.1f}s\n"

        icon = self.ICONS["clock"]
        return f"\n[{self.COLORS['timestamp']}]{icon} Elapsed: {elapsed:.1f}s[/]\n"

    def print(self, text: str) -> None:
        """Print formatted text to console.

        Args:
            text: Rich-formatted text to print
        """
        if self._console:
            self._console.print(text, markup=True)
        else:
            # Strip markup for plain output
            import re

            plain = re.sub(r"\[/?[^\]]+\]", "", text)
            print(plain)

    def print_panel(self, content: str, title: str = "", border_style: str = "blue") -> None:
        """Print content in a Rich panel.

        Args:
            content: Content to display
            title: Panel title
            border_style: Panel border color
        """
        if self._console and self._rich_available and Panel:
            panel = Panel(content, title=title, border_style=border_style)
            self._console.print(panel)
        else:
            if title:
                print(f"\n=== {title} ===")
            print(content)
            print()

    def create_progress_bar(self) -> Optional["Progress"]:
        """Create a Rich progress bar instance.

        Returns:
            Progress instance or None if Rich not available
        """
        if not self._rich_available or not Progress:
            return None

        return Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            BarColumn(),
            TextColumn("[progress.percentage]{task.percentage:>3.0f}%"),
            console=self._console,
        )
