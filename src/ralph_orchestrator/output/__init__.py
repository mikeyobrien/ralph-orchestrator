# ABOUTME: Output formatter module initialization
# ABOUTME: Exports base classes, formatter implementations, and legacy console classes

"""Output formatting module for Claude adapter responses.

This module provides:

1. Legacy output utilities (backward compatible):
   - DiffStats, DiffFormatter, RalphConsole - Rich terminal utilities
   - RICH_AVAILABLE - Rich library availability flag

2. New formatter classes for structured output:
   - PlainTextFormatter: Basic text output without colors
   - RichTerminalFormatter: Rich terminal output with colors and panels
   - JsonFormatter: Structured JSON output for programmatic consumption

Example usage (new formatters):
    from ralph_orchestrator.output import (
        RichTerminalFormatter,
        VerbosityLevel,
        ToolCallInfo,
    )

    formatter = RichTerminalFormatter(verbosity=VerbosityLevel.VERBOSE)
    tool_info = ToolCallInfo(tool_name="Read", tool_id="abc123", input_params={"path": "test.py"})
    output = formatter.format_tool_call(tool_info, iteration=1)
    formatter.print(output)

Example usage (legacy console):
    from ralph_orchestrator.output import RalphConsole

    console = RalphConsole()
    console.print_status("Processing...")
    console.print_success("Done!")
"""

# Import legacy classes from console module (backward compatibility)
from .console import (
    RICH_AVAILABLE,
    DiffFormatter,
    DiffStats,
    RalphConsole,
)

# Import new formatter base classes
from .base import (
    FormatContext,
    MessageType,
    OutputFormatter,
    TokenUsage,
    ToolCallInfo,
    VerbosityLevel,
)

# Import new formatter implementations
from .json_formatter import JsonFormatter
from .plain import PlainTextFormatter
from .rich_formatter import RichTerminalFormatter

__all__ = [
    # Legacy exports (backward compatibility)
    "RICH_AVAILABLE",
    "DiffStats",
    "DiffFormatter",
    "RalphConsole",
    # New base classes
    "OutputFormatter",
    "VerbosityLevel",
    "MessageType",
    "TokenUsage",
    "ToolCallInfo",
    "FormatContext",
    # New formatters
    "PlainTextFormatter",
    "RichTerminalFormatter",
    "JsonFormatter",
    # Factory function
    "create_formatter",
]


def create_formatter(
    format_type: str = "rich",
    verbosity: VerbosityLevel = VerbosityLevel.NORMAL,
    **kwargs,
) -> OutputFormatter:
    """Factory function to create appropriate formatter.

    Args:
        format_type: Type of formatter ("plain", "rich", "json")
        verbosity: Verbosity level for output
        **kwargs: Additional arguments passed to formatter constructor

    Returns:
        Configured OutputFormatter instance

    Raises:
        ValueError: If format_type is not recognized
    """
    formatters = {
        "plain": PlainTextFormatter,
        "text": PlainTextFormatter,
        "rich": RichTerminalFormatter,
        "terminal": RichTerminalFormatter,
        "json": JsonFormatter,
    }

    if format_type.lower() not in formatters:
        raise ValueError(
            f"Unknown format type: {format_type}. "
            f"Valid options: {', '.join(formatters.keys())}"
        )

    formatter_class = formatters[format_type.lower()]
    return formatter_class(verbosity=verbosity, **kwargs)
