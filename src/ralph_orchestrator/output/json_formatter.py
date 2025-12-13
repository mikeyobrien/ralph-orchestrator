# ABOUTME: JSON output formatter for programmatic consumption
# ABOUTME: Produces structured JSON output for parsing by other tools

"""JSON output formatter for Claude adapter."""

import json
from datetime import datetime
from typing import Any, Dict, List, Optional

from .base import (
    MessageType,
    OutputFormatter,
    ToolCallInfo,
    VerbosityLevel,
)


class JsonFormatter(OutputFormatter):
    """JSON formatter for programmatic output consumption.

    Produces structured JSON output suitable for parsing by other tools,
    logging systems, or downstream processing pipelines.
    """

    def __init__(
        self,
        verbosity: VerbosityLevel = VerbosityLevel.NORMAL,
        pretty: bool = True,
        include_timestamps: bool = True,
    ) -> None:
        """Initialize JSON formatter.

        Args:
            verbosity: Output verbosity level
            pretty: Pretty-print JSON with indentation
            include_timestamps: Include timestamps in output
        """
        super().__init__(verbosity)
        self._pretty = pretty
        self._include_timestamps = include_timestamps
        self._events: List[Dict[str, Any]] = []

    def _to_json(self, obj: Dict[str, Any]) -> str:
        """Convert object to JSON string.

        Args:
            obj: Dictionary to serialize

        Returns:
            JSON string
        """
        if self._pretty:
            return json.dumps(obj, indent=2, default=str, ensure_ascii=False)
        return json.dumps(obj, default=str, ensure_ascii=False)

    def _create_event(
        self,
        event_type: str,
        data: Dict[str, Any],
        iteration: int = 0,
    ) -> Dict[str, Any]:
        """Create a structured event object.

        Args:
            event_type: Type of event
            data: Event data
            iteration: Current iteration number

        Returns:
            Event dictionary
        """
        event = {
            "type": event_type,
            "iteration": iteration,
            "data": data,
        }

        if self._include_timestamps:
            event["timestamp"] = datetime.now().isoformat()

        return event

    def _record_event(self, event: Dict[str, Any]) -> None:
        """Record event for later retrieval.

        Args:
            event: Event dictionary to record
        """
        self._events.append(event)

    def get_events(self) -> List[Dict[str, Any]]:
        """Get all recorded events.

        Returns:
            List of event dictionaries
        """
        return self._events.copy()

    def clear_events(self) -> None:
        """Clear recorded events."""
        self._events.clear()

    def format_tool_call(
        self,
        tool_info: ToolCallInfo,
        iteration: int = 0,
    ) -> str:
        """Format a tool call as JSON.

        Args:
            tool_info: Tool call information
            iteration: Current iteration number

        Returns:
            JSON string representation
        """
        if not self.should_display(MessageType.TOOL_CALL):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.TOOL_CALL, tool_info, context)

        data: Dict[str, Any] = {
            "tool_name": tool_info.tool_name,
            "tool_id": tool_info.tool_id,
        }

        if self._verbosity.value >= VerbosityLevel.VERBOSE.value:
            data["input_params"] = tool_info.input_params

        if tool_info.start_time:
            data["start_time"] = tool_info.start_time.isoformat()

        event = self._create_event("tool_call", data, iteration)
        self._record_event(event)
        return self._to_json(event)

    def format_tool_result(
        self,
        tool_info: ToolCallInfo,
        iteration: int = 0,
    ) -> str:
        """Format a tool result as JSON.

        Args:
            tool_info: Tool call info with result
            iteration: Current iteration number

        Returns:
            JSON string representation
        """
        if not self.should_display(MessageType.TOOL_RESULT):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.TOOL_RESULT, tool_info, context)

        data: Dict[str, Any] = {
            "tool_name": tool_info.tool_name,
            "tool_id": tool_info.tool_id,
            "is_error": tool_info.is_error,
        }

        if tool_info.duration_ms is not None:
            data["duration_ms"] = tool_info.duration_ms

        if self._verbosity.value >= VerbosityLevel.VERBOSE.value:
            result = tool_info.result
            if isinstance(result, str) and len(result) > 1000:
                data["result"] = self.summarize_content(result, 1000)
                data["result_truncated"] = True
                data["result_full_length"] = len(result)
            else:
                data["result"] = result

        if tool_info.end_time:
            data["end_time"] = tool_info.end_time.isoformat()

        event = self._create_event("tool_result", data, iteration)
        self._record_event(event)
        return self._to_json(event)

    def format_assistant_message(
        self,
        message: str,
        iteration: int = 0,
    ) -> str:
        """Format an assistant message as JSON.

        Args:
            message: Assistant message text
            iteration: Current iteration number

        Returns:
            JSON string representation
        """
        if not self.should_display(MessageType.ASSISTANT):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.ASSISTANT, message, context)

        data: Dict[str, Any] = {}

        if self._verbosity == VerbosityLevel.NORMAL and len(message) > 1000:
            data["message"] = self.summarize_content(message, 1000)
            data["message_truncated"] = True
            data["message_full_length"] = len(message)
        else:
            data["message"] = message
            data["message_truncated"] = False

        data["message_length"] = len(message)

        event = self._create_event("assistant_message", data, iteration)
        self._record_event(event)
        return self._to_json(event)

    def format_system_message(
        self,
        message: str,
        iteration: int = 0,
    ) -> str:
        """Format a system message as JSON.

        Args:
            message: System message text
            iteration: Current iteration number

        Returns:
            JSON string representation
        """
        if not self.should_display(MessageType.SYSTEM):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.SYSTEM, message, context)

        data = {
            "message": message,
        }

        event = self._create_event("system_message", data, iteration)
        self._record_event(event)
        return self._to_json(event)

    def format_error(
        self,
        error: str,
        exception: Optional[Exception] = None,
        iteration: int = 0,
    ) -> str:
        """Format an error as JSON.

        Args:
            error: Error message
            exception: Optional exception object
            iteration: Current iteration number

        Returns:
            JSON string representation
        """
        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.ERROR, error, context)

        data: Dict[str, Any] = {
            "error": error,
        }

        if exception:
            data["exception_type"] = type(exception).__name__
            data["exception_str"] = str(exception)

            if self._verbosity.value >= VerbosityLevel.VERBOSE.value:
                import traceback

                data["traceback"] = traceback.format_exception(
                    type(exception), exception, exception.__traceback__
                )

        event = self._create_event("error", data, iteration)
        self._record_event(event)
        return self._to_json(event)

    def format_progress(
        self,
        message: str,
        current: int = 0,
        total: int = 0,
        iteration: int = 0,
    ) -> str:
        """Format progress information as JSON.

        Args:
            message: Progress message
            current: Current progress value
            total: Total progress value
            iteration: Current iteration number

        Returns:
            JSON string representation
        """
        if not self.should_display(MessageType.PROGRESS):
            return ""

        context = self._create_context(iteration)
        self._notify_callbacks(MessageType.PROGRESS, message, context)

        data: Dict[str, Any] = {
            "message": message,
            "current": current,
            "total": total,
        }

        if total > 0:
            data["percentage"] = round((current / total) * 100, 1)

        event = self._create_event("progress", data, iteration)
        self._record_event(event)
        return self._to_json(event)

    def format_token_usage(self, show_session: bool = True) -> str:
        """Format token usage summary as JSON.

        Args:
            show_session: Include session totals

        Returns:
            JSON string representation
        """
        usage = self._token_usage

        data: Dict[str, Any] = {
            "current": {
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
                "total_tokens": usage.total_tokens,
                "cost": usage.cost,
            },
        }

        if show_session:
            data["session"] = {
                "input_tokens": usage.session_input_tokens,
                "output_tokens": usage.session_output_tokens,
                "total_tokens": usage.session_total_tokens,
                "cost": usage.session_cost,
            }

        if usage.model:
            data["model"] = usage.model

        event = self._create_event("token_usage", data, 0)
        return self._to_json(event)

    def format_section_header(self, title: str, iteration: int = 0) -> str:
        """Format a section header as JSON.

        Args:
            title: Section title
            iteration: Current iteration number

        Returns:
            JSON string representation
        """
        data = {
            "title": title,
            "elapsed_seconds": self.get_elapsed_time(),
        }

        event = self._create_event("section_start", data, iteration)
        self._record_event(event)
        return self._to_json(event)

    def format_section_footer(self) -> str:
        """Format a section footer as JSON.

        Returns:
            JSON string representation
        """
        data = {
            "elapsed_seconds": self.get_elapsed_time(),
        }

        event = self._create_event("section_end", data, 0)
        self._record_event(event)
        return self._to_json(event)

    def get_summary(self) -> Dict[str, Any]:
        """Get a summary of all recorded events.

        Returns:
            Summary dictionary with counts and totals
        """
        event_counts: Dict[str, int] = {}
        for event in self._events:
            event_type = event.get("type", "unknown")
            event_counts[event_type] = event_counts.get(event_type, 0) + 1

        return {
            "total_events": len(self._events),
            "event_counts": event_counts,
            "token_usage": {
                "total_tokens": self._token_usage.session_total_tokens,
                "total_cost": self._token_usage.session_cost,
            },
            "elapsed_seconds": self.get_elapsed_time(),
        }

    def export_events(self) -> str:
        """Export all recorded events as a JSON array.

        Returns:
            JSON string with all events
        """
        return self._to_json({"events": self._events, "summary": self.get_summary()})
