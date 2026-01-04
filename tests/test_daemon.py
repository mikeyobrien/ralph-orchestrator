# ABOUTME: Tests for daemon process manager
# ABOUTME: TDD tests for Plan 02-01: Process Manager

import unittest
import tempfile
import os
import signal
import time
import subprocess
from pathlib import Path
from unittest.mock import patch, MagicMock


class TestDaemonManager(unittest.TestCase):
    """Tests for DaemonManager - daemon process lifecycle management."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.pid_file = Path(self.temp_dir) / "daemon.pid"

    def tearDown(self):
        """Clean up test fixtures."""
        import shutil
        # Kill any leftover processes (but not ourselves!)
        if self.pid_file.exists():
            try:
                pid = int(self.pid_file.read_text().strip())
                # Don't kill our own process!
                if pid != os.getpid():
                    os.kill(pid, signal.SIGTERM)
            except (ValueError, ProcessLookupError, OSError):
                pass
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_import_daemon_manager(self):
        """Test that DaemonManager can be imported."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        self.assertIsNotNone(DaemonManager)

    def test_init_creates_pid_directory(self):
        """Test that DaemonManager creates PID file directory."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        nested_pid = Path(self.temp_dir) / "nested" / "daemon.pid"
        manager = DaemonManager(pid_file=nested_pid)
        self.assertTrue(nested_pid.parent.exists())

    def test_status_not_running(self):
        """Test status when daemon is not running."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        manager = DaemonManager(pid_file=self.pid_file)
        status = manager.status()
        self.assertFalse(status["running"])
        self.assertNotIn("pid", status)

    def test_status_with_stale_pid_file(self):
        """Test status with stale PID file (process not running)."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        # Write a PID that doesn't exist
        self.pid_file.write_text("99999999")
        manager = DaemonManager(pid_file=self.pid_file)
        status = manager.status()
        self.assertFalse(status["running"])
        # Stale PID file should be cleaned up
        self.assertFalse(self.pid_file.exists())

    def test_write_pid_creates_file(self):
        """Test that _write_pid creates PID file with correct content."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        manager = DaemonManager(pid_file=self.pid_file)
        manager._write_pid()
        self.assertTrue(self.pid_file.exists())
        self.assertEqual(int(self.pid_file.read_text().strip()), os.getpid())

    def test_read_pid_returns_none_when_missing(self):
        """Test that _read_pid returns None when file missing."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        manager = DaemonManager(pid_file=self.pid_file)
        self.assertIsNone(manager._read_pid())

    def test_read_pid_returns_int(self):
        """Test that _read_pid returns integer PID."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        self.pid_file.write_text("12345")
        manager = DaemonManager(pid_file=self.pid_file)
        self.assertEqual(manager._read_pid(), 12345)

    def test_read_pid_handles_invalid_content(self):
        """Test that _read_pid handles invalid content gracefully."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        self.pid_file.write_text("not-a-pid")
        manager = DaemonManager(pid_file=self.pid_file)
        self.assertIsNone(manager._read_pid())

    def test_remove_pid_deletes_file(self):
        """Test that _remove_pid deletes PID file."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        self.pid_file.write_text("12345")
        manager = DaemonManager(pid_file=self.pid_file)
        manager._remove_pid()
        self.assertFalse(self.pid_file.exists())

    def test_remove_pid_handles_missing_file(self):
        """Test that _remove_pid handles missing file gracefully."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        manager = DaemonManager(pid_file=self.pid_file)
        # Should not raise exception
        manager._remove_pid()

    def test_stop_returns_false_when_not_running(self):
        """Test that stop returns False when daemon not running."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        manager = DaemonManager(pid_file=self.pid_file)
        self.assertFalse(manager.stop())

    def test_stop_kills_process_and_removes_pid(self):
        """Test that stop kills the daemon process and removes PID file."""
        from ralph_orchestrator.daemon.manager import DaemonManager

        # Start a simple subprocess that we can stop (sleep 60 seconds)
        proc = subprocess.Popen(
            ["sleep", "60"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )

        # Write PID file
        self.pid_file.write_text(str(proc.pid))

        manager = DaemonManager(pid_file=self.pid_file)
        result = manager.stop()

        self.assertTrue(result)
        self.assertFalse(self.pid_file.exists())

        # Wait for process to actually terminate
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
            self.fail("Process was not terminated by stop()")


class TestDaemonManagerDefaultPaths(unittest.TestCase):
    """Tests for DaemonManager default path handling."""

    def test_default_pid_file_path(self):
        """Test that default PID file is in ~/.ralph/daemon.pid."""
        from ralph_orchestrator.daemon.manager import DaemonManager
        manager = DaemonManager()
        expected = Path.home() / ".ralph" / "daemon.pid"
        self.assertEqual(manager.pid_file, expected)


if __name__ == "__main__":
    unittest.main()
