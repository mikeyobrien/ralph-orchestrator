# ABOUTME: Rich terminal formatter with colors, panels, and progress indicators
# ABOUTME: Provides visually enhanced output using the Rich library with smart content detection

"""Rich terminal output formatter for Claude adapter.

This formatter provides intelligent content detection and rendering:
- Diffs are rendered with color-coded additions/deletions
- Code blocks get syntax highlighting
- Markdown is rendered with proper formatting
- Error tracebacks are highlighted for readability
"""

import logging
import re
from datetime import datetime
from io import StringIO
from typing import Optional

from .base import (
    MessageType,
    OutputFormatter,
    ToolCallInfo,
    VerbosityLevel,
)
from .content_detector import ContentDetector, ContentType

_logger = logging.getLogger(__name__)

# Try to import Rich components with fallback
try:
    from rich.console import Console
    from rich.panel import Panel
    from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn
    from rich.syntax import Syntax
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
        smart_detection: bool = True,
    ) -> None:
        """Initialize rich terminal formatter.

        Args:
            verbosity: Output verbosity level
            console: Optional Rich console instance (creates new if None)
            smart_detection: Enable smart content detection (diff, code, markdown)
        """
        super().__init__(verbosity)
        self._rich_available = RICH_AVAILABLE
        self._smart_detection = smart_detection
        self._content_detector = ContentDetector() if smart_detection else None

        if RICH_AVAILABLE:
            self._console = console or Console()
            # Import DiffFormatter for diff rendering
            from .console import DiffFormatter
            self._diff_formatter = DiffFormatter(self._console)
        else:
            self._console = None
            self._diff_formatter = None

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

        With smart_detection enabled, detects and renders:
        - Diffs with color-coded additions/deletions
        - Code blocks with syntax highlighting
        - Markdown with proper formatting
        - Error tracebacks with special highlighting

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

        # Summarize if needed (only for normal verbosity)
        display_message = message
        if self._verbosity == VerbosityLevel.NORMAL and len(message) > 1000:
            display_message = self.summarize_content(message, 1000)

        if not self._rich_available:
            return f"ASSISTANT: {display_message}"

        # Use smart detection if enabled
        if self._smart_detection and self._content_detector:
            content_type = self._content_detector.detect(display_message)
            return self._render_smart_content(display_message, content_type)

        # Fallback to simple formatting
        icon = self.ICONS["assistant"]
        return f"{icon} [{self.COLORS['assistant']}]{escape(display_message)}[/]"

    def _render_smart_content(self, text: str, content_type: ContentType) -> str:
        """Render content based on detected type.

        Args:
            text: Text content to render
            content_type: Detected content type

        Returns:
            Formatted string (may include Rich markup)
        """
        if content_type == ContentType.DIFF:
            return self._render_diff(text)
        elif content_type == ContentType.CODE_BLOCK:
            return self._render_code_blocks(text)
        elif content_type == ContentType.MARKDOWN:
            return self._render_markdown(text)
        elif content_type == ContentType.MARKDOWN_TABLE:
            return self._render_markdown(text)  # Tables use markdown renderer
        elif content_type == ContentType.ERROR_TRACEBACK:
            return self._render_traceback(text)
        else:
            # Plain text - escape and format
            icon = self.ICONS["assistant"]
            return f"{icon} [{self.COLORS['assistant']}]{escape(text)}[/]"

    def _render_diff(self, text: str) -> str:
        """Render diff content with colors.

        Uses the DiffFormatter for enhanced diff visualization with
        color-coded additions/deletions and file statistics.

        Args:
            text: Diff text to render

        Returns:
            Empty string (diff is printed directly to console)
        """
        if self._diff_formatter and self._console:
            # DiffFormatter prints directly, so capture would require buffer
            # For now, print directly and return marker
            self._diff_formatter.format_and_print(text)
            return ""  # Already printed
        return f"[dim]{text}[/dim]"

    def _render_code_blocks(self, text: str) -> str:
        """Render text with code blocks using syntax highlighting.

        Extracts code blocks, renders them with Rich Syntax, and
        formats the surrounding text.

        Args:
            text: Text containing code blocks

        Returns:
            Formatted string with code block markers for print_smart()
        """
        if not self._console or not self._content_detector:
            return f"[dim]{text}[/dim]"

        # Split by code blocks and render each part
        parts = []
        pattern = r"```(\w+)?\n(.*?)\n```"
        last_end = 0

        for match in re.finditer(pattern, text, re.DOTALL):
            # Text before code block
            before = text[last_end:match.start()].strip()
            if before:
                parts.append(("text", before))

            # Code block
            language = match.group(1) or "text"
            code = match.group(2)
            parts.append(("code", language, code))
            last_end = match.end()

        # Text after last code block
        after = text[last_end:].strip()
        if after:
            parts.append(("text", after))

        # Render to string buffer using console
        buffer = StringIO()
        temp_console = Console(file=buffer, force_terminal=True, width=100)

        for part in parts:
            if part[0] == "text":
                temp_console.print(part[1], markup=True, highlight=True)
            else:  # code
                _, language, code = part
                syntax = Syntax(
                    code,
                    language,
                    theme="monokai",
                    line_numbers=True,
                    word_wrap=True,
                )
                temp_console.print(syntax)

        return buffer.getvalue()

    def _render_markdown(self, text: str) -> str:
        """Render markdown content with Rich formatting.

        Uses Rich's Markdown renderer for headings, lists, emphasis, etc.

        Args:
            text: Markdown text to render

        Returns:
            Formatted markdown string
        """
        if not self._console:
            return text

        try:
            from rich.markdown import Markdown

            # Preprocess for task lists
            processed = self._preprocess_markdown(text)

            buffer = StringIO()
            temp_console = Console(file=buffer, force_terminal=True, width=100)
            temp_console.print(Markdown(processed))
            return buffer.getvalue()
        except ImportError:
            return text

    def _preprocess_markdown(self, text: str) -> str:
        """Preprocess markdown for enhanced rendering.

        Converts task list checkboxes to visual indicators.

        Args:
            text: Raw markdown

        Returns:
            Preprocessed markdown
        """
        # Convert task lists: [ ] -> ☐, [x] -> ☑
        text = re.sub(r"\[\s\]", "☐", text)
        text = re.sub(r"\[[xX]\]", "☑", text)
        return text

    def _render_traceback(self, text: str) -> str:
        """Render error traceback with syntax highlighting.

        Uses Python syntax highlighting for better readability.

        Args:
            text: Traceback text to render

        Returns:
            Formatted traceback string
        """
        if not self._console:
            return f"[red]{text}[/red]"

        try:
            buffer = StringIO()
            temp_console = Console(file=buffer, force_terminal=True, width=100)
            temp_console.print("[red bold]⚠ Error Traceback:[/red bold]")
            syntax = Syntax(
                text,
                "python",
                theme="monokai",
                line_numbers=False,
                word_wrap=True,
                background_color="grey11",
            )
            temp_console.print(syntax)
            return buffer.getvalue()
        except Exception as e:
            _logger.warning("Rich traceback rendering failed: %s: %s", type(e).__name__, e)
            return f"[red]{escape(text)}[/red]"

    def print_smart(self, message: str, iteration: int = 0) -> None:
        """Print message with smart content detection directly to console.

        This is the preferred method for displaying assistant messages
        as it handles all content types appropriately and prints directly.

        Args:
            message: Message text to print
            iteration: Current iteration number
        """
        if not self.should_display(MessageType.ASSISTANT):
            return

        if self._verbosity == VerbosityLevel.QUIET:
            return

        if not self._console:
            print(f"ASSISTANT: {message}")
            return

        # Use smart detection
        if self._smart_detection and self._content_detector:
            content_type = self._content_detector.detect(message)

            if content_type == ContentType.DIFF:
                # DiffFormatter prints directly
                if self._diff_formatter:
                    self._diff_formatter.format_and_print(message)
                return

        # For other content types, use format and print
        formatted = self.format_assistant_message(message, iteration)
        if formatted:
            self._console.print(formatted, markup=True)

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
