# ABOUTME: Error formatter for Ralph Orchestrator
# ABOUTME: Provides structured error messages with user-friendly suggestions

"""
Error formatter for Ralph Orchestrator.

This module provides structured error messages with user-friendly suggestions
and security-aware error sanitization for Claude SDK and adapter errors.
"""

from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass


@dataclass
class ErrorMessage:
    """Formatted error message with main message and suggestion.

    Attributes:
        message: The main error message (user-facing)
        suggestion: A helpful suggestion for resolving the error
    """
    message: str
    suggestion: str

    def __str__(self) -> str:
        """Return combined error and suggestion."""
        return f"{self.message} | {self.suggestion}"


class ClaudeErrorFormatter:
    """Formats error messages with user-friendly suggestions.

    This class provides static methods to format various error types
    encountered during Claude SDK operations into structured messages
    with helpful suggestions for resolution.

    All methods use security-aware sanitization to prevent information
    disclosure of sensitive data in error messages.
    """

    @staticmethod
    def format_timeout_error(iteration: int, timeout: int) -> ErrorMessage:
        """Format timeout error message.

        Args:
            iteration: Current iteration number
            timeout: Timeout limit in seconds

        Returns:
            Formatted error message with suggestion
        """
        return ErrorMessage(
            message=f"Iteration {iteration} exceeded timeout limit of {timeout}s",
            suggestion="Try: Increase iteration_timeout in config or simplify your prompt"
        )

    @staticmethod
    def format_process_terminated_error(iteration: int) -> ErrorMessage:
        """Format process termination error message.

        Args:
            iteration: Current iteration number

        Returns:
            Formatted error message with suggestion
        """
        return ErrorMessage(
            message=f"Iteration {iteration} failed: Claude subprocess terminated unexpectedly",
            suggestion="Try: Check if Claude Code CLI is properly installed and has correct permissions"
        )

    @staticmethod
    def format_interrupted_error(iteration: int) -> ErrorMessage:
        """Format interrupted error message (SIGTERM received).

        Args:
            iteration: Current iteration number

        Returns:
            Formatted error message with suggestion
        """
        return ErrorMessage(
            message=f"Iteration {iteration} was interrupted (SIGTERM)",
            suggestion="This usually happens when stopping Ralph - no action needed"
        )

    @staticmethod
    def format_connection_error(iteration: int) -> ErrorMessage:
        """Format connection error message.

        Args:
            iteration: Current iteration number

        Returns:
            Formatted error message with suggestion
        """
        return ErrorMessage(
            message=f"Iteration {iteration} failed: Cannot connect to Claude CLI",
            suggestion="Try: Verify Claude Code CLI is installed: claude --version"
        )

    @staticmethod
    def format_rate_limit_error(iteration: int, retry_after: int = 60) -> ErrorMessage:
        """Format rate limit error message.

        Args:
            iteration: Current iteration number
            retry_after: Seconds until retry is recommended

        Returns:
            Formatted error message with suggestion
        """
        return ErrorMessage(
            message=f"Iteration {iteration} failed: Rate limit exceeded",
            suggestion=f"Try: Wait {retry_after}s before retrying or reduce request frequency"
        )

    @staticmethod
    def format_authentication_error(iteration: int) -> ErrorMessage:
        """Format authentication error message.

        Args:
            iteration: Current iteration number

        Returns:
            Formatted error message with suggestion
        """
        return ErrorMessage(
            message=f"Iteration {iteration} failed: Authentication error",
            suggestion="Try: Check your API credentials or re-authenticate with 'claude login'"
        )

    @staticmethod
    def format_permission_error(iteration: int, path: str = "") -> ErrorMessage:
        """Format permission error message.

        Args:
            iteration: Current iteration number
            path: Optional path that caused the permission error

        Returns:
            Formatted error message with suggestion
        """
        # Sanitize path to avoid information disclosure
        safe_path = path if path and len(path) < 100 else ""
        if safe_path:
            return ErrorMessage(
                message=f"Iteration {iteration} failed: Permission denied for '{safe_path}'",
                suggestion="Try: Check file permissions or run with appropriate privileges"
            )
        return ErrorMessage(
            message=f"Iteration {iteration} failed: Permission denied",
            suggestion="Try: Check file/directory permissions or run with appropriate privileges"
        )

    @staticmethod
    def format_generic_error(iteration: int, error_type: str, error_str: str) -> ErrorMessage:
        """Format generic error message with security sanitization.

        Args:
            iteration: Current iteration number
            error_type: Type of the exception (e.g., 'ValueError')
            error_str: String representation of the error

        Returns:
            Formatted error message with sanitized content
        """
        # Import here to avoid circular imports
        from .security import SecurityValidator

        # Sanitize error string to prevent information disclosure
        sanitized_error_str = SecurityValidator.mask_sensitive_data(error_str)

        # Truncate very long error messages
        if len(sanitized_error_str) > 200:
            sanitized_error_str = sanitized_error_str[:197] + "..."

        return ErrorMessage(
            message=f"Iteration {iteration} failed: {error_type}: {sanitized_error_str}",
            suggestion="Check logs for details or try reducing prompt complexity"
        )

    @staticmethod
    def format_error_from_exception(iteration: int, exception: Exception) -> ErrorMessage:
        """Format error message from exception.

        Analyzes the exception type and message to provide the most
        appropriate error format with helpful suggestions.

        Args:
            iteration: Current iteration number
            exception: The exception that occurred

        Returns:
            Formatted error message with appropriate suggestion
        """
        error_type = type(exception).__name__
        error_str = str(exception)

        # Match error patterns and provide specific suggestions

        # Process transport issues (subprocess terminated)
        if "ProcessTransport is not ready" in error_str:
            return ClaudeErrorFormatter.format_process_terminated_error(iteration)

        # SIGTERM interruption (exit code 143 = 128 + 15)
        if "Command failed with exit code 143" in error_str:
            return ClaudeErrorFormatter.format_interrupted_error(iteration)

        # Connection errors
        if error_type == "CLIConnectionError" or "connection" in error_str.lower():
            return ClaudeErrorFormatter.format_connection_error(iteration)

        # Timeout errors
        if error_type in ("TimeoutError", "asyncio.TimeoutError") or "timeout" in error_str.lower():
            return ClaudeErrorFormatter.format_timeout_error(iteration, 0)

        # Rate limit errors
        if "rate limit" in error_str.lower() or error_type == "RateLimitError":
            return ClaudeErrorFormatter.format_rate_limit_error(iteration)

        # Authentication errors
        if "authentication" in error_str.lower() or "auth" in error_str.lower() or error_type == "AuthenticationError":
            return ClaudeErrorFormatter.format_authentication_error(iteration)

        # Permission errors
        if error_type == "PermissionError" or "permission denied" in error_str.lower():
            return ClaudeErrorFormatter.format_permission_error(iteration)

        # Fall back to generic error format
        return ClaudeErrorFormatter.format_generic_error(iteration, error_type, error_str)
