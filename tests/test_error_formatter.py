# ABOUTME: Tests for error formatter module
# ABOUTME: Tests structured error messages and security sanitization

"""Tests for the error_formatter module."""

import pytest
from ralph_orchestrator.error_formatter import ClaudeErrorFormatter, ErrorMessage


class TestErrorMessage:
    """Tests for the ErrorMessage dataclass."""

    def test_error_message_creation(self):
        """Test basic ErrorMessage creation."""
        error = ErrorMessage(
            message="Test error message",
            suggestion="Test suggestion"
        )
        assert error.message == "Test error message"
        assert error.suggestion == "Test suggestion"

    def test_error_message_str(self):
        """Test ErrorMessage string representation."""
        error = ErrorMessage(
            message="Error occurred",
            suggestion="Try this fix"
        )
        assert str(error) == "Error occurred | Try this fix"

    def test_error_message_with_empty_values(self):
        """Test ErrorMessage with empty strings."""
        error = ErrorMessage(message="", suggestion="")
        assert str(error) == " | "


class TestClaudeErrorFormatterBasic:
    """Tests for basic ClaudeErrorFormatter methods."""

    def test_format_timeout_error(self):
        """Test timeout error formatting."""
        error = ClaudeErrorFormatter.format_timeout_error(iteration=5, timeout=300)

        assert isinstance(error, ErrorMessage)
        assert "5" in error.message
        assert "300" in error.message
        assert "timeout" in error.message.lower()
        assert "timeout" in error.suggestion.lower() or "config" in error.suggestion.lower()

    def test_format_process_terminated_error(self):
        """Test process terminated error formatting."""
        error = ClaudeErrorFormatter.format_process_terminated_error(iteration=3)

        assert isinstance(error, ErrorMessage)
        assert "3" in error.message
        assert "terminated" in error.message.lower() or "subprocess" in error.message.lower()
        assert "cli" in error.suggestion.lower() or "install" in error.suggestion.lower()

    def test_format_interrupted_error(self):
        """Test interrupted (SIGTERM) error formatting."""
        error = ClaudeErrorFormatter.format_interrupted_error(iteration=7)

        assert isinstance(error, ErrorMessage)
        assert "7" in error.message
        assert "interrupt" in error.message.lower() or "sigterm" in error.message.lower()
        assert "stopping" in error.suggestion.lower() or "no action" in error.suggestion.lower()

    def test_format_connection_error(self):
        """Test connection error formatting."""
        error = ClaudeErrorFormatter.format_connection_error(iteration=2)

        assert isinstance(error, ErrorMessage)
        assert "2" in error.message
        assert "connect" in error.message.lower()
        assert "claude" in error.suggestion.lower()

    def test_format_rate_limit_error(self):
        """Test rate limit error formatting."""
        error = ClaudeErrorFormatter.format_rate_limit_error(iteration=4, retry_after=120)

        assert isinstance(error, ErrorMessage)
        assert "4" in error.message
        assert "rate limit" in error.message.lower()
        assert "120" in error.suggestion

    def test_format_rate_limit_error_default_retry(self):
        """Test rate limit error with default retry time."""
        error = ClaudeErrorFormatter.format_rate_limit_error(iteration=1)

        assert "60" in error.suggestion

    def test_format_authentication_error(self):
        """Test authentication error formatting."""
        error = ClaudeErrorFormatter.format_authentication_error(iteration=1)

        assert isinstance(error, ErrorMessage)
        assert "auth" in error.message.lower()
        assert "login" in error.suggestion.lower() or "credentials" in error.suggestion.lower()

    def test_format_permission_error_with_path(self):
        """Test permission error with specific path."""
        error = ClaudeErrorFormatter.format_permission_error(iteration=6, path="/home/user/file.txt")

        assert isinstance(error, ErrorMessage)
        assert "6" in error.message
        assert "permission" in error.message.lower()
        assert "/home/user/file.txt" in error.message
        assert "permission" in error.suggestion.lower()

    def test_format_permission_error_without_path(self):
        """Test permission error without specific path."""
        error = ClaudeErrorFormatter.format_permission_error(iteration=6)

        assert isinstance(error, ErrorMessage)
        assert "permission" in error.message.lower()
        assert "permission" in error.suggestion.lower()

    def test_format_permission_error_with_long_path(self):
        """Test permission error with very long path (should be excluded)."""
        long_path = "/very/long/path/" + "a" * 200
        error = ClaudeErrorFormatter.format_permission_error(iteration=6, path=long_path)

        # Long path should not be included in message
        assert long_path not in error.message


class TestClaudeErrorFormatterGeneric:
    """Tests for generic error formatting with security sanitization."""

    def test_format_generic_error(self):
        """Test generic error formatting."""
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=10,
            error_type="RuntimeError",
            error_str="Something went wrong"
        )

        assert isinstance(error, ErrorMessage)
        assert "10" in error.message
        assert "RuntimeError" in error.message
        assert "Something went wrong" in error.message

    def test_format_generic_error_masks_api_key(self):
        """Test that generic error masks API keys."""
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=1,
            error_type="AuthError",
            error_str="Invalid API key: sk-abcdefghijklmnop12345"
        )

        # API key should be masked
        assert "sk-abcdefghijklmnop12345" not in error.message
        assert "sk-" in error.message or "***" in error.message

    def test_format_generic_error_masks_password(self):
        """Test that generic error masks passwords."""
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=1,
            error_type="ConfigError",
            error_str="password=supersecret123"
        )

        # Password should be masked
        assert "supersecret123" not in error.message
        assert "***" in error.message or "*********" in error.message

    def test_format_generic_error_truncates_long_messages(self):
        """Test that very long error messages are truncated."""
        long_error = "x" * 500
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=1,
            error_type="Error",
            error_str=long_error
        )

        # Should be truncated with ellipsis
        assert len(error.message) < 400  # Much shorter than original
        assert "..." in error.message


class TestClaudeErrorFormatterFromException:
    """Tests for format_error_from_exception method."""

    def test_format_from_process_transport_exception(self):
        """Test formatting from ProcessTransport exception."""
        exception = RuntimeError("ProcessTransport is not ready for messaging")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=5, exception=exception)

        assert "terminated" in error.message.lower() or "subprocess" in error.message.lower()
        assert "cli" in error.suggestion.lower() or "install" in error.suggestion.lower()

    def test_format_from_exit_code_143_exception(self):
        """Test formatting from SIGTERM exit code exception."""
        exception = RuntimeError("Command failed with exit code 143")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=3, exception=exception)

        assert "interrupt" in error.message.lower() or "sigterm" in error.message.lower()

    def test_format_from_exit_code_1_exception(self):
        """Test formatting from exit code 1 exception."""
        exception = Exception(
            "Command failed with exit code 1 (exit code: 1)\n"
            "Error output: Check stderr output for details"
        )
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=1, exception=exception)

        assert "Claude CLI command failed" in error.message
        assert "claude --version" in error.suggestion
        assert "claude login" in error.suggestion

    def test_format_from_exit_code_2_exception(self):
        """Test generic command failed errors."""
        exception = Exception("Command failed with exit code 2")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=1, exception=exception)

        assert "Exception" in error.message
        assert "Command failed with exit code 2" in error.message

    def test_format_from_exception_message_structure(self):
        """Test that error messages have proper structure."""
        exception = Exception("Command failed with exit code 1")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=5, exception=exception)

        assert isinstance(error.message, str)
        assert isinstance(error.suggestion, str)
        assert len(error.message) > 0
        assert len(error.suggestion) > 0
        assert "Iteration 5" in error.message

    def test_format_from_timeout_exception(self):
        """Test formatting from timeout exception."""
        import asyncio
        exception = asyncio.TimeoutError()
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=2, exception=exception)

        assert "timeout" in error.message.lower()

    def test_format_from_permission_exception(self):
        """Test formatting from PermissionError."""
        exception = PermissionError("Permission denied: /etc/passwd")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=4, exception=exception)

        assert "permission" in error.message.lower()

    def test_format_from_connection_keyword_exception(self):
        """Test formatting from exception with 'connection' keyword."""
        exception = RuntimeError("Connection refused to Claude service")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=1, exception=exception)

        assert "connect" in error.message.lower()
        assert "claude" in error.suggestion.lower()

    def test_format_from_rate_limit_exception(self):
        """Test formatting from rate limit exception."""
        exception = RuntimeError("Rate limit exceeded, please retry later")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=6, exception=exception)

        assert "rate limit" in error.message.lower()

    def test_format_from_auth_exception(self):
        """Test formatting from authentication exception."""
        exception = RuntimeError("Authentication failed: invalid token")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=1, exception=exception)

        assert "auth" in error.message.lower()

    def test_format_from_unknown_exception(self):
        """Test formatting from unknown exception type."""
        exception = ValueError("Unknown error occurred")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=8, exception=exception)

        assert isinstance(error, ErrorMessage)
        assert "ValueError" in error.message
        assert "Unknown error occurred" in error.message


class TestClaudeErrorFormatterSecurity:
    """Security-focused tests for error formatter."""

    def test_masks_bearer_token(self):
        """Test that bearer tokens are masked."""
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=1,
            error_type="AuthError",
            error_str="Bearer abcd1234efgh5678ijkl9012mnop"
        )
        assert "abcd1234efgh5678ijkl9012mnop" not in error.message

    def test_masks_google_api_key(self):
        """Test that Google API keys are masked."""
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=1,
            error_type="ConfigError",
            error_str="Using key: AIzaSyA0B1C2D3E4F5G6H7I8J9K0L1M2N3O4P5Q"
        )
        assert "AIzaSyA0B1C2D3E4F5G6H7I8J9K0L1M2N3O4P5Q" not in error.message

    def test_masks_ssh_paths(self):
        """Test that SSH paths are masked."""
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=1,
            error_type="FileError",
            error_str="Cannot read /home/user/.ssh/id_rsa"
        )
        # SSH paths should be redacted
        assert ".ssh" not in error.message or "REDACTED" in error.message

    def test_preserves_safe_error_content(self):
        """Test that non-sensitive error content is preserved."""
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=1,
            error_type="ValueError",
            error_str="Invalid format for date: 2024-01-15"
        )
        # Normal error content should be preserved
        assert "Invalid format" in error.message
        assert "2024-01-15" in error.message


class TestClaudeErrorFormatterEdgeCases:
    """Edge case tests for error formatter."""

    def test_iteration_zero(self):
        """Test formatting with iteration 0."""
        error = ClaudeErrorFormatter.format_timeout_error(iteration=0, timeout=60)
        assert "0" in error.message

    def test_negative_iteration(self):
        """Test formatting with negative iteration (edge case)."""
        error = ClaudeErrorFormatter.format_timeout_error(iteration=-1, timeout=60)
        assert "-1" in error.message

    def test_large_iteration_number(self):
        """Test formatting with very large iteration number."""
        error = ClaudeErrorFormatter.format_timeout_error(iteration=999999, timeout=60)
        assert "999999" in error.message

    def test_zero_timeout(self):
        """Test formatting with zero timeout."""
        error = ClaudeErrorFormatter.format_timeout_error(iteration=1, timeout=0)
        assert "0s" in error.message

    def test_empty_exception_message(self):
        """Test formatting from exception with empty message."""
        exception = RuntimeError("")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=1, exception=exception)
        assert isinstance(error, ErrorMessage)
        assert "RuntimeError" in error.message

    def test_none_in_exception_string(self):
        """Test formatting from exception with None-like content."""
        exception = RuntimeError("None")
        error = ClaudeErrorFormatter.format_error_from_exception(iteration=1, exception=exception)
        assert isinstance(error, ErrorMessage)

    def test_unicode_in_error_message(self):
        """Test formatting with unicode characters in error."""
        error = ClaudeErrorFormatter.format_generic_error(
            iteration=1,
            error_type="UnicodeError",
            error_str="Invalid character: \u2603 (snowman)"
        )
        assert isinstance(error, ErrorMessage)
        # Unicode should be preserved
        assert "snowman" in error.message or "\u2603" in error.message


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
