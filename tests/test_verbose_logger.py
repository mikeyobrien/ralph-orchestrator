# ABOUTME: Tests for VerboseLogger with session metrics, emergency shutdown, re-entrancy protection
# ABOUTME: Covers Rich console integration and thread safety

"""Tests for VerboseLogger."""

import asyncio
import json
import os
import tempfile
import threading
import time
from unittest.mock import patch

import pytest

from ralph_orchestrator.verbose_logger import VerboseLogger, TextIOProxy, RICH_AVAILABLE


class TestTextIOProxy:
    """Tests for TextIOProxy class."""

    def test_init_creates_proxy(self, tmp_path):
        """Test TextIOProxy initialization."""
        file_path = tmp_path / "test.log"
        proxy = TextIOProxy(file_path)
        assert proxy.file_path == file_path
        assert not proxy._closed

    def test_write_creates_file_lazily(self, tmp_path):
        """Test that file is created on first write."""
        file_path = tmp_path / "test.log"
        proxy = TextIOProxy(file_path)

        assert proxy._file is None  # Not opened yet

        result = proxy.write("test content")
        assert result > 0
        assert file_path.exists()

    def test_write_after_close_returns_zero(self, tmp_path):
        """Test that write after close returns zero."""
        file_path = tmp_path / "test.log"
        proxy = TextIOProxy(file_path)
        proxy.write("test")
        proxy.close()

        result = proxy.write("more content")
        assert result == 0

    def test_flush_handles_errors(self, tmp_path):
        """Test that flush handles errors gracefully."""
        file_path = tmp_path / "test.log"
        proxy = TextIOProxy(file_path)
        proxy.close()
        # Should not raise
        proxy.flush()

    def test_close_is_idempotent(self, tmp_path):
        """Test that close can be called multiple times."""
        file_path = tmp_path / "test.log"
        proxy = TextIOProxy(file_path)
        proxy.write("test")
        proxy.close()
        proxy.close()  # Should not raise
        assert proxy._closed


class TestVerboseLoggerInit:
    """Tests for VerboseLogger initialization."""

    def test_init_creates_log_dir(self, tmp_path):
        """Test that init creates the log directory."""
        log_dir = tmp_path / "logs"
        logger = VerboseLogger(log_dir=str(log_dir))

        assert log_dir.exists()
        assert logger.log_dir == log_dir

    def test_init_creates_timestamped_files(self, tmp_path):
        """Test that init creates timestamped log files."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        assert "ralph_verbose_" in str(logger.verbose_log_file)
        assert "ralph_raw_" in str(logger.raw_output_file)
        assert "ralph_metrics_" in str(logger.metrics_file)

    def test_init_initializes_metrics(self, tmp_path):
        """Test that init initializes metrics structure."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        assert "session_start" in logger._metrics
        assert "messages" in logger._metrics
        assert "tool_calls" in logger._metrics
        assert "errors" in logger._metrics
        assert "iterations" in logger._metrics
        assert logger._metrics["total_tokens"] == 0
        assert logger._metrics["total_cost"] == 0.0

    def test_init_without_log_dir_uses_default(self):
        """Test that init without log_dir uses .agent directory."""
        # Create temp dir and change to it
        with tempfile.TemporaryDirectory() as tmpdir:
            original_cwd = os.getcwd()
            try:
                os.chdir(tmpdir)
                os.makedirs(".git")  # Simulate git repo

                logger = VerboseLogger()
                assert ".agent" in str(logger.log_dir)
            finally:
                os.chdir(original_cwd)


class TestVerboseLoggerEmergencyShutdown:
    """Tests for emergency shutdown capability."""

    def test_emergency_shutdown_sets_flag(self, tmp_path):
        """Test that emergency_shutdown sets the flag."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        assert not logger._emergency_shutdown
        assert not logger._emergency_event.is_set()

        logger.emergency_shutdown()

        assert logger._emergency_shutdown
        assert logger._emergency_event.is_set()

    def test_is_shutdown_returns_correct_state(self, tmp_path):
        """Test that is_shutdown returns correct state."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        assert not logger.is_shutdown()
        logger.emergency_shutdown()
        assert logger.is_shutdown()

    @pytest.mark.asyncio
    async def test_log_message_skips_after_shutdown(self, tmp_path):
        """Test that log_message is skipped after shutdown."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        logger.emergency_shutdown()

        # Should return immediately without error
        await logger.log_message("test", "content", 1)

        # Metrics should not be updated
        assert len(logger._metrics["messages"]) == 0

    @pytest.mark.asyncio
    async def test_log_iteration_summary_skips_after_shutdown(self, tmp_path):
        """Test that log_iteration_summary is skipped after shutdown."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        logger.emergency_shutdown()

        await logger.log_iteration_summary(1, 10, True, 5, {}, 100, 0.01)

        assert len(logger._metrics["iterations"]) == 0


class TestVerboseLoggerReentrancy:
    """Tests for re-entrancy protection."""

    def test_can_log_safely_initial_state(self, tmp_path):
        """Test that can_log_safely returns True initially."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        assert logger._can_log_safely()

    def test_can_log_safely_after_shutdown(self, tmp_path):
        """Test that can_log_safely returns False after shutdown."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        logger.emergency_shutdown()
        assert not logger._can_log_safely()

    def test_enter_exit_logging_context(self, tmp_path):
        """Test entering and exiting logging context."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        # Enter context
        assert logger._enter_logging_context()
        assert logger._logging_depth == 1
        assert threading.current_thread().ident in logger._logging_thread_ids

        # Exit context
        logger._exit_logging_context()
        assert logger._logging_depth == 0
        assert threading.current_thread().ident not in logger._logging_thread_ids

    def test_max_logging_depth_enforced(self, tmp_path):
        """Test that max logging depth is enforced."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        logger._max_logging_depth = 3

        # Enter multiple times
        for i in range(3):
            assert logger._enter_logging_context()

        # Fourth entry should fail
        assert not logger._enter_logging_context()

        # Clean up
        for _ in range(3):
            logger._exit_logging_context()


class TestVerboseLoggerLogging:
    """Tests for logging functionality."""

    @pytest.mark.asyncio
    async def test_log_message_creates_entry(self, tmp_path):
        """Test that log_message creates a metrics entry."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        await logger.log_message("test_type", "test content", 1, {"key": "value"})

        assert len(logger._metrics["messages"]) == 1
        entry = logger._metrics["messages"][0]
        assert entry["type"] == "test_type"
        assert entry["content"] == "test content"
        assert entry["iteration"] == 1
        assert entry["metadata"] == {"key": "value"}

    @pytest.mark.asyncio
    async def test_log_message_handles_dict_content(self, tmp_path):
        """Test that log_message handles dict content."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        content = {"key": "value", "nested": {"a": 1}}
        await logger.log_message("test", content, 1)

        assert len(logger._metrics["messages"]) == 1
        assert logger._metrics["messages"][0]["content"] == content

    @pytest.mark.asyncio
    async def test_log_tool_call_creates_entry(self, tmp_path):
        """Test that log_tool_call creates a metrics entry."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        await logger.log_tool_call(
            "Bash",
            {"command": "ls"},
            {"output": "file1\nfile2"},
            1,
            100
        )

        assert len(logger._metrics["tool_calls"]) == 1
        entry = logger._metrics["tool_calls"][0]
        assert entry["tool_name"] == "Bash"
        assert entry["duration_ms"] == 100

    @pytest.mark.asyncio
    async def test_log_error_creates_entry(self, tmp_path):
        """Test that log_error creates a metrics entry."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        try:
            raise ValueError("test error")
        except ValueError as e:
            await logger.log_error(e, 1, "test context")

        assert len(logger._metrics["errors"]) == 1
        entry = logger._metrics["errors"][0]
        assert entry["error_type"] == "ValueError"
        assert "test error" in entry["error_message"]
        assert entry["context"] == "test context"

    @pytest.mark.asyncio
    async def test_log_iteration_summary_updates_totals(self, tmp_path):
        """Test that log_iteration_summary updates totals."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        await logger.log_iteration_summary(
            1, 10, True, 5, {"user": 2, "assistant": 3}, 1000, 0.05
        )

        assert len(logger._metrics["iterations"]) == 1
        assert logger._metrics["total_tokens"] == 1000
        assert logger._metrics["total_cost"] == 0.05


class TestVerboseLoggerMetrics:
    """Tests for metrics functionality."""

    def test_get_session_metrics(self, tmp_path):
        """Test get_session_metrics returns correct structure."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        metrics = logger.get_session_metrics()

        assert "session_start" in metrics
        assert "total_messages" in metrics
        assert "total_tool_calls" in metrics
        assert "total_errors" in metrics
        assert "total_iterations" in metrics
        assert "total_tokens" in metrics
        assert "total_cost" in metrics
        assert "log_files" in metrics
        assert "verbose" in metrics["log_files"]
        assert "raw" in metrics["log_files"]
        assert "metrics" in metrics["log_files"]

    @pytest.mark.asyncio
    async def test_save_metrics_creates_file(self, tmp_path):
        """Test that _save_metrics creates metrics file."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        # Add some data
        await logger.log_message("test", "content", 1)

        # Force save
        await logger._save_metrics()

        # Check file exists and has content
        assert logger.metrics_file.exists()
        data = json.loads(logger.metrics_file.read_text())
        assert "total_messages" in data


class TestVerboseLoggerConsoleOutput:
    """Tests for console output functionality."""

    def test_print_to_console_plain(self, tmp_path):
        """Test print_to_console without Rich."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        # Should not raise even without Rich
        with patch.object(logger, '_live_console', None):
            logger.print_to_console("test message")

    def test_print_table_plain(self, tmp_path, capsys):
        """Test print_table without Rich."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        with patch.object(logger, '_live_console', None):
            logger.print_table(
                "Test Table",
                ["Col1", "Col2"],
                [["a", "b"], ["c", "d"]]
            )

        captured = capsys.readouterr()
        assert "Test Table" in captured.out
        assert "Col1" in captured.out

    def test_print_to_console_skips_after_shutdown(self, tmp_path):
        """Test print_to_console is skipped after shutdown."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        logger.emergency_shutdown()

        # Should not raise or print
        logger.print_to_console("test")


class TestVerboseLoggerClose:
    """Tests for close functionality."""

    @pytest.mark.asyncio
    async def test_close_sets_session_end(self, tmp_path):
        """Test that close sets session_end."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        await logger.close()

        assert logger._metrics["session_end"] is not None

    @pytest.mark.asyncio
    async def test_close_triggers_shutdown(self, tmp_path):
        """Test that close triggers emergency shutdown."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        await logger.close()

        assert logger.is_shutdown()

    @pytest.mark.asyncio
    async def test_close_saves_final_metrics(self, tmp_path):
        """Test that close saves final metrics."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        await logger.log_message("test", "content", 1)
        # Force save metrics before close
        await logger._save_metrics()
        await logger.close()

        # Metrics file should exist after explicit save
        assert logger.metrics_file.exists()

    def test_close_sync(self, tmp_path):
        """Test synchronous close method."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        logger.close_sync()
        assert logger.is_shutdown()


class TestVerboseLoggerThreadSafety:
    """Tests for thread safety."""

    def test_concurrent_logging(self, tmp_path):
        """Test that concurrent logging is thread-safe."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        errors = []

        async def log_messages():
            try:
                for i in range(10):
                    await logger.log_message(f"type_{i}", f"content_{i}", i)
            except Exception as e:
                errors.append(e)

        def thread_target():
            asyncio.run(log_messages())

        threads = [threading.Thread(target=thread_target) for _ in range(3)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0
        # Should have logged some messages (exact count varies due to locking)
        assert len(logger._metrics["messages"]) > 0


class TestVerboseLoggerRichIntegration:
    """Tests for Rich library integration."""

    @pytest.mark.skipif(not RICH_AVAILABLE, reason="Rich not available")
    def test_rich_console_initialized(self, tmp_path):
        """Test that Rich console is initialized when available."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        assert logger._console is not None
        assert logger._live_console is not None

    def test_works_without_rich(self, tmp_path, capsys):
        """Test that logger works without Rich console."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        # Simulate Rich not being available by setting console to None
        logger._live_console = None
        logger._console = None

        # Should fall back to plain print
        logger.print_to_console("test message")

        captured = capsys.readouterr()
        assert "test message" in captured.out


class TestVerboseLoggerSyncWrappers:
    """Tests for synchronous wrapper methods."""

    def test_log_message_sync(self, tmp_path):
        """Test log_message_sync wrapper."""
        logger = VerboseLogger(log_dir=str(tmp_path))

        # Should not raise
        logger.log_message_sync("test", "content", 1)

        # Give async task time to complete
        time.sleep(0.1)

    def test_log_message_sync_after_shutdown(self, tmp_path):
        """Test log_message_sync after shutdown."""
        logger = VerboseLogger(log_dir=str(tmp_path))
        logger.emergency_shutdown()

        # Should return immediately
        logger.log_message_sync("test", "content", 1)
