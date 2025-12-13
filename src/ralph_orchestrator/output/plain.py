# ABOUTME: Plain text output formatter for non-terminal environments
# ABOUTME: Provides basic text formatting without colors or special characters

"""Plain text output formatter for Claude adapter."""

from datetime import datetime
from typing import Optional

from .base import (
    FormatContext,
    MessageType,
    OutputFormatter,
    ToolCallInfo,
    VerbosityLevel,
)


class PlainTextFormatter(OutputFormatter):
    """Plain text formatter for environments without rich terminal support.

    Produces readable output without ANSI codes, colors, or special characters.
    Suitable for logging to files or basic terminal output.
    """

    # Formatting constants
    SEPARATOR_WIDTH = 60
    HEADER_CHAR = "="
    SUBHEADER_CHAR = "-"
    SECTION_CHAR = "#"

    def __init__(self, verbosity: VerbosityLevel = VerbosityLevel.NORMAL) -> None:
        """Initialize plain text formatter.

        Args:
            verbosity: Output verbosity level
        """
        super().__init__(verbosity)

    def _timestamp(self) -> str:
        """Get formatted timestamp string."""
        return datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    def _separator(self, char: str = "-", width: int = None) -> str:
        """Create a separator line."""
        return char * (width or self.SEPARATOR_WIDTH)

    def format_tool_call(
        self,
        tool_info: ToolCallInfo,
        iteration: int = 0,
    ) -> str:
        """Format a tool call for plain text display.

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

        lines = [
            self._separator(),
            f"[{self._timestamp()}] TOOL CALL: {tool_info.tool_name}",
            f"  ID: {tool_info.tool_id[:12]}...",
        ]

        if self._verbosity.value >= VerbosityLevel.VERBOSE.value:
            if tool_info.input_params:
                lines.append("  Input Parameters:")
                for key, value in tool_info.input_params.items():
                    value_str = str(value)
                    if len(value_str) > 100:
                        value_str = value_str[:97] + "..."
                    lines.append(f"    {key}: {value_str}")

        lines.append(self._separator())
        return "\n".join(lines)

    def format_tool_result(
        self,
        tool_info: ToolCallInfo,
        iteration: int = 0,
    ) -> str:
        """Format a tool result for plain text display.

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

        status = "ERROR" if tool_info.is_error else "Success"
        duration = f" ({tool_info.duration_ms}ms)" if tool_info.duration_ms else ""

        lines = [
            f"[TOOL RESULT] {tool_info.tool_name}{duration}",
            f"  ID: {tool_info.tool_id[:12]}...",
            f"  Status: {status}",
        ]

        if self._verbosity.value >= VerbosityLevel.VERBOSE.value and tool_info.result:
            result_str = str(tool_info.result)
            if len(result_str) > 500:
                result_str = self.summarize_content(result_str, 500)
            lines.append("  Output:")
            for line in result_str.split("\n"):
                lines.append(f"    {line}")

        lines.append(self._separator())
        return "\n".join(lines)

    def format_assistant_message(
        self,
        message: str,
        iteration: int = 0,
    ) -> str:
        """Format an assistant message for plain text display.

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

        # Summarize if too long and not verbose
        if self._verbosity == VerbosityLevel.NORMAL and len(message) > 1000:
            message = self.summarize_content(message, 1000)

        return f"[{self._timestamp()}] ASSISTANT:\n{message}\n"

    def format_system_message(
        self,
        message: str,
        iteration: int = 0,
    ) -> str:
        """Format a system message for plain text display.

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

        return f"[{self._timestamp()}] SYSTEM: {message}\n"

    def format_error(
        self,
        error: str,
        exception: Optional[Exception] = None,
        iteration: int = 0,
    ) -> str:
        """Format an error for plain text display.

        Args:
            error: Error message
            exception: Optional exception object
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.ERROR, error, context)

        lines = [
            self._separator(self.HEADER_CHAR),
            f"[{self._timestamp()}] ERROR (Iteration {iteration})",
            f"  Message: {error}",
        ]

        if exception and self._verbosity.value >= VerbosityLevel.VERBOSE.value:
            lines.append(f"  Type: {type(exception).__name__}")
            import traceback

            tb = "".join(traceback.format_exception(type(exception), exception, exception.__traceback__))
            lines.append("  Traceback:")
            for line in tb.split("\n"):
                lines.append(f"    {line}")

        lines.append(self._separator(self.HEADER_CHAR))
        return "\n".join(lines)

    def format_progress(
        self,
        message: str,
        current: int = 0,
        total: int = 0,
        iteration: int = 0,
    ) -> str:
        """Format progress information for plain text display.

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

        if total > 0:
            pct = (current / total) * 100
            bar_width = 30
            filled = int(bar_width * current / total)
            bar = "#" * filled + "-" * (bar_width - filled)
            return f"[{bar}] {pct:.1f}% - {message}"
        return f"[...] {message}"

    def format_token_usage(self, show_session: bool = True) -> str:
        """Format token usage summary for plain text display.

        Args:
            show_session: Include session totals

        Returns:
            Formatted string representation
        """
        usage = self._token_usage
        lines = [
            self._separator(self.SUBHEADER_CHAR),
            "TOKEN USAGE:",
            f"  Current: {usage.total_tokens:,} tokens (${usage.cost:.4f})",
            f"    Input: {usage.input_tokens:,} | Output: {usage.output_tokens:,}",
        ]

        if show_session:
            lines.extend([
                f"  Session: {usage.session_total_tokens:,} tokens (${usage.session_cost:.4f})",
                f"    Input: {usage.session_input_tokens:,} | Output: {usage.session_output_tokens:,}",
            ])

        if usage.model:
            lines.append(f"  Model: {usage.model}")

        lines.append(self._separator(self.SUBHEADER_CHAR))
        return "\n".join(lines)

    def format_section_header(self, title: str, iteration: int = 0) -> str:
        """Format a section header for plain text display.

        Args:
            title: Section title
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        lines = [
            "",
            self._separator(self.HEADER_CHAR),
            f"{title} (Iteration {iteration})" if iteration else title,
            self._separator(self.HEADER_CHAR),
        ]
        return "\n".join(lines)

    def format_section_footer(self) -> str:
        """Format a section footer for plain text display.

        Returns:
            Formatted string representation
        """
        elapsed = self.get_elapsed_time()
        return f"\n{self._separator(self.SUBHEADER_CHAR)}\nElapsed: {elapsed:.1f}s\n"
