# ABOUTME: Tests for graceful signal handling
# ABOUTME: Verifies subprocess-first cleanup, emergency shutdown, and async task cancellation

"""Tests for graceful signal handling in Ralph Orchestrator."""

import asyncio
import signal
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from ralph_orchestrator.adapters.claude import ClaudeAdapter
from ralph_orchestrator.async_logger import AsyncFileLogger


class TestClaudeAdapterSignalHandling(unittest.TestCase):
    """Test Claude adapter signal handling methods."""

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_subprocess_pid_attribute_exists(self):
        """Test that _subprocess_pid attribute exists."""
        adapter = ClaudeAdapter()
        self.assertIsNone(adapter._subprocess_pid)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_kill_subprocess_sync_no_process(self):
        """Test kill_subprocess_sync when no process is running."""
        adapter = ClaudeAdapter()
        # Should not raise any exceptions
        adapter.kill_subprocess_sync()
        self.assertIsNone(adapter._subprocess_pid)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    @patch('os.kill')
    def test_kill_subprocess_sync_with_process(self, mock_kill):
        """Test kill_subprocess_sync with a running process."""
        adapter = ClaudeAdapter()
        adapter._subprocess_pid = 12345

        adapter.kill_subprocess_sync()

        # Should have called kill with SIGTERM and SIGKILL
        self.assertEqual(mock_kill.call_count, 2)
        mock_kill.assert_any_call(12345, signal.SIGTERM)
        mock_kill.assert_any_call(12345, signal.SIGKILL)
        # PID should be cleared
        self.assertIsNone(adapter._subprocess_pid)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    @patch('os.kill')
    def test_kill_subprocess_sync_process_already_dead(self, mock_kill):
        """Test kill_subprocess_sync when process is already dead."""
        mock_kill.side_effect = ProcessLookupError()
        adapter = ClaudeAdapter()
        adapter._subprocess_pid = 12345

        # Should not raise exception
        adapter.kill_subprocess_sync()
        self.assertIsNone(adapter._subprocess_pid)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    @patch('os.kill')
    def test_kill_subprocess_sync_permission_denied(self, mock_kill):
        """Test kill_subprocess_sync with permission error."""
        mock_kill.side_effect = PermissionError()
        adapter = ClaudeAdapter()
        adapter._subprocess_pid = 12345

        # Should not raise exception (best effort)
        adapter.kill_subprocess_sync()
        self.assertIsNone(adapter._subprocess_pid)


class TestAsyncFileLoggerEmergencyShutdown(unittest.TestCase):
    """Test AsyncFileLogger emergency shutdown functionality."""

    def test_emergency_shutdown_flag_initial(self):
        """Test that emergency shutdown is initially False."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_file = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_file))
            self.assertFalse(logger._emergency_shutdown)
            self.assertFalse(logger.is_shutdown())

    def test_emergency_shutdown_method(self):
        """Test that emergency_shutdown sets the flag."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_file = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_file))

            logger.emergency_shutdown()

            self.assertTrue(logger._emergency_shutdown)
            self.assertTrue(logger.is_shutdown())
            self.assertTrue(logger._emergency_event.is_set())

    def test_logging_skipped_after_shutdown(self):
        """Test that logging operations are skipped after shutdown."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_file = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_file))

            # Log something before shutdown
            asyncio.run(logger.log("INFO", "Before shutdown"))
            self.assertTrue(log_file.exists())

            # Get current file size
            initial_size = log_file.stat().st_size

            # Trigger shutdown
            logger.emergency_shutdown()

            # Try to log after shutdown
            asyncio.run(logger.log("INFO", "After shutdown"))

            # File size should not have changed
            final_size = log_file.stat().st_size
            self.assertEqual(initial_size, final_size)

    def test_sync_logging_skipped_after_shutdown(self):
        """Test that sync logging is skipped after shutdown."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_file = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_file))

            logger.emergency_shutdown()

            # These should all return immediately without doing anything
            logger.log_info_sync("Should not log")
            logger.log_error_sync("Should not log")
            logger.log_warning_sync("Should not log")
            logger.log_success_sync("Should not log")

            # File should not exist (no logging occurred)
            self.assertFalse(log_file.exists())

    def test_emergency_event_signal_safe(self):
        """Test that emergency_event can be set from different threads."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_file = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_file))

            def signal_handler():
                logger.emergency_shutdown()

            # Simulate calling from a different thread (like a signal handler)
            thread = threading.Thread(target=signal_handler)
            thread.start()
            thread.join()

            self.assertTrue(logger.is_shutdown())


class TestOrchestratorSignalHandling(unittest.TestCase):
    """Test RalphOrchestrator signal handling."""

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_orchestrator_has_signal_handling_attributes(self):
        """Test that orchestrator has required signal handling attributes."""
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            prompt_file = Path(tmpdir) / "PROMPT.md"
            prompt_file.write_text("# Test Prompt")

            orch = RalphOrchestrator(prompt_file_or_config=str(prompt_file))

            # Check attributes exist
            self.assertFalse(orch.stop_requested)
            self.assertIsNone(orch._running_task)
            self.assertIsNone(orch._async_logger)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_set_async_logger(self):
        """Test that async logger can be set."""
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            prompt_file = Path(tmpdir) / "PROMPT.md"
            prompt_file.write_text("# Test Prompt")
            log_file = Path(tmpdir) / "test.log"

            orch = RalphOrchestrator(prompt_file_or_config=str(prompt_file))
            logger = AsyncFileLogger(str(log_file))

            orch.set_async_logger(logger)

            self.assertIsNotNone(orch._async_logger)
            self.assertEqual(orch._async_logger, logger)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_signal_handler_sets_stop_flag(self):
        """Test that signal handler sets stop_requested flag."""
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            prompt_file = Path(tmpdir) / "PROMPT.md"
            prompt_file.write_text("# Test Prompt")

            orch = RalphOrchestrator(prompt_file_or_config=str(prompt_file))
            self.assertFalse(orch.stop_requested)

            # Simulate signal
            orch._signal_handler(signal.SIGINT, None)

            self.assertTrue(orch.stop_requested)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_signal_handler_triggers_logger_shutdown(self):
        """Test that signal handler triggers logger emergency shutdown."""
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            prompt_file = Path(tmpdir) / "PROMPT.md"
            prompt_file.write_text("# Test Prompt")
            log_file = Path(tmpdir) / "test.log"

            orch = RalphOrchestrator(prompt_file_or_config=str(prompt_file))
            logger = AsyncFileLogger(str(log_file))
            orch.set_async_logger(logger)

            self.assertFalse(logger.is_shutdown())

            # Simulate signal
            orch._signal_handler(signal.SIGINT, None)

            self.assertTrue(logger.is_shutdown())

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_signal_handler_calls_kill_subprocess(self):
        """Test that signal handler calls kill_subprocess_sync on adapter."""
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            prompt_file = Path(tmpdir) / "PROMPT.md"
            prompt_file.write_text("# Test Prompt")

            orch = RalphOrchestrator(prompt_file_or_config=str(prompt_file))

            # Mock the adapter's kill method
            orch.current_adapter.kill_subprocess_sync = MagicMock()

            # Simulate signal
            orch._signal_handler(signal.SIGINT, None)

            orch.current_adapter.kill_subprocess_sync.assert_called_once()

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_emergency_cleanup_exists(self):
        """Test that _emergency_cleanup method exists and is async."""
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            prompt_file = Path(tmpdir) / "PROMPT.md"
            prompt_file.write_text("# Test Prompt")

            orch = RalphOrchestrator(prompt_file_or_config=str(prompt_file))

            self.assertTrue(hasattr(orch, '_emergency_cleanup'))
            self.assertTrue(asyncio.iscoroutinefunction(orch._emergency_cleanup))


class TestClaudeAdapterCleanupTransport(unittest.IsolatedAsyncioTestCase):
    """Test async cleanup transport method."""

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    async def test_cleanup_transport_no_process(self):
        """Test _cleanup_transport when no process is running."""
        adapter = ClaudeAdapter()
        # Should not raise any exceptions
        await adapter._cleanup_transport()
        self.assertIsNone(adapter._subprocess_pid)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    @patch('os.kill')
    async def test_cleanup_transport_with_process(self, mock_kill):
        """Test _cleanup_transport with a running process."""
        adapter = ClaudeAdapter()
        adapter._subprocess_pid = 12345

        await adapter._cleanup_transport()

        # Should have called kill
        self.assertTrue(mock_kill.called)
        self.assertIsNone(adapter._subprocess_pid)


if __name__ == "__main__":
    unittest.main()
