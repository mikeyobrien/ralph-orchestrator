# ABOUTME: Base classes and interfaces for output formatting
# ABOUTME: Defines OutputFormatter ABC with verbosity levels, event types, and token tracking

"""Base classes for Claude adapter output formatting."""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Any, Callable, Dict, List, Optional


class VerbosityLevel(Enum):
    """Verbosity levels for output formatting."""

    QUIET = 0  # Only errors and final results
    NORMAL = 1  # Tool calls, assistant messages (no details)
    VERBOSE = 2  # Full tool inputs/outputs, detailed messages
    DEBUG = 3  # Everything including internal state


class MessageType(Enum):
    """Types of messages that can be formatted."""

    SYSTEM = "system"
    ASSISTANT = "assistant"
    USER = "user"
    TOOL_CALL = "tool_call"
    TOOL_RESULT = "tool_result"
    ERROR = "error"
    INFO = "info"
    PROGRESS = "progress"


@dataclass
class TokenUsage:
    """Tracks token usage and costs."""

    input_tokens: int = 0
    output_tokens: int = 0
    total_tokens: int = 0
    cost: float = 0.0
    model: str = ""

    # Running totals across session
    session_input_tokens: int = 0
    session_output_tokens: int = 0
    session_total_tokens: int = 0
    session_cost: float = 0.0

    def add(
        self,
        input_tokens: int = 0,
        output_tokens: int = 0,
        cost: float = 0.0,
        model: str = "",
    ) -> None:
        """Add tokens to current and session totals."""
        self.input_tokens = input_tokens
        self.output_tokens = output_tokens
        self.total_tokens = input_tokens + output_tokens
        self.cost = cost
        if model:
            self.model = model

        self.session_input_tokens += input_tokens
        self.session_output_tokens += output_tokens
        self.session_total_tokens += input_tokens + output_tokens
        self.session_cost += cost

    def reset_current(self) -> None:
        """Reset current iteration tokens (keep session totals)."""
        self.input_tokens = 0
        self.output_tokens = 0
        self.total_tokens = 0
        self.cost = 0.0


@dataclass
class ToolCallInfo:
    """Information about a tool call."""

    tool_name: str
    tool_id: str
    input_params: Dict[str, Any] = field(default_factory=dict)
    start_time: Optional[datetime] = None
    end_time: Optional[datetime] = None
    result: Optional[Any] = None
    is_error: bool = False
    duration_ms: Optional[int] = None


@dataclass
class FormatContext:
    """Context information for formatting operations."""

    iteration: int = 0
    verbosity: VerbosityLevel = VerbosityLevel.NORMAL
    timestamp: Optional[datetime] = None
    token_usage: Optional[TokenUsage] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.timestamp is None:
            self.timestamp = datetime.now()
        if self.token_usage is None:
            self.token_usage = TokenUsage()


class OutputFormatter(ABC):
    """Abstract base class for output formatters.

    Formatters handle rendering of Claude adapter events to different output
    formats (plain text, rich terminal, JSON, etc.). They support verbosity
    levels and consistent token usage tracking.
    """

    def __init__(self, verbosity: VerbosityLevel = VerbosityLevel.NORMAL) -> None:
        """Initialize formatter with verbosity level.

        Args:
            verbosity: Output verbosity level
        """
        self._verbosity = verbosity
        self._token_usage = TokenUsage()
        self._start_time = datetime.now()
        self._callbacks: List[Callable[[MessageType, Any, FormatContext], None]] = []

    @property
    def verbosity(self) -> VerbosityLevel:
        """Get current verbosity level."""
        return self._verbosity

    @verbosity.setter
    def verbosity(self, level: VerbosityLevel) -> None:
        """Set verbosity level."""
        self._verbosity = level

    @property
    def token_usage(self) -> TokenUsage:
        """Get current token usage."""
        return self._token_usage

    def should_display(self, message_type: MessageType) -> bool:
        """Check if message type should be displayed at current verbosity.

        Args:
            message_type: Type of message to check

        Returns:
            True if message should be displayed
        """
        # Always show errors
        if message_type == MessageType.ERROR:
            return True

        if self._verbosity == VerbosityLevel.QUIET:
            return False

        if self._verbosity == VerbosityLevel.NORMAL:
            return message_type in (
                MessageType.ASSISTANT,
                MessageType.TOOL_CALL,
                MessageType.PROGRESS,
                MessageType.INFO,
            )

        # VERBOSE and DEBUG show everything
        return True

    def register_callback(
        self, callback: Callable[[MessageType, Any, FormatContext], None]
    ) -> None:
        """Register a callback for format events.

        Args:
            callback: Function to call with (message_type, content, context)
        """
        self._callbacks.append(callback)

    def _notify_callbacks(
        self, message_type: MessageType, content: Any, context: FormatContext
    ) -> None:
        """Notify all registered callbacks."""
        for callback in self._callbacks:
            try:
                callback(message_type, content, context)
            except Exception:
                pass  # Don't let callback errors break formatting

    def _create_context(
        self, iteration: int = 0, metadata: Optional[Dict[str, Any]] = None
    ) -> FormatContext:
        """Create a format context with current state.

        Args:
            iteration: Current iteration number
            metadata: Additional metadata

        Returns:
            FormatContext instance
        """
        return FormatContext(
            iteration=iteration,
            verbosity=self._verbosity,
            timestamp=datetime.now(),
            token_usage=self._token_usage,
            metadata=metadata or {},
        )

    def update_tokens(
        self,
        input_tokens: int = 0,
        output_tokens: int = 0,
        cost: float = 0.0,
        model: str = "",
    ) -> None:
        """Update token usage tracking.

        Args:
            input_tokens: Number of input tokens used
            output_tokens: Number of output tokens used
            cost: Cost in USD
            model: Model name
        """
        self._token_usage.add(input_tokens, output_tokens, cost, model)

    @abstractmethod
    def format_tool_call(
        self,
        tool_info: ToolCallInfo,
        iteration: int = 0,
    ) -> str:
        """Format a tool call for display.

        Args:
            tool_info: Tool call information
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        pass

    @abstractmethod
    def format_tool_result(
        self,
        tool_info: ToolCallInfo,
        iteration: int = 0,
    ) -> str:
        """Format a tool result for display.

        Args:
            tool_info: Tool call info with result
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        pass

    @abstractmethod
    def format_assistant_message(
        self,
        message: str,
        iteration: int = 0,
    ) -> str:
        """Format an assistant message for display.

        Args:
            message: Assistant message text
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        pass

    @abstractmethod
    def format_system_message(
        self,
        message: str,
        iteration: int = 0,
    ) -> str:
        """Format a system message for display.

        Args:
            message: System message text
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        pass

    @abstractmethod
    def format_error(
        self,
        error: str,
        exception: Optional[Exception] = None,
        iteration: int = 0,
    ) -> str:
        """Format an error for display.

        Args:
            error: Error message
            exception: Optional exception object
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        pass

    @abstractmethod
    def format_progress(
        self,
        message: str,
        current: int = 0,
        total: int = 0,
        iteration: int = 0,
    ) -> str:
        """Format progress information for display.

        Args:
            message: Progress message
            current: Current progress value
            total: Total progress value
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        pass

    @abstractmethod
    def format_token_usage(self, show_session: bool = True) -> str:
        """Format token usage summary for display.

        Args:
            show_session: Include session totals

        Returns:
            Formatted string representation
        """
        pass

    @abstractmethod
    def format_section_header(self, title: str, iteration: int = 0) -> str:
        """Format a section header for display.

        Args:
            title: Section title
            iteration: Current iteration number

        Returns:
            Formatted string representation
        """
        pass

    @abstractmethod
    def format_section_footer(self) -> str:
        """Format a section footer for display.

        Returns:
            Formatted string representation
        """
        pass

    def summarize_content(self, content: str, max_length: int = 500) -> str:
        """Summarize long content for display.

        Args:
            content: Content to summarize
            max_length: Maximum length before truncation

        Returns:
            Summarized content
        """
        if len(content) <= max_length:
            return content

        # Truncate with indicator
        half = (max_length - 20) // 2
        return f"{content[:half]}\n... [{len(content)} chars truncated] ...\n{content[-half:]}"

    def get_elapsed_time(self) -> float:
        """Get elapsed time since formatter creation.

        Returns:
            Elapsed time in seconds
        """
        return (datetime.now() - self._start_time).total_seconds()
