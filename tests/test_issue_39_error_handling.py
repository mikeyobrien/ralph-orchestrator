"""
Test for GitHub issue #39 error handling improvements
"""

import pytest
from ralph_orchestrator.error_formatter import ClaudeErrorFormatter


class TestIssue39ErrorHandling:
    """Test error handling improvements for GitHub issue #39"""
    
    def test_command_failed_exit_code_1(self):
        """Test that exit code 1 errors are properly formatted"""
        # This is the exact error from the GitHub issue
        exception = Exception("Command failed with exit code 1 (exit code: 1)\nError output: Check stderr output for details")
        
        error_msg = ClaudeErrorFormatter.format_error_from_exception(
            iteration=1,
            exception=exception
        )
        
        # Check that the error is properly formatted with helpful message
        assert "Claude CLI command failed" in error_msg.message
        assert "claude --version" in error_msg.suggestion
        assert "claude login" in error_msg.suggestion
    
    def test_command_failed_exit_code_143(self):
        """Test that exit code 143 (SIGTERM) is still handled correctly"""
        exception = Exception("Command failed with exit code 143")
        
        error_msg = ClaudeErrorFormatter.format_error_from_exception(
            iteration=1,
            exception=exception
        )
        
        assert "interrupted" in error_msg.message.lower()
        assert "SIGTERM" in error_msg.message
    
    def test_generic_command_failed(self):
        """Test generic command failed errors"""
        exception = Exception("Command failed with exit code 2")
        
        error_msg = ClaudeErrorFormatter.format_error_from_exception(
            iteration=1,
            exception=exception
        )
        
        # Should fall back to generic error handling
        assert "Exception" in error_msg.message
        assert "Command failed with exit code 2" in error_msg.message
    
    def test_error_message_structure(self):
        """Test that error messages have proper structure"""
        exception = Exception("Command failed with exit code 1")
        
        error_msg = ClaudeErrorFormatter.format_error_from_exception(
            iteration=5,
            exception=exception
        )
        
        # Check structure
        assert isinstance(error_msg.message, str)
        assert isinstance(error_msg.suggestion, str)
        assert len(error_msg.message) > 0
        assert len(error_msg.suggestion) > 0
        assert "Iteration 5" in error_msg.message
