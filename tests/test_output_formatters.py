# ABOUTME: Tests for the new output formatter module
# ABOUTME: Tests VerbosityLevel, OutputFormatter base class, and all formatter implementations

"""Tests for the new output formatter module."""

import json
from datetime import datetime
from unittest.mock import Mock, patch

import pytest

from ralph_orchestrator.output import (
    FormatContext,
    JsonFormatter,
    MessageType,
    OutputFormatter,
    PlainTextFormatter,
    RichTerminalFormatter,
    TokenUsage,
    ToolCallInfo,
    VerbosityLevel,
    create_formatter,
)


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
        formatter = RichTerminalFormatter()
        # Console may or may not be available depending on Rich
        console = formatter.console
        # Just verify the property works


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
