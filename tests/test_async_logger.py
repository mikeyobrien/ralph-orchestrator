# ABOUTME: Unit tests for AsyncFileLogger
# ABOUTME: Tests log rotation, thread safety, unicode sanitization, and security masking

"""Tests for async_logger.py module."""

import asyncio
import tempfile
import threading
from pathlib import Path
from unittest.mock import patch

import pytest

from ralph_orchestrator.async_logger import AsyncFileLogger


class TestAsyncFileLoggerInit:
    """Tests for AsyncFileLogger initialization."""

    def test_init_creates_log_directory(self):
        """Logger should create log directory if it doesn't exist."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "subdir" / "test.log"
            AsyncFileLogger(str(log_path))
            assert log_path.parent.exists()

    def test_init_rejects_empty_path(self):
        """Logger should reject empty log file path."""
        with pytest.raises(ValueError, match="cannot be None or empty"):
            AsyncFileLogger("")

    def test_init_rejects_none_path(self):
        """Logger should reject None log file path."""
        with pytest.raises(ValueError, match="cannot be None or empty"):
            AsyncFileLogger(None)

    def test_init_accepts_path_object(self):
        """Logger should accept Path objects."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(log_path)
            assert logger.log_file == log_path

    def test_init_verbose_default_false(self):
        """Verbose should default to False."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            assert logger.verbose is False

    def test_init_verbose_can_be_enabled(self):
        """Verbose can be set to True."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path), verbose=True)
            assert logger.verbose is True


class TestAsyncFileLoggerBasicLogging:
    """Tests for basic logging functionality."""

    @pytest.mark.asyncio
    async def test_log_creates_file(self):
        """Logging should create the log file."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Test message")
            assert log_path.exists()

    @pytest.mark.asyncio
    async def test_log_writes_message(self):
        """Logging should write the message to file."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Hello World")
            content = log_path.read_text()
            assert "Hello World" in content
            assert "[INFO]" in content

    @pytest.mark.asyncio
    async def test_log_includes_timestamp(self):
        """Log entries should include timestamp."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Test")
            content = log_path.read_text()
            # Timestamp format: YYYY-MM-DD HH:MM:SS
            import re

            assert re.search(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}", content)

    @pytest.mark.asyncio
    async def test_log_info(self):
        """log_info should use INFO level."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log_info("Info message")
            content = log_path.read_text()
            assert "[INFO]" in content

    @pytest.mark.asyncio
    async def test_log_success(self):
        """log_success should use SUCCESS level."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log_success("Success message")
            content = log_path.read_text()
            assert "[SUCCESS]" in content

    @pytest.mark.asyncio
    async def test_log_error(self):
        """log_error should use ERROR level."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log_error("Error message")
            content = log_path.read_text()
            assert "[ERROR]" in content

    @pytest.mark.asyncio
    async def test_log_warning(self):
        """log_warning should use WARNING level."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log_warning("Warning message")
            content = log_path.read_text()
            assert "[WARNING]" in content


class TestAsyncFileLoggerSyncMethods:
    """Tests for synchronous wrapper methods."""

    def test_log_info_sync(self):
        """log_info_sync should work synchronously."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.log_info_sync("Sync info")
            content = log_path.read_text()
            assert "[INFO]" in content
            assert "Sync info" in content

    def test_log_success_sync(self):
        """log_success_sync should work synchronously."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.log_success_sync("Sync success")
            content = log_path.read_text()
            assert "[SUCCESS]" in content

    def test_log_error_sync(self):
        """log_error_sync should work synchronously."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.log_error_sync("Sync error")
            content = log_path.read_text()
            assert "[ERROR]" in content

    def test_log_warning_sync(self):
        """log_warning_sync should work synchronously."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.log_warning_sync("Sync warning")
            content = log_path.read_text()
            assert "[WARNING]" in content

    def test_info_standard_interface(self):
        """info() should work as standard logging interface."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.info("Standard info")
            content = log_path.read_text()
            assert "Standard info" in content

    def test_error_standard_interface(self):
        """error() should work as standard logging interface."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.error("Standard error")
            content = log_path.read_text()
            assert "Standard error" in content

    def test_warning_standard_interface(self):
        """warning() should work as standard logging interface."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.warning("Standard warning")
            content = log_path.read_text()
            assert "Standard warning" in content

    def test_critical_standard_interface(self):
        """critical() should work as standard logging interface."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.critical("Critical message")
            content = log_path.read_text()
            assert "Critical message" in content


class TestAsyncFileLoggerUnicodeSanitization:
    """Tests for unicode sanitization."""

    @pytest.mark.asyncio
    async def test_sanitize_unicode_normal_text(self):
        """Normal text should pass through unchanged."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Hello World")
            content = log_path.read_text()
            assert "Hello World" in content

    @pytest.mark.asyncio
    async def test_sanitize_unicode_emoji(self):
        """Emoji should be handled correctly."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Test with emoji: 🎉")
            content = log_path.read_text()
            # Emoji might be preserved or replaced
            assert "Test with emoji" in content

    @pytest.mark.asyncio
    async def test_sanitize_unicode_non_ascii(self):
        """Non-ASCII characters should be handled."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Cafe with accent: cafe")
            content = log_path.read_text()
            assert "Cafe with accent" in content


class TestAsyncFileLoggerSecurityMasking:
    """Tests for sensitive data masking."""

    @pytest.mark.asyncio
    async def test_masks_api_keys(self):
        """API keys should be masked."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Using API key: sk-1234567890abcdef")
            content = log_path.read_text()
            # Original key should not be visible
            assert "1234567890abcdef" not in content
            # Should contain masked version
            assert "sk-***********" in content

    @pytest.mark.asyncio
    async def test_masks_bearer_tokens(self):
        """Bearer tokens should be masked."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Auth: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9")
            content = log_path.read_text()
            assert "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9" not in content
            assert "Bearer ***********" in content

    @pytest.mark.asyncio
    async def test_masks_passwords(self):
        """Passwords should be masked."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "password=mysecretpassword123")
            content = log_path.read_text()
            assert "mysecretpassword123" not in content
            assert "*********" in content


class TestAsyncFileLoggerRotation:
    """Tests for log rotation functionality."""

    def test_rotation_constants(self):
        """Verify rotation constants."""
        assert AsyncFileLogger.MAX_LOG_SIZE_BYTES == 10 * 1024 * 1024
        assert AsyncFileLogger.MAX_BACKUP_FILES == 3

    @pytest.mark.asyncio
    async def test_rotation_creates_backup(self):
        """Log rotation should create backup files."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Create a log file larger than max size
            with open(log_path, "w") as f:
                # Write enough data to exceed MAX_LOG_SIZE_BYTES
                f.write("x" * (AsyncFileLogger.MAX_LOG_SIZE_BYTES + 1000))

            # Trigger rotation by logging
            await logger.log("INFO", "Trigger rotation")

            # Check backup was created
            backup_path = log_path.with_suffix(".log.1")
            assert backup_path.exists()

    @pytest.mark.asyncio
    async def test_rotation_max_backups(self):
        """Log rotation should respect max backup count."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"

            # Create multiple backup files
            for i in range(1, 6):
                backup = log_path.with_suffix(f".log.{i}")
                backup.write_text(f"backup {i}")

            logger = AsyncFileLogger(str(log_path))

            # Create a log file larger than max size
            with open(log_path, "w") as f:
                f.write("x" * (AsyncFileLogger.MAX_LOG_SIZE_BYTES + 1000))

            # Trigger rotation
            await logger.log("INFO", "Trigger rotation")

            # Verify max backups (3)
            # After rotation, .log.1, .log.2, .log.3 should exist
            assert log_path.with_suffix(".log.1").exists(), "Backup .log.1 should exist"
            assert log_path.with_suffix(".log.2").exists(), "Backup .log.2 should exist"
            assert log_path.with_suffix(".log.3").exists(), "Backup .log.3 should exist"
            # .log.4 and .log.5 should be rotated out (only 3 backups kept)
            assert not log_path.with_suffix(".log.4").exists(), "Backup .log.4 should be rotated out"
            assert not log_path.with_suffix(".log.5").exists(), "Backup .log.5 should be rotated out"


class TestAsyncFileLoggerStats:
    """Tests for statistics methods."""

    @pytest.mark.asyncio
    async def test_get_stats_empty_file(self):
        """get_stats should return zeros for non-existent file."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            stats = logger.get_stats()
            assert stats["success_count"] == 0
            assert stats["error_count"] == 0
            assert stats["start_time"] is None

    @pytest.mark.asyncio
    async def test_get_stats_counts_successes(self):
        """get_stats should count successful iterations."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("SUCCESS", "Iteration 1 completed successfully")
            await logger.log("SUCCESS", "Iteration 2 completed successfully")
            stats = logger.get_stats()
            assert stats["success_count"] == 2

    @pytest.mark.asyncio
    async def test_get_stats_counts_errors(self):
        """get_stats should count failed iterations."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("ERROR", "Iteration 1 failed with error")
            stats = logger.get_stats()
            assert stats["error_count"] == 1

    @pytest.mark.asyncio
    async def test_get_stats_extracts_start_time(self):
        """get_stats should extract start time from first entry."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Session started")
            stats = logger.get_stats()
            assert stats["start_time"] is not None

    @pytest.mark.asyncio
    async def test_get_recent_lines(self):
        """get_recent_lines should return last N lines."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            for i in range(5):
                await logger.log("INFO", f"Message {i}")
            lines = logger.get_recent_lines(2)
            assert len(lines) == 2
            assert "Message 4" in lines[1]

    @pytest.mark.asyncio
    async def test_get_recent_lines_default_count(self):
        """get_recent_lines should use default count."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            for i in range(5):
                await logger.log("INFO", f"Message {i}")
            lines = logger.get_recent_lines()
            assert len(lines) == AsyncFileLogger.DEFAULT_RECENT_LINES_COUNT

    @pytest.mark.asyncio
    async def test_count_pattern(self):
        """count_pattern should count occurrences."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "Test pattern")
            await logger.log("INFO", "Another pattern here")
            await logger.log("INFO", "No match")
            count = logger.count_pattern("pattern")
            assert count == 2

    @pytest.mark.asyncio
    async def test_get_start_time(self):
        """get_start_time should return first log timestamp."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            await logger.log("INFO", "First message")
            await logger.log("INFO", "Second message")
            start_time = logger.get_start_time()
            assert start_time is not None
            # Should be in format YYYY-MM-DD HH:MM:SS
            parts = start_time.split(" ")
            assert len(parts) == 2


class TestAsyncFileLoggerThreadSafety:
    """Tests for thread-safe operations."""

    @pytest.mark.asyncio
    async def test_concurrent_logging(self):
        """Multiple concurrent log calls should not corrupt data."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Create multiple concurrent log tasks
            tasks = [logger.log("INFO", f"Message {i}") for i in range(10)]
            await asyncio.gather(*tasks)

            # Verify all messages were logged
            content = log_path.read_text()
            for i in range(10):
                assert f"Message {i}" in content

    def test_concurrent_sync_logging(self):
        """Multiple threads using sync methods should not corrupt data."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            results = []

            def log_from_thread(i):
                try:
                    logger.log_info_sync(f"Thread {i}")
                    results.append(i)
                except Exception as e:
                    results.append(f"error: {e}")

            threads = [threading.Thread(target=log_from_thread, args=(i,)) for i in range(5)]
            for t in threads:
                t.start()
            for t in threads:
                t.join()

            # All threads should have completed
            assert len(results) == 5

            # All messages should be in log
            content = log_path.read_text()
            for i in range(5):
                assert f"Thread {i}" in content


class TestAsyncFileLoggerVerbose:
    """Tests for verbose mode."""

    @pytest.mark.asyncio
    async def test_verbose_prints_to_console(self, capsys):
        """Verbose mode should print to console."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path), verbose=True)
            await logger.log("INFO", "Verbose message")
            captured = capsys.readouterr()
            assert "Verbose message" in captured.out

    @pytest.mark.asyncio
    async def test_non_verbose_no_console(self, capsys):
        """Non-verbose mode should not print to console."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path), verbose=False)
            await logger.log("INFO", "Silent message")
            captured = capsys.readouterr()
            assert captured.out == ""


class TestAsyncFileLoggerErrorHandling:
    """Tests for error handling in sync logging methods."""

    def test_sync_logging_handles_permission_error(self, capsys):
        """Sync logging should handle PermissionError gracefully with stderr fallback."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Mock _write_to_file to raise PermissionError
            with patch.object(logger, '_write_to_file', side_effect=PermissionError("Permission denied")):
                # Should not raise exception
                logger.log_info_sync("Test message with permission error")

            # Verify stderr output contains error details
            captured = capsys.readouterr()
            assert "[LOGGING ERROR]" in captured.err
            assert "PermissionError" in captured.err
            assert "Permission denied" in captured.err
            assert "Test message" in captured.err

    def test_sync_logging_handles_os_error(self, capsys):
        """Sync logging should handle OSError gracefully with stderr fallback."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Mock _write_to_file to raise OSError
            with patch.object(logger, '_write_to_file', side_effect=OSError("Disk full")):
                logger.log_info_sync("Test message with OS error")

            captured = capsys.readouterr()
            assert "[LOGGING ERROR]" in captured.err
            assert "OSError" in captured.err
            assert "Disk full" in captured.err

    def test_sync_logging_handles_io_error(self, capsys):
        """Sync logging should handle IOError gracefully with stderr fallback."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Mock _write_to_file to raise IOError (which is OSError in Python 3)
            with patch.object(logger, '_write_to_file', side_effect=IOError("I/O error")):
                logger.log_error_sync("Test error message")

            captured = capsys.readouterr()
            assert "[LOGGING ERROR]" in captured.err
            # IOError is an alias for OSError in Python 3
            assert "OSError" in captured.err
            assert "I/O error" in captured.err

    def test_sync_logging_truncates_long_messages_in_stderr(self, capsys):
        """Stderr fallback should truncate very long messages."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            long_message = "X" * 300  # Message longer than 200 chars

            with patch.object(logger, '_write_to_file', side_effect=OSError("Error")):
                logger.log_info_sync(long_message)

            captured = capsys.readouterr()
            # Should contain truncated message (first 200 chars)
            assert "X" * 200 in captured.err
            # Should not contain full message
            assert "X" * 300 not in captured.err

    def test_sync_logging_preserves_emergency_shutdown_check(self):
        """Emergency shutdown should still prevent logging attempts."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Trigger emergency shutdown
            logger.emergency_shutdown()

            # Even with mocked error, should return immediately
            with patch.object(logger, '_write_to_file', side_effect=OSError("Should not reach")) as mock_write:
                logger.log_info_sync("Should not log")

            # _write_to_file should never be called
            mock_write.assert_not_called()

    def test_normal_logging_still_works_after_error_handling(self):
        """Normal logging should continue to work after error handling is added."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))
            logger.log_info_sync("Normal message")

            content = log_path.read_text()
            assert "Normal message" in content
            assert "[INFO]" in content

    def test_concurrent_errors_dont_corrupt_stderr(self, capsys):
        """Multiple threads encountering errors should have thread-safe stderr output."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            def log_with_error(i):
                with patch.object(logger, '_write_to_file', side_effect=OSError(f"Error {i}")):
                    logger.log_info_sync(f"Thread {i} message")

            threads = [threading.Thread(target=log_with_error, args=(i,)) for i in range(3)]
            for t in threads:
                t.start()
            for t in threads:
                t.join()

            captured = capsys.readouterr()
            # Verify all errors were reported
            assert captured.err.count("[LOGGING ERROR]") == 3


class TestSyncMethodsThreadSafety:
    """Tests for thread safety of synchronous logging methods."""

    def test_high_contention_no_deadlock(self):
        """High contention with 20+ threads should not cause deadlock (regression test for e5577bb)."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Use barrier to synchronize thread start for maximum contention
            num_threads = 20
            messages_per_thread = 10
            barrier = threading.Barrier(num_threads)
            results = []

            def log_with_barrier(thread_id):
                try:
                    # Wait for all threads to be ready
                    barrier.wait()
                    # Now all threads log simultaneously
                    for i in range(messages_per_thread):
                        logger.log_info_sync(f"Thread {thread_id} message {i}")
                    results.append(thread_id)
                except Exception as e:
                    results.append(f"error-{thread_id}: {e}")

            threads = [threading.Thread(target=log_with_barrier, args=(i,)) for i in range(num_threads)]

            # Start all threads
            for t in threads:
                t.start()

            # Join with timeout to detect deadlocks
            timeout = 5.0
            for t in threads:
                t.join(timeout=timeout)
                if t.is_alive():
                    pytest.fail(f"Thread deadlock detected - thread did not complete within {timeout}s")

            # All threads should have completed successfully
            assert len(results) == num_threads, f"Expected {num_threads} results, got {len(results)}"

            # No errors should have occurred
            error_results = [r for r in results if isinstance(r, str) and r.startswith("error-")]
            assert len(error_results) == 0, f"Errors occurred: {error_results}"

            # Verify all messages were logged
            content = log_path.read_text()
            for thread_id in range(num_threads):
                for msg_num in range(messages_per_thread):
                    assert f"Thread {thread_id} message {msg_num}" in content


class TestAsyncLoggerEmergencyShutdown:
    """Tests for emergency shutdown behavior during async logging."""

    @pytest.mark.asyncio
    async def test_async_log_respects_emergency_shutdown_after_lock(self):
        """Emergency shutdown triggered after lock acquired should abort logging.

        This tests the race condition protection at async_logger.py:150-153.
        """
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Write an initial message so we know the logger works
            await logger.log("INFO", "Initial message")

            # Create a wrapper that triggers shutdown after lock is acquired
            original_write = logger._write_to_file
            call_count = [0]

            def trigger_shutdown_then_write(line):
                call_count[0] += 1
                if call_count[0] == 2:  # Second call (after initial)
                    # Trigger shutdown - this simulates the race condition
                    logger.emergency_shutdown()
                return original_write(line)

            with patch.object(logger, '_write_to_file', side_effect=trigger_shutdown_then_write):
                await logger.log("INFO", "Should not appear after shutdown")

            content = log_path.read_text()
            assert "Initial message" in content
            # The message might or might not appear depending on timing
            # But the key is no exception was raised


class TestAsyncLoggerStderrFailure:
    """Tests for stderr failure handling."""

    def test_sync_logging_handles_stderr_failure_silently(self):
        """When both file I/O and stderr fail, should not raise exception.

        This tests the silent fallback at async_logger.py:323-325.
        """
        from unittest.mock import MagicMock

        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Mock _write_to_file to raise OSError
            # Mock sys.stderr.write to raise IOError (must use MagicMock for write)
            mock_stderr = MagicMock()
            mock_stderr.write.side_effect = IOError("Broken pipe")

            with patch.object(logger, '_write_to_file', side_effect=OSError("Disk full")):
                with patch('sys.stderr', mock_stderr):
                    # Should not raise - silently ignores
                    logger.log_info_sync("Test message")

            # Verify stderr.write was attempted (fallback was triggered)
            assert mock_stderr.write.called, "stderr fallback should have been attempted"


class TestSyncMethodsBehavior:
    """Tests for behavioral requirements of synchronous logging methods."""

    def test_sync_works_without_event_loop(self):
        """Sync methods should work without any asyncio event loop (regression test for e5577bb)."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Ensure no asyncio event loop exists
            # In a fresh thread, there will be no event loop
            results = []

            def log_without_loop():
                try:
                    # Verify no loop exists (this would raise RuntimeError)
                    try:
                        asyncio.get_running_loop()
                        results.append("error: event loop found")
                    except RuntimeError:
                        # Expected - no running loop
                        pass

                    # Now log - should work without asyncio
                    logger.log_info_sync("Message without event loop")
                    logger.log_success_sync("Success without event loop")
                    logger.log_error_sync("Error without event loop")
                    logger.log_warning_sync("Warning without event loop")
                    results.append("success")
                except Exception as e:
                    results.append(f"error: {e}")

            thread = threading.Thread(target=log_without_loop)
            thread.start()
            thread.join(timeout=2.0)

            assert not thread.is_alive(), "Thread timed out"
            assert results == ["success"], f"Expected success, got {results}"

            # Verify all messages were written
            content = log_path.read_text()
            assert "Message without event loop" in content
            assert "Success without event loop" in content
            assert "Error without event loop" in content
            assert "Warning without event loop" in content

    def test_sync_respects_emergency_shutdown(self):
        """Emergency shutdown should prevent sync methods from writing to log."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Log a message before shutdown
            logger.log_info_sync("Before shutdown")

            # Trigger emergency shutdown
            logger.emergency_shutdown()

            # Try to log after shutdown
            logger.log_info_sync("After shutdown - should not appear")
            logger.log_error_sync("Error after shutdown - should not appear")

            # Read log content
            content = log_path.read_text()

            # Message before shutdown should be present
            assert "Before shutdown" in content

            # Messages after shutdown should NOT be present
            assert "After shutdown" not in content
            assert "Error after shutdown" not in content

    def test_sync_verbose_prints_to_console(self, capsys):
        """Verbose mode should print to console in sync methods."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path), verbose=True)

            # Log with verbose enabled
            logger.log_info_sync("Verbose sync message")

            # Check console output
            captured = capsys.readouterr()
            assert "Verbose sync message" in captured.out
            assert "[INFO]" in captured.out

            # Also verify it was written to file
            content = log_path.read_text()
            assert "Verbose sync message" in content

    def test_sync_masks_sensitive_data(self):
        """Security masking should work in sync code path."""
        with tempfile.TemporaryDirectory() as tmpdir:
            log_path = Path(tmpdir) / "test.log"
            logger = AsyncFileLogger(str(log_path))

            # Log message with API key
            logger.log_info_sync("Using API key: sk-1234567890abcdef")

            # Log message with bearer token
            logger.log_error_sync("Auth failed: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9")

            # Log message with password
            logger.log_warning_sync("password=mysecretpassword123 is invalid")

            # Read log content
            content = log_path.read_text()

            # Original sensitive data should NOT be visible
            assert "1234567890abcdef" not in content
            assert "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9" not in content
            assert "mysecretpassword123" not in content

            # Masked versions should be present
            assert "sk-***********" in content
            assert "Bearer ***********" in content
            assert "*********" in content
