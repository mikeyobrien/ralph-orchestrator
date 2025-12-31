# ABOUTME: Tests for the new output formatter module
# ABOUTME: Tests VerbosityLevel, OutputFormatter base class, and all formatter implementations

"""Tests for the new output formatter module."""

import json
from datetime import datetime

import pytest

from ralph_orchestrator.output import (
    FormatContext,
    JsonFormatter,
    MessageType,
    PlainTextFormatter,
    RichTerminalFormatter,
    TokenUsage,
    ToolCallInfo,
    VerbosityLevel,
    create_formatter,
)
from ralph_orchestrator.output.content_detector import ContentDetector, ContentType


class TestVerbosityLevel:
    """Tests for VerbosityLevel enum."""

    def test_verbosity_values(self):
        """Test verbosity level ordering."""
        assert VerbosityLevel.QUIET.value == 0
        assert VerbosityLevel.NORMAL.value == 1
        assert VerbosityLevel.VERBOSE.value == 2
        assert VerbosityLevel.DEBUG.value == 3

    def test_verbosity_comparison(self):
        """Test verbosity levels can be compared."""
        assert VerbosityLevel.QUIET.value < VerbosityLevel.NORMAL.value
        assert VerbosityLevel.NORMAL.value < VerbosityLevel.VERBOSE.value
        assert VerbosityLevel.VERBOSE.value < VerbosityLevel.DEBUG.value


class TestMessageType:
    """Tests for MessageType enum."""

    def test_message_types(self):
        """Test all message types are defined."""
        assert MessageType.SYSTEM.value == "system"
        assert MessageType.ASSISTANT.value == "assistant"
        assert MessageType.USER.value == "user"
        assert MessageType.TOOL_CALL.value == "tool_call"
        assert MessageType.TOOL_RESULT.value == "tool_result"
        assert MessageType.ERROR.value == "error"
        assert MessageType.INFO.value == "info"
        assert MessageType.PROGRESS.value == "progress"


class TestTokenUsage:
    """Tests for TokenUsage dataclass."""

    def test_default_values(self):
        """Test default token usage values."""
        usage = TokenUsage()
        assert usage.input_tokens == 0
        assert usage.output_tokens == 0
        assert usage.total_tokens == 0
        assert usage.cost == 0.0
        assert usage.session_total_tokens == 0
        assert usage.session_cost == 0.0

    def test_add_tokens(self):
        """Test adding tokens updates all counts."""
        usage = TokenUsage()
        usage.add(input_tokens=100, output_tokens=50, cost=0.01, model="claude")

        assert usage.input_tokens == 100
        assert usage.output_tokens == 50
        assert usage.total_tokens == 150
        assert usage.cost == 0.01
        assert usage.model == "claude"
        assert usage.session_input_tokens == 100
        assert usage.session_output_tokens == 50
        assert usage.session_total_tokens == 150
        assert usage.session_cost == 0.01

    def test_cumulative_session_tokens(self):
        """Test session tokens accumulate across adds."""
        usage = TokenUsage()
        usage.add(input_tokens=100, output_tokens=50, cost=0.01)
        usage.add(input_tokens=200, output_tokens=100, cost=0.02)

        # Current should be last add
        assert usage.input_tokens == 200
        assert usage.output_tokens == 100
        assert usage.cost == 0.02

        # Session should be cumulative
        assert usage.session_input_tokens == 300
        assert usage.session_output_tokens == 150
        assert usage.session_total_tokens == 450
        assert usage.session_cost == 0.03

    def test_reset_current(self):
        """Test resetting current while keeping session."""
        usage = TokenUsage()
        usage.add(input_tokens=100, output_tokens=50, cost=0.01)
        usage.reset_current()

        assert usage.input_tokens == 0
        assert usage.output_tokens == 0
        assert usage.cost == 0.0
        assert usage.session_total_tokens == 100 + 50  # Session preserved


class TestToolCallInfo:
    """Tests for ToolCallInfo dataclass."""

    def test_default_values(self):
        """Test default tool call info values."""
        info = ToolCallInfo(tool_name="Read", tool_id="abc123")
        assert info.tool_name == "Read"
        assert info.tool_id == "abc123"
        assert info.input_params == {}
        assert info.result is None
        assert info.is_error is False

    def test_custom_values(self):
        """Test tool call info with custom values."""
        now = datetime.now()
        info = ToolCallInfo(
            tool_name="Write",
            tool_id="xyz789",
            input_params={"path": "test.py", "content": "code"},
            start_time=now,
            result="Success",
            is_error=False,
            duration_ms=150,
        )
        assert info.tool_name == "Write"
        assert info.input_params == {"path": "test.py", "content": "code"}
        assert info.duration_ms == 150


class TestFormatContext:
    """Tests for FormatContext dataclass."""

    def test_default_values(self):
        """Test default context values."""
        ctx = FormatContext()
        assert ctx.iteration == 0
        assert ctx.verbosity == VerbosityLevel.NORMAL
        assert ctx.timestamp is not None
        assert ctx.token_usage is not None

    def test_custom_values(self):
        """Test context with custom values."""
        usage = TokenUsage()
        ctx = FormatContext(
            iteration=5,
            verbosity=VerbosityLevel.DEBUG,
            token_usage=usage,
            metadata={"key": "value"},
        )
        assert ctx.iteration == 5
        assert ctx.verbosity == VerbosityLevel.DEBUG
        assert ctx.metadata == {"key": "value"}


class TestPlainTextFormatter:
    """Tests for PlainTextFormatter."""

    def test_init(self):
        """Test formatter initialization."""
        formatter = PlainTextFormatter()
        assert formatter.verbosity == VerbosityLevel.NORMAL

    def test_verbosity_setting(self):
        """Test setting verbosity level."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.DEBUG)
        assert formatter.verbosity == VerbosityLevel.DEBUG

        formatter.verbosity = VerbosityLevel.QUIET
        assert formatter.verbosity == VerbosityLevel.QUIET

    def test_format_tool_call(self):
        """Test tool call formatting."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.VERBOSE)
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="abc123def456",
            input_params={"path": "test.py"},
        )

        output = formatter.format_tool_call(tool_info, iteration=1)
        assert "TOOL CALL: Read" in output
        assert "abc123def456"[:12] in output
        assert "path" in output
        assert "test.py" in output

    def test_format_tool_call_quiet(self):
        """Test tool call hidden in quiet mode."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.QUIET)
        tool_info = ToolCallInfo(tool_name="Read", tool_id="abc123")

        output = formatter.format_tool_call(tool_info)
        assert output == ""

    def test_format_tool_result(self):
        """Test tool result formatting."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.VERBOSE)
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="abc123def456",
            result="File content here",
            is_error=False,
            duration_ms=50,
        )

        output = formatter.format_tool_result(tool_info)
        assert "TOOL RESULT" in output
        assert "Read" in output
        assert "50ms" in output
        assert "Success" in output
        assert "File content here" in output

    def test_format_tool_result_error(self):
        """Test tool result with error formatting."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.VERBOSE)
        tool_info = ToolCallInfo(
            tool_name="Write",
            tool_id="xyz789",
            result="Permission denied",
            is_error=True,
        )

        output = formatter.format_tool_result(tool_info)
        assert "ERROR" in output

    def test_format_assistant_message(self):
        """Test assistant message formatting."""
        formatter = PlainTextFormatter()
        output = formatter.format_assistant_message("Hello, I can help you!")
        assert "ASSISTANT" in output
        assert "Hello, I can help you!" in output

    def test_format_assistant_message_truncated(self):
        """Test long assistant message truncation."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.NORMAL)
        long_message = "x" * 2000
        output = formatter.format_assistant_message(long_message)
        assert "truncated" in output

    def test_format_system_message(self):
        """Test system message formatting."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.VERBOSE)
        output = formatter.format_system_message("System initialized")
        assert "SYSTEM" in output
        assert "System initialized" in output

    def test_format_error(self):
        """Test error formatting."""
        formatter = PlainTextFormatter()
        output = formatter.format_error("Something went wrong", iteration=5)
        assert "ERROR" in output
        assert "Iteration 5" in output
        assert "Something went wrong" in output

    def test_format_error_with_exception(self):
        """Test error formatting with exception."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.VERBOSE)
        try:
            raise ValueError("Test error")
        except ValueError as e:
            output = formatter.format_error("Error occurred", exception=e)
            assert "ValueError" in output
            assert "Traceback" in output

    def test_format_progress(self):
        """Test progress formatting."""
        formatter = PlainTextFormatter()
        output = formatter.format_progress("Processing", current=50, total=100)
        assert "50.0%" in output
        assert "Processing" in output

    def test_format_token_usage(self):
        """Test token usage formatting."""
        formatter = PlainTextFormatter()
        formatter.update_tokens(input_tokens=100, output_tokens=50, cost=0.01)
        output = formatter.format_token_usage()
        assert "TOKEN USAGE" in output
        assert "150" in output  # total tokens
        assert "$0.01" in output

    def test_format_section_header(self):
        """Test section header formatting."""
        formatter = PlainTextFormatter()
        output = formatter.format_section_header("Test Section", iteration=3)
        assert "Test Section" in output
        assert "Iteration 3" in output

    def test_format_section_footer(self):
        """Test section footer formatting."""
        formatter = PlainTextFormatter()
        output = formatter.format_section_footer()
        assert "Elapsed" in output

    def test_summarize_content(self):
        """Test content summarization."""
        formatter = PlainTextFormatter()
        long_text = "a" * 1000
        summary = formatter.summarize_content(long_text, max_length=100)
        assert len(summary) < len(long_text)
        assert "truncated" in summary

    def test_callbacks(self):
        """Test callback registration and notification."""
        formatter = PlainTextFormatter()
        callback_data = []

        def callback(msg_type, content, ctx):
            callback_data.append((msg_type, content))

        formatter.register_callback(callback)
        formatter.format_assistant_message("Test message")

        assert len(callback_data) == 1
        assert callback_data[0][0] == MessageType.ASSISTANT

    def test_format_tool_call_long_param_truncation(self):
        """Test tool call truncates long parameter values."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.VERBOSE)
        long_value = "x" * 200
        tool_info = ToolCallInfo(
            tool_name="Write",
            tool_id="abc123def456",
            input_params={"content": long_value},
        )
        output = formatter.format_tool_call(tool_info)
        # Should contain truncated value (97 chars + "...")
        assert "..." in output
        # Full value should not be in output
        assert long_value not in output

    def test_format_tool_result_quiet(self):
        """Test tool result returns empty in quiet mode."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.QUIET)
        tool_info = ToolCallInfo(tool_name="Read", tool_id="123", result="ok")
        output = formatter.format_tool_result(tool_info)
        assert output == ""

    def test_format_tool_result_long_summarization(self):
        """Test tool result summarizes long output."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.VERBOSE)
        long_result = "x" * 1000
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="123",
            result=long_result,
        )
        output = formatter.format_tool_result(tool_info)
        # Should be truncated
        assert "truncated" in output
        assert len(output) < len(long_result)

    def test_format_assistant_quiet(self):
        """Test assistant returns empty in quiet mode."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.QUIET)
        output = formatter.format_assistant_message("Hello")
        assert output == ""

    def test_format_system_quiet(self):
        """Test system message returns empty in quiet mode."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.QUIET)
        output = formatter.format_system_message("System init")
        assert output == ""

    def test_format_progress_quiet(self):
        """Test progress returns empty in quiet mode."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.QUIET)
        output = formatter.format_progress("Working", 50, 100)
        assert output == ""

    def test_format_progress_without_total(self):
        """Test progress without total shows indeterminate."""
        formatter = PlainTextFormatter()
        output = formatter.format_progress("Processing", current=0, total=0)
        assert "[...]" in output
        assert "Processing" in output

    def test_format_token_usage_with_model(self):
        """Test token usage includes model when set."""
        formatter = PlainTextFormatter()
        formatter.update_tokens(input_tokens=100, output_tokens=50, cost=0.01, model="claude-3-opus")
        output = formatter.format_token_usage()
        assert "claude-3-opus" in output

    def test_callback_exception_ignored(self):
        """Test callback exceptions don't break formatting."""
        formatter = PlainTextFormatter()

        def bad_callback(msg_type, content, ctx):
            raise ValueError("Callback error")

        formatter.register_callback(bad_callback)
        # Should not raise despite bad callback
        output = formatter.format_assistant_message("Test")
        assert "ASSISTANT" in output
        assert "Test" in output

    def test_token_usage_property(self):
        """Test token_usage property access."""
        formatter = PlainTextFormatter()
        usage = formatter.token_usage
        assert usage is not None
        assert usage.input_tokens == 0
        formatter.update_tokens(input_tokens=100, output_tokens=50)
        assert formatter.token_usage.input_tokens == 100


class TestRichTerminalFormatter:
    """Tests for RichTerminalFormatter."""

    def test_init(self):
        """Test formatter initialization."""
        formatter = RichTerminalFormatter()
        assert formatter.verbosity == VerbosityLevel.NORMAL

    def test_format_tool_call(self):
        """Test tool call formatting with Rich."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="abc123def456",
            input_params={"path": "test.py"},
        )

        output = formatter.format_tool_call(tool_info)
        assert "TOOL CALL" in output
        assert "Read" in output

    def test_format_tool_result_success(self):
        """Test successful tool result with Rich formatting."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="abc123",
            result="content",
            is_error=False,
            duration_ms=100,
        )

        output = formatter.format_tool_result(tool_info)
        assert "TOOL RESULT" in output
        assert "Success" in output or "success" in output.lower()

    def test_format_tool_result_error(self):
        """Test error tool result with Rich formatting."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        tool_info = ToolCallInfo(
            tool_name="Write",
            tool_id="xyz789",
            result="Failed",
            is_error=True,
        )

        output = formatter.format_tool_result(tool_info)
        assert "ERROR" in output

    def test_format_assistant_message(self):
        """Test assistant message with Rich."""
        formatter = RichTerminalFormatter()
        output = formatter.format_assistant_message("Hello!")
        assert "Hello!" in output

    def test_format_error(self):
        """Test error formatting with Rich."""
        formatter = RichTerminalFormatter()
        output = formatter.format_error("Error message", iteration=1)
        assert "ERROR" in output
        assert "Error message" in output

    def test_format_progress(self):
        """Test progress formatting with Rich."""
        formatter = RichTerminalFormatter()
        output = formatter.format_progress("Working", current=25, total=100)
        assert "25" in output
        assert "Working" in output

    def test_format_token_usage(self):
        """Test token usage formatting with Rich."""
        formatter = RichTerminalFormatter()
        formatter.update_tokens(input_tokens=500, output_tokens=200, cost=0.05)
        output = formatter.format_token_usage()
        assert "TOKEN USAGE" in output
        assert "500" in output
        assert "200" in output

    def test_console_property(self):
        """Test console property access."""
        RichTerminalFormatter()
        # Console may or may not be available depending on Rich
        # Just verify the property works

    def test_format_system_message(self):
        """Test system message formatting with Rich."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        output = formatter.format_system_message("System message here")
        assert "SYSTEM" in output
        assert "System message here" in output

    def test_format_section_header(self):
        """Test section header formatting with Rich."""
        formatter = RichTerminalFormatter()
        output = formatter.format_section_header("Test Section", iteration=5)
        assert "Test Section" in output
        assert "Iteration 5" in output

    def test_format_section_header_no_iteration(self):
        """Test section header without iteration number."""
        formatter = RichTerminalFormatter()
        output = formatter.format_section_header("Just Title")
        assert "Just Title" in output

    def test_format_section_footer(self):
        """Test section footer formatting with Rich."""
        formatter = RichTerminalFormatter()
        output = formatter.format_section_footer()
        assert "Elapsed" in output

    def test_print_method(self):
        """Test print method outputs to console."""
        formatter = RichTerminalFormatter()
        # Should not raise - just verify it works
        formatter.print("[bold]Test output[/]")

    def test_print_panel(self):
        """Test print_panel method."""
        formatter = RichTerminalFormatter()
        # Should not raise
        formatter.print_panel("Content here", title="Test Panel", border_style="green")

    def test_create_progress_bar(self):
        """Test creating progress bar."""
        formatter = RichTerminalFormatter()
        progress = formatter.create_progress_bar()
        # May be None if Rich not available, but shouldn't raise
        if progress is not None:
            assert hasattr(progress, 'add_task')

    def test_format_tool_call_with_long_params(self):
        """Test tool call with long parameter values gets truncated."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        long_value = "x" * 200
        tool_info = ToolCallInfo(
            tool_name="Write",
            tool_id="abc123def456",
            input_params={"content": long_value},
        )
        output = formatter.format_tool_call(tool_info)
        # Should be truncated - value itself is truncated to 97 chars + "..."
        assert "..." in output
        # The full 200 char value should not be in output
        assert long_value not in output

    def test_format_tool_result_with_long_output(self):
        """Test tool result with long output gets summarized."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        long_result = "line\n" * 100  # More than 20 lines
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="abc123",
            result=long_result,
            is_error=False,
        )
        output = formatter.format_tool_result(tool_info)
        # Should have indication of more lines
        assert "more lines" in output or "..." in output

    def test_format_progress_without_total(self):
        """Test progress without total shows spinner style."""
        formatter = RichTerminalFormatter()
        output = formatter.format_progress("Working", current=0, total=0)
        assert "Working" in output
        assert "..." in output

    def test_format_token_usage_without_session(self):
        """Test token usage without session totals."""
        formatter = RichTerminalFormatter()
        formatter.update_tokens(input_tokens=100, output_tokens=50, cost=0.01)
        output = formatter.format_token_usage(show_session=False)
        assert "TOKEN USAGE" in output
        assert "100" in output

    def test_format_token_usage_with_model(self):
        """Test token usage displays model name."""
        formatter = RichTerminalFormatter()
        formatter.update_tokens(input_tokens=100, output_tokens=50, cost=0.01, model="claude-3-opus")
        output = formatter.format_token_usage()
        assert "claude-3-opus" in output

    def test_format_error_with_exception_verbose(self):
        """Test error with exception shows traceback in verbose mode."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        try:
            raise ValueError("Test exception")
        except ValueError as e:
            output = formatter.format_error("Error occurred", exception=e, iteration=1)
            assert "ValueError" in output
            assert "Traceback" in output

    def test_format_assistant_quiet_mode(self):
        """Test assistant message hidden in quiet mode."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.QUIET)
        output = formatter.format_assistant_message("Hello")
        assert output == ""

    def test_format_assistant_normal_truncation(self):
        """Test long assistant message gets truncated in normal mode."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.NORMAL)
        long_message = "x" * 2000
        output = formatter.format_assistant_message(long_message)
        # Should be truncated
        assert "truncated" in output or len(output) < len(long_message) + 50


class TestRichFormatterWithoutRich:
    """Tests for RichTerminalFormatter when Rich is not available."""

    def test_fallback_tool_call(self):
        """Test tool call fallback without Rich."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        formatter._rich_available = False
        formatter._console = None

        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="abc123def456",
            input_params={"path": "test.py"},
        )
        output = formatter.format_tool_call(tool_info)
        assert "TOOL CALL: Read" in output
        assert "abc123def456"[:12] in output

    def test_fallback_tool_result(self):
        """Test tool result fallback without Rich."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        formatter._rich_available = False
        formatter._console = None

        tool_info = ToolCallInfo(
            tool_name="Write",
            tool_id="xyz789",
            result="File content here",
            is_error=False,
        )
        output = formatter.format_tool_result(tool_info)
        assert "TOOL RESULT: Write" in output
        assert "Success" in output
        assert "File content here" in output

    def test_fallback_tool_result_error(self):
        """Test error tool result fallback without Rich."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        formatter._rich_available = False
        formatter._console = None

        tool_info = ToolCallInfo(
            tool_name="Write",
            tool_id="xyz789",
            result="Error message",
            is_error=True,
        )
        output = formatter.format_tool_result(tool_info)
        assert "ERROR" in output

    def test_fallback_assistant_message(self):
        """Test assistant message fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        output = formatter.format_assistant_message("Hello there!")
        assert "ASSISTANT: Hello there!" in output

    def test_fallback_system_message(self):
        """Test system message fallback without Rich."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
        formatter._rich_available = False
        formatter._console = None

        output = formatter.format_system_message("System init")
        assert "SYSTEM: System init" in output

    def test_fallback_error(self):
        """Test error fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        output = formatter.format_error("Something failed", iteration=1)
        assert "ERROR: Something failed" in output

    def test_fallback_progress_with_total(self):
        """Test progress with total fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        output = formatter.format_progress("Working", current=50, total=100)
        assert "50%" in output
        assert "Working" in output

    def test_fallback_progress_without_total(self):
        """Test progress without total fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        output = formatter.format_progress("Processing")
        assert "[...]" in output
        assert "Processing" in output

    def test_fallback_token_usage(self):
        """Test token usage fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None
        formatter.update_tokens(input_tokens=100, output_tokens=50, cost=0.01)

        output = formatter.format_token_usage()
        assert "TOKEN USAGE" in output
        assert "150" in output

    def test_fallback_token_usage_with_session(self):
        """Test token usage with session fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None
        formatter.update_tokens(input_tokens=100, output_tokens=50, cost=0.01)
        formatter.update_tokens(input_tokens=200, output_tokens=100, cost=0.02)

        output = formatter.format_token_usage(show_session=True)
        assert "Session" in output

    def test_fallback_section_header(self):
        """Test section header fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        output = formatter.format_section_header("Test", iteration=3)
        assert "=" in output
        assert "Test" in output
        assert "Iteration 3" in output

    def test_fallback_section_footer(self):
        """Test section footer fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        output = formatter.format_section_footer()
        assert "=" in output
        assert "Elapsed" in output

    def test_fallback_print(self):
        """Test print fallback without Rich strips markup."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        # Should not raise and should strip markup
        formatter.print("[bold]Test[/]")

    def test_fallback_print_panel(self):
        """Test print_panel fallback without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        # Should not raise
        formatter.print_panel("Content", title="Title")

    def test_fallback_print_panel_no_title(self):
        """Test print_panel without title fallback."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        # Should not raise
        formatter.print_panel("Content")

    def test_fallback_create_progress_bar(self):
        """Test create_progress_bar returns None without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False
        formatter._console = None

        result = formatter.create_progress_bar()
        assert result is None

    def test_fallback_timestamp(self):
        """Test timestamp formatting without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False

        ts = formatter._timestamp()
        # Should be plain timestamp without markup
        assert "[" not in ts or ts.count("[") == 0

    def test_fallback_full_timestamp(self):
        """Test full timestamp formatting without Rich."""
        formatter = RichTerminalFormatter()
        formatter._rich_available = False

        ts = formatter._full_timestamp()
        # Should contain date
        import datetime
        year = str(datetime.datetime.now().year)
        assert year in ts


class TestJsonFormatter:
    """Tests for JsonFormatter."""

    def test_init(self):
        """Test formatter initialization."""
        formatter = JsonFormatter()
        assert formatter.verbosity == VerbosityLevel.NORMAL

    def test_format_tool_call(self):
        """Test tool call JSON formatting."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.VERBOSE)
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="abc123",
            input_params={"path": "test.py"},
        )

        output = formatter.format_tool_call(tool_info, iteration=1)
        data = json.loads(output)

        assert data["type"] == "tool_call"
        assert data["iteration"] == 1
        assert data["data"]["tool_name"] == "Read"
        assert data["data"]["tool_id"] == "abc123"
        assert data["data"]["input_params"] == {"path": "test.py"}
        assert "timestamp" in data

    def test_format_tool_result(self):
        """Test tool result JSON formatting."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.VERBOSE)
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="abc123",
            result="file content",
            is_error=False,
            duration_ms=50,
        )

        output = formatter.format_tool_result(tool_info)
        data = json.loads(output)

        assert data["type"] == "tool_result"
        assert data["data"]["is_error"] is False
        assert data["data"]["duration_ms"] == 50
        assert data["data"]["result"] == "file content"

    def test_format_assistant_message(self):
        """Test assistant message JSON formatting."""
        formatter = JsonFormatter()
        output = formatter.format_assistant_message("Hello!", iteration=2)
        data = json.loads(output)

        assert data["type"] == "assistant_message"
        assert data["iteration"] == 2
        assert data["data"]["message"] == "Hello!"

    def test_format_assistant_message_truncated(self):
        """Test long message truncation in JSON."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.NORMAL)
        long_message = "x" * 2000
        output = formatter.format_assistant_message(long_message)
        data = json.loads(output)

        assert data["data"]["message_truncated"] is True
        assert data["data"]["message_full_length"] == 2000

    def test_format_system_message(self):
        """Test system message JSON formatting."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.VERBOSE)
        output = formatter.format_system_message("Init complete")
        data = json.loads(output)

        assert data["type"] == "system_message"
        assert data["data"]["message"] == "Init complete"

    def test_format_error(self):
        """Test error JSON formatting."""
        formatter = JsonFormatter()
        output = formatter.format_error("Error occurred", iteration=3)
        data = json.loads(output)

        assert data["type"] == "error"
        assert data["iteration"] == 3
        assert data["data"]["error"] == "Error occurred"

    def test_format_error_with_exception(self):
        """Test error JSON formatting with exception."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.VERBOSE)
        try:
            raise ValueError("Test")
        except ValueError as e:
            output = formatter.format_error("Error", exception=e)
            data = json.loads(output)

            assert data["data"]["exception_type"] == "ValueError"
            assert "traceback" in data["data"]

    def test_format_progress(self):
        """Test progress JSON formatting."""
        formatter = JsonFormatter()
        output = formatter.format_progress("Working", current=50, total=100, iteration=1)
        data = json.loads(output)

        assert data["type"] == "progress"
        assert data["data"]["current"] == 50
        assert data["data"]["total"] == 100
        assert data["data"]["percentage"] == 50.0

    def test_format_token_usage(self):
        """Test token usage JSON formatting."""
        formatter = JsonFormatter()
        formatter.update_tokens(input_tokens=100, output_tokens=50, cost=0.01, model="claude")
        output = formatter.format_token_usage()
        data = json.loads(output)

        assert data["type"] == "token_usage"
        assert data["data"]["current"]["input_tokens"] == 100
        assert data["data"]["current"]["output_tokens"] == 50
        assert data["data"]["model"] == "claude"

    def test_events_recording(self):
        """Test event recording."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.VERBOSE)
        formatter.format_tool_call(ToolCallInfo(tool_name="Read", tool_id="1"))
        formatter.format_tool_result(ToolCallInfo(tool_name="Read", tool_id="1", result="ok"))

        events = formatter.get_events()
        assert len(events) == 2
        assert events[0]["type"] == "tool_call"
        assert events[1]["type"] == "tool_result"

    def test_clear_events(self):
        """Test clearing events."""
        formatter = JsonFormatter()
        formatter.format_assistant_message("test")
        assert len(formatter.get_events()) == 1

        formatter.clear_events()
        assert len(formatter.get_events()) == 0

    def test_get_summary(self):
        """Test event summary."""
        formatter = JsonFormatter()
        formatter.format_tool_call(ToolCallInfo(tool_name="Read", tool_id="1"))
        formatter.format_tool_call(ToolCallInfo(tool_name="Write", tool_id="2"))
        formatter.format_assistant_message("hello")

        summary = formatter.get_summary()
        assert summary["total_events"] == 3
        assert summary["event_counts"]["tool_call"] == 2
        assert summary["event_counts"]["assistant_message"] == 1

    def test_export_events(self):
        """Test exporting all events."""
        formatter = JsonFormatter()
        formatter.format_assistant_message("test")
        export = formatter.export_events()

        data = json.loads(export)
        assert "events" in data
        assert "summary" in data
        assert len(data["events"]) == 1

    def test_pretty_vs_compact(self):
        """Test pretty vs compact JSON output."""
        compact_formatter = JsonFormatter(pretty=False)
        pretty_formatter = JsonFormatter(pretty=True)

        compact = compact_formatter.format_assistant_message("test")
        pretty = pretty_formatter.format_assistant_message("test")

        # Compact should be single line, pretty should have newlines
        assert "\n" not in compact
        assert "\n" in pretty

    def test_timestamps_optional(self):
        """Test timestamps can be disabled."""
        formatter = JsonFormatter(include_timestamps=False)
        output = formatter.format_assistant_message("test")
        data = json.loads(output)

        assert "timestamp" not in data

    def test_format_tool_call_quiet(self):
        """Test tool call returns empty in quiet mode."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.QUIET)
        tool_info = ToolCallInfo(tool_name="Read", tool_id="123")
        output = formatter.format_tool_call(tool_info)
        assert output == ""

    def test_format_tool_call_with_start_time(self):
        """Test tool call includes start_time when set."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.VERBOSE)
        start = datetime.now()
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="123",
            start_time=start,
        )
        output = formatter.format_tool_call(tool_info)
        data = json.loads(output)
        assert "start_time" in data["data"]

    def test_format_tool_result_quiet(self):
        """Test tool result returns empty in quiet mode."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.QUIET)
        tool_info = ToolCallInfo(tool_name="Read", tool_id="123", result="ok")
        output = formatter.format_tool_result(tool_info)
        assert output == ""

    def test_format_tool_result_with_end_time(self):
        """Test tool result includes end_time when set."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.VERBOSE)
        end = datetime.now()
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="123",
            result="content",
            end_time=end,
        )
        output = formatter.format_tool_result(tool_info)
        data = json.loads(output)
        assert "end_time" in data["data"]

    def test_format_tool_result_truncated(self):
        """Test very long tool result gets truncated."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.VERBOSE)
        long_result = "x" * 2000
        tool_info = ToolCallInfo(
            tool_name="Read",
            tool_id="123",
            result=long_result,
        )
        output = formatter.format_tool_result(tool_info)
        data = json.loads(output)
        assert data["data"]["result_truncated"] is True
        assert data["data"]["result_full_length"] == 2000

    def test_format_assistant_quiet(self):
        """Test assistant message returns empty in quiet mode."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.QUIET)
        output = formatter.format_assistant_message("Hello")
        assert output == ""

    def test_format_system_quiet(self):
        """Test system message returns empty in quiet mode."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.QUIET)
        output = formatter.format_system_message("System")
        assert output == ""

    def test_format_progress_quiet(self):
        """Test progress returns empty in quiet mode."""
        formatter = JsonFormatter(verbosity=VerbosityLevel.QUIET)
        output = formatter.format_progress("Working", 50, 100)
        assert output == ""

    def test_format_section_header(self):
        """Test section header formatting."""
        formatter = JsonFormatter()
        output = formatter.format_section_header("Test Section", iteration=2)
        data = json.loads(output)
        assert data["type"] == "section_start"
        assert data["data"]["title"] == "Test Section"
        assert data["iteration"] == 2
        assert "elapsed_seconds" in data["data"]

    def test_format_section_footer(self):
        """Test section footer formatting."""
        formatter = JsonFormatter()
        output = formatter.format_section_footer()
        data = json.loads(output)
        assert data["type"] == "section_end"
        assert "elapsed_seconds" in data["data"]


class TestCreateFormatter:
    """Tests for create_formatter factory function."""

    def test_create_plain_formatter(self):
        """Test creating plain text formatter."""
        formatter = create_formatter("plain")
        assert isinstance(formatter, PlainTextFormatter)

    def test_create_rich_formatter(self):
        """Test creating rich terminal formatter."""
        formatter = create_formatter("rich")
        assert isinstance(formatter, RichTerminalFormatter)

    def test_create_json_formatter(self):
        """Test creating JSON formatter."""
        formatter = create_formatter("json")
        assert isinstance(formatter, JsonFormatter)

    def test_create_with_verbosity(self):
        """Test creating formatter with verbosity."""
        formatter = create_formatter("plain", verbosity=VerbosityLevel.DEBUG)
        assert formatter.verbosity == VerbosityLevel.DEBUG

    def test_create_with_aliases(self):
        """Test format type aliases."""
        assert isinstance(create_formatter("text"), PlainTextFormatter)
        assert isinstance(create_formatter("terminal"), RichTerminalFormatter)

    def test_create_case_insensitive(self):
        """Test format type is case insensitive."""
        assert isinstance(create_formatter("PLAIN"), PlainTextFormatter)
        assert isinstance(create_formatter("Rich"), RichTerminalFormatter)
        assert isinstance(create_formatter("JSON"), JsonFormatter)

    def test_create_invalid_type(self):
        """Test invalid format type raises error."""
        with pytest.raises(ValueError) as exc:
            create_formatter("invalid")
        assert "Unknown format type" in str(exc.value)


class TestShouldDisplay:
    """Tests for should_display method."""

    def test_error_always_displayed(self):
        """Test errors are always displayed."""
        for level in VerbosityLevel:
            formatter = PlainTextFormatter(verbosity=level)
            assert formatter.should_display(MessageType.ERROR) is True

    def test_quiet_hides_most(self):
        """Test quiet mode hides most message types."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.QUIET)
        assert formatter.should_display(MessageType.ASSISTANT) is False
        assert formatter.should_display(MessageType.TOOL_CALL) is False
        assert formatter.should_display(MessageType.INFO) is False

    def test_normal_shows_important(self):
        """Test normal mode shows important messages."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.NORMAL)
        assert formatter.should_display(MessageType.ASSISTANT) is True
        assert formatter.should_display(MessageType.TOOL_CALL) is True
        assert formatter.should_display(MessageType.PROGRESS) is True

    def test_verbose_shows_all(self):
        """Test verbose mode shows all messages."""
        formatter = PlainTextFormatter(verbosity=VerbosityLevel.VERBOSE)
        for msg_type in MessageType:
            assert formatter.should_display(msg_type) is True


class TestContentType:
    """Tests for ContentType enum."""

    def test_content_type_values(self):
        """Test all content types are defined."""
        assert ContentType.PLAIN_TEXT.value == "plain_text"
        assert ContentType.DIFF.value == "diff"
        assert ContentType.CODE_BLOCK.value == "code_block"
        assert ContentType.MARKDOWN.value == "markdown"
        assert ContentType.MARKDOWN_TABLE.value == "markdown_table"
        assert ContentType.ERROR_TRACEBACK.value == "error_traceback"


class TestContentDetector:
    """Tests for ContentDetector class."""

    def test_detect_empty_text(self):
        """Test empty text returns plain text."""
        detector = ContentDetector()
        assert detector.detect("") == ContentType.PLAIN_TEXT
        assert detector.detect("   ") == ContentType.PLAIN_TEXT
        assert detector.detect(None) == ContentType.PLAIN_TEXT

    def test_detect_plain_text(self):
        """Test plain text detection."""
        detector = ContentDetector()
        assert detector.detect("Hello world") == ContentType.PLAIN_TEXT
        assert detector.detect("Just a simple message.") == ContentType.PLAIN_TEXT

    def test_detect_diff_git_header(self):
        """Test diff detection with git header."""
        detector = ContentDetector()
        diff_text = """diff --git a/file.py b/file.py
index 123..456 789
--- a/file.py
+++ b/file.py
@@ -1,3 +1,4 @@
+added line
 context"""
        assert detector.detect(diff_text) == ContentType.DIFF

    def test_detect_diff_hunk_markers(self):
        """Test diff detection with hunk markers."""
        detector = ContentDetector()
        diff_text = """@@ -1,3 +1,4 @@
+added line
-removed line
 context line"""
        assert detector.detect(diff_text) == ContentType.DIFF

    def test_detect_diff_with_hunk_and_changes(self):
        """Test diff detection with hunk and +/- lines."""
        detector = ContentDetector()
        # Real diff content needs @@ markers or diff --git header
        diff_text = """@@ -1,2 +1,3 @@
+added
-removed
+another added"""
        assert detector.detect(diff_text) == ContentType.DIFF

    def test_is_diff_methods(self):
        """Test is_diff individual method."""
        detector = ContentDetector()
        assert detector.is_diff("diff --git a/x b/x") is True
        assert detector.is_diff("@@ -1,3 +1,4 @@\n+added") is True
        # File markers alone need both --- a/ and +++ b/ patterns
        assert detector.is_diff("--- a/file\n+++ b/file") is True
        assert detector.is_diff("just text") is False
        assert detector.is_diff("") is False
        assert detector.is_diff(None) is False
        # Markdown-like content should NOT be detected as diff
        assert detector.is_diff("- list item") is False
        assert detector.is_diff("---") is False  # Markdown hr

    def test_detect_code_block(self):
        """Test code block detection."""
        detector = ContentDetector()
        code_text = """Here is some code:

```python
print("hello")
```

That's all."""
        assert detector.detect(code_text) == ContentType.CODE_BLOCK

    def test_detect_code_block_no_language(self):
        """Test code block without language specifier."""
        detector = ContentDetector()
        code_text = """```
some code
```"""
        assert detector.detect(code_text) == ContentType.CODE_BLOCK

    def test_is_code_block_method(self):
        """Test is_code_block individual method."""
        detector = ContentDetector()
        assert detector.is_code_block("```python\ncode\n```") is True
        assert detector.is_code_block("just text") is False
        assert detector.is_code_block("") is False
        assert detector.is_code_block(None) is False

    def test_detect_markdown_heading(self):
        """Test markdown detection with heading and list."""
        detector = ContentDetector()
        md_text = """# Title

- item 1
- item 2"""
        assert detector.detect(md_text) == ContentType.MARKDOWN

    def test_detect_markdown_bold_and_list(self):
        """Test markdown detection with emphasis and list."""
        detector = ContentDetector()
        md_text = """Here is **bold text** and more content.

1. First item
2. Second item"""
        assert detector.detect(md_text) == ContentType.MARKDOWN

    def test_detect_markdown_requires_threshold(self):
        """Test markdown requires multiple indicators."""
        detector = ContentDetector()
        # Single indicator should not trigger markdown
        assert detector.detect("# Just a heading") == ContentType.PLAIN_TEXT
        assert detector.detect("- single item") == ContentType.PLAIN_TEXT
        assert detector.detect("**bold only**") == ContentType.PLAIN_TEXT

    def test_is_markdown_method(self):
        """Test is_markdown individual method."""
        detector = ContentDetector()
        assert detector.is_markdown("# Title\n\n- list item") is True
        assert detector.is_markdown("plain text") is False
        assert detector.is_markdown("") is False
        assert detector.is_markdown(None) is False

    def test_detect_markdown_table(self):
        """Test markdown table detection."""
        detector = ContentDetector()
        table_text = """| Column A | Column B |
|----------|----------|
| Value 1  | Value 2  |"""
        assert detector.detect(table_text) == ContentType.MARKDOWN_TABLE

    def test_is_markdown_table_method(self):
        """Test is_markdown_table individual method."""
        detector = ContentDetector()
        assert detector.is_markdown_table("| A |\n|---|\n| 1 |") is True
        assert detector.is_markdown_table("just text") is False
        assert detector.is_markdown_table("| not a table") is False
        assert detector.is_markdown_table("") is False
        assert detector.is_markdown_table(None) is False

    def test_detect_error_traceback(self):
        """Test error traceback detection."""
        detector = ContentDetector()
        traceback_text = """Traceback (most recent call last):
  File "test.py", line 10, in <module>
    raise ValueError("test")
ValueError: test"""
        assert detector.detect(traceback_text) == ContentType.ERROR_TRACEBACK

    def test_detect_error_traceback_file_line(self):
        """Test error traceback detection with File line."""
        detector = ContentDetector()
        traceback_text = '''  File "/path/to/file.py", line 42, in function
    some_call()'''
        assert detector.detect(traceback_text) == ContentType.ERROR_TRACEBACK

    def test_is_error_traceback_method(self):
        """Test is_error_traceback individual method."""
        detector = ContentDetector()
        assert detector.is_error_traceback("Traceback (most recent call last):") is True
        assert detector.is_error_traceback('  File "x.py", line 1') is True
        assert detector.is_error_traceback("ValueError: test") is True
        assert detector.is_error_traceback("just text") is False
        assert detector.is_error_traceback("") is False
        assert detector.is_error_traceback(None) is False

    def test_detect_priority_code_block_over_diff(self):
        """Test code block takes priority over diff-like content."""
        detector = ContentDetector()
        # Code block with diff-like content inside
        text = """```diff
+added
-removed
```"""
        assert detector.detect(text) == ContentType.CODE_BLOCK

    def test_detect_priority_code_block_over_markdown(self):
        """Test code block takes priority over markdown."""
        detector = ContentDetector()
        text = """# Heading

```python
def foo():
    pass
```

- list item"""
        assert detector.detect(text) == ContentType.CODE_BLOCK

    def test_extract_code_blocks(self):
        """Test extracting code blocks from text."""
        detector = ContentDetector()
        text = """Some text

```python
print("hello")
```

More text

```javascript
console.log("hi")
```"""
        blocks = detector.extract_code_blocks(text)
        assert len(blocks) == 2
        assert blocks[0] == ("python", 'print("hello")')
        assert blocks[1] == ("javascript", 'console.log("hi")')

    def test_extract_code_blocks_no_language(self):
        """Test extracting code blocks without language."""
        detector = ContentDetector()
        text = """```
plain code
```"""
        blocks = detector.extract_code_blocks(text)
        assert len(blocks) == 1
        assert blocks[0] == (None, "plain code")

    def test_extract_code_blocks_empty(self):
        """Test extracting from text with no code blocks."""
        detector = ContentDetector()
        assert detector.extract_code_blocks("no code here") == []
        assert detector.extract_code_blocks("") == []
        assert detector.extract_code_blocks(None) == []

    def test_markdown_task_list(self):
        """Test markdown detection with task lists."""
        detector = ContentDetector()
        text = """# Todo

- [ ] Task 1
- [x] Task 2"""
        assert detector.detect(text) == ContentType.MARKDOWN

    def test_markdown_blockquote(self):
        """Test markdown detection with blockquotes."""
        detector = ContentDetector()
        text = """> Quote line 1

Some text

> Another quote"""
        # Need 2+ indicators - blockquotes count as one
        assert detector.is_markdown(text) is False  # Only one type of indicator

    def test_markdown_horizontal_rule(self):
        """Test markdown detection with horizontal rule."""
        detector = ContentDetector()
        text = """# Title

---

Some content"""
        assert detector.detect(text) == ContentType.MARKDOWN


class TestRichTerminalFormatterSmartDetection:
    """Tests for RichTerminalFormatter smart content detection."""

    def test_smart_detection_enabled_by_default(self):
        """Test smart detection is enabled by default."""
        formatter = RichTerminalFormatter()
        assert formatter._smart_detection is True
        assert formatter._content_detector is not None

    def test_smart_detection_can_be_disabled(self):
        """Test smart detection can be disabled."""
        formatter = RichTerminalFormatter(smart_detection=False)
        assert formatter._smart_detection is False
        assert formatter._content_detector is None

    def test_format_assistant_plain_text(self):
        """Test plain text formatting with smart detection."""
        formatter = RichTerminalFormatter()
        output = formatter.format_assistant_message("Hello world")
        assert "Hello world" in output

    def test_format_assistant_diff_detected(self):
        """Test diff content is detected and rendered specially."""
        formatter = RichTerminalFormatter()
        diff_text = """diff --git a/file.py b/file.py
--- a/file.py
+++ b/file.py
@@ -1,3 +1,4 @@
+added line
 context"""
        # Diff rendering prints directly, returns empty string
        formatter.format_assistant_message(diff_text)
        # Output may be empty since diff prints directly, or contain diff text
        # The important thing is it doesn't crash

    def test_format_assistant_code_block_detected(self):
        """Test code block content is detected and rendered with syntax."""
        formatter = RichTerminalFormatter()
        code_text = """Here is some code:

```python
print("hello")
```

That's all."""
        output = formatter.format_assistant_message(code_text)
        # Should contain the code content (rendered with syntax highlighting)
        assert "print" in output or "hello" in output

    def test_format_assistant_markdown_detected(self):
        """Test markdown content is detected and rendered."""
        formatter = RichTerminalFormatter()
        md_text = """# Title

- item 1
- item 2

**Bold text** here."""
        output = formatter.format_assistant_message(md_text)
        # Should contain the content (rendered with markdown)
        assert "Title" in output or "item" in output

    def test_format_assistant_traceback_detected(self):
        """Test error traceback is detected and rendered specially."""
        formatter = RichTerminalFormatter()
        traceback_text = """Traceback (most recent call last):
  File "test.py", line 10, in <module>
    raise ValueError("test")
ValueError: test"""
        output = formatter.format_assistant_message(traceback_text)
        # Should contain traceback content with special formatting
        assert "Traceback" in output or "ValueError" in output

    def test_format_assistant_table_detected(self):
        """Test markdown table is detected and rendered."""
        formatter = RichTerminalFormatter()
        table_text = """| Column A | Column B |
|----------|----------|
| Value 1  | Value 2  |"""
        output = formatter.format_assistant_message(table_text)
        # Should contain table content
        assert "Column" in output or "Value" in output

    def test_format_assistant_respects_quiet_mode(self):
        """Test quiet mode still returns empty."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.QUIET)
        output = formatter.format_assistant_message("Hello world")
        assert output == ""

    def test_format_assistant_without_smart_detection(self):
        """Test formatting without smart detection uses simple format."""
        formatter = RichTerminalFormatter(smart_detection=False)
        output = formatter.format_assistant_message("Hello world")
        # Should still format but without smart detection
        assert "Hello world" in output

    def test_print_smart_method(self):
        """Test print_smart method exists and works."""
        formatter = RichTerminalFormatter()
        # Should not raise
        formatter.print_smart("Hello world")

    def test_print_smart_quiet_mode(self):
        """Test print_smart respects quiet mode."""
        formatter = RichTerminalFormatter(verbosity=VerbosityLevel.QUIET)
        # Should not raise and should do nothing
        formatter.print_smart("Hello world")

    def test_render_smart_content_plain(self):
        """Test _render_smart_content with plain text."""
        formatter = RichTerminalFormatter()
        output = formatter._render_smart_content("plain text", ContentType.PLAIN_TEXT)
        assert "plain text" in output

    def test_preprocess_markdown_task_lists(self):
        """Test markdown preprocessing converts task lists."""
        formatter = RichTerminalFormatter()
        text = "- [ ] unchecked\n- [x] checked"
        processed = formatter._preprocess_markdown(text)
        assert "☐" in processed
        assert "☑" in processed
        assert "[ ]" not in processed
        assert "[x]" not in processed
