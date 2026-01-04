# ABOUTME: Daemon process manager for background orchestration
# ABOUTME: Enables ralph run --daemon to return immediately

import os
import sys
import signal
import atexit
from pathlib import Path
from typing import Optional, Callable, Any


class DaemonManager:
    """Manages Ralph daemon process lifecycle.

    This class provides functionality to:
    - Start a function as a background daemon process (double-fork pattern)
    - Stop a running daemon via SIGTERM
    - Check daemon status via PID file
    - Manage PID file lifecycle

    The double-fork pattern is used to properly detach from the controlling terminal,
    which is the standard Unix daemon pattern.
    """

    def __init__(self, pid_file: Optional[Path] = None):
        """Initialize DaemonManager.

        Args:
            pid_file: Path to PID file. Defaults to ~/.ralph/daemon.pid
        """
        self.pid_file = pid_file or Path.home() / ".ralph" / "daemon.pid"
        self.pid_file.parent.mkdir(parents=True, exist_ok=True)

    def start(self, func: Callable[..., Any], *args: Any, **kwargs: Any) -> None:
        """Start function as daemon process using double-fork pattern.

        The parent process returns immediately after forking,
        while the child process runs the function in the background.

        Args:
            func: Function to run as daemon
            *args: Positional arguments to pass to func
            **kwargs: Keyword arguments to pass to func
        """
        # First fork - parent returns immediately
        pid = os.fork()
        if pid > 0:
            return  # Parent returns immediately

        # First child - become session leader
        os.setsid()

        # Second fork - detach from controlling terminal
        pid = os.fork()
        if pid > 0:
            os._exit(0)  # First child exits

        # We are now the daemon process (second child)

        # Redirect standard file descriptors to /dev/null
        sys.stdout.flush()
        sys.stderr.flush()

        with open('/dev/null', 'r') as devnull:
            os.dup2(devnull.fileno(), sys.stdin.fileno())

        # Write PID file
        self._write_pid()
        atexit.register(self._remove_pid)

        # Set up signal handlers for graceful shutdown
        signal.signal(signal.SIGTERM, self._signal_handler)
        signal.signal(signal.SIGINT, self._signal_handler)

        # Run the actual function
        try:
            func(*args, **kwargs)
        except Exception:
            pass  # Daemon should not crash loudly
        finally:
            self._remove_pid()
            os._exit(0)

    def stop(self) -> bool:
        """Stop running daemon.

        Sends SIGTERM to the daemon process and removes PID file.

        Returns:
            True if daemon was stopped, False if not running
        """
        pid = self._read_pid()
        if not pid:
            return False

        try:
            os.kill(pid, signal.SIGTERM)
            self._remove_pid()
            return True
        except ProcessLookupError:
            self._remove_pid()
            return False
        except OSError:
            self._remove_pid()
            return False

    def status(self) -> dict:
        """Get daemon status.

        Returns:
            Dict with 'running' bool and optional 'pid' int
        """
        pid = self._read_pid()
        if not pid:
            return {"running": False}

        try:
            os.kill(pid, 0)  # Check if process exists (signal 0)
            return {"running": True, "pid": pid}
        except ProcessLookupError:
            self._remove_pid()
            return {"running": False}
        except OSError:
            # Process exists but we can't signal it (permission issue)
            return {"running": True, "pid": pid}

    def _write_pid(self) -> None:
        """Write current process PID to file."""
        self.pid_file.write_text(str(os.getpid()))

    def _read_pid(self) -> Optional[int]:
        """Read PID from file.

        Returns:
            PID as int, or None if file missing or invalid
        """
        if not self.pid_file.exists():
            return None
        try:
            return int(self.pid_file.read_text().strip())
        except ValueError:
            return None

    def _remove_pid(self) -> None:
        """Remove PID file if it exists."""
        if self.pid_file.exists():
            try:
                self.pid_file.unlink()
            except OSError:
                pass  # Ignore errors on cleanup

    def _signal_handler(self, signum: int, frame: Any) -> None:
        """Handle termination signals gracefully."""
        self._remove_pid()
        sys.exit(0)
