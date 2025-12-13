# ABOUTME: Advanced async logging with rotation, thread safety, and security features
# ABOUTME: Provides dual interface (async + sync) with unicode sanitization

"""
Advanced async logging with rotation for Ralph Orchestrator.

Features:
- Automatic log rotation at 10MB with 3 backups
- Thread-safe rotation with threading.Lock
- Unicode sanitization for encoding errors
- Security-aware logging (masks sensitive data)
- Dual interface: async methods + sync wrappers
"""

import asyncio
import functools
import shutil
import sys
import threading
import warnings
from datetime import datetime
from pathlib import Path
from typing import Optional

from ralph_orchestrator.security import SecurityValidator


def async_method_warning(func):
    """Decorator to warn when async methods are called without await."""

    @functools.wraps(func)
    def wrapper(self, *args, **kwargs):
        coro = func(self, *args, **kwargs)

        class WarningCoroutine:
            """Wrapper that warns when garbage collected without being awaited."""

            def __init__(self, coro, method_name):
                self._coro = coro
                self._method_name = method_name
                self._warned = False
                self._awaited = False

            def __await__(self):
                self._awaited = True
                return self._coro.__await__()

            def __del__(self):
                if not self._warned and not self._awaited:
                    warnings.warn(
                        f"AsyncFileLogger.{self._method_name}() was called without await. "
                        "The message was not logged. Use 'await logger.{self._method_name}(...)' "
                        f"or 'logger.{self._method_name}_sync(...)' instead.",
                        RuntimeWarning,
                        stacklevel=3,
                    )
                    self._warned = True

            def close(self):
                """Support close() method for compatibility."""
                pass

        return WarningCoroutine(coro, func.__name__)

    return wrapper


class AsyncFileLogger:
    """Async file logger with timestamps, rotation, and security features."""

    # Log rotation constants
    MAX_LOG_SIZE_BYTES = 10 * 1024 * 1024  # 10MB in bytes
    MAX_BACKUP_FILES = 3

    # Default values for log parsing
    DEFAULT_RECENT_LINES_COUNT = 3

    def __init__(self, log_file: str, verbose: bool = False) -> None:
        """
        Initialize async logger.

        Args:
            log_file: Path to log file
            verbose: If True, also print to console

        Raises:
            ValueError: If log_file is None or empty
        """
        if not log_file:
            raise ValueError("log_file cannot be None or empty")

        # Convert to string for validation if it's a Path object
        log_file_str = str(log_file)
        if not log_file_str or not log_file_str.strip():
            raise ValueError("log_file cannot be empty")

        self.log_file = Path(log_file)
        self.verbose = verbose
        self._lock = asyncio.Lock()  # For async methods (single event loop)
        self._thread_lock = threading.Lock()  # For sync methods (multi-threaded)
        self._rotation_lock = threading.Lock()  # Thread safety for file rotation

        # Emergency shutdown flag for graceful signal handling
        self._emergency_shutdown = False
        # Threading event for immediate signal-safe notification
        self._emergency_event = threading.Event()

        # Ensure log directory exists
        self.log_file.parent.mkdir(parents=True, exist_ok=True)

        # Rotate log if needed on startup (single-threaded, safe)
        self._rotate_if_needed()

    def emergency_shutdown(self) -> None:
        """
        Signal emergency shutdown to make logging operations non-blocking.

        This method is signal-safe and can be called from signal handlers.
        After calling this, all logging operations will be skipped to allow
        rapid shutdown without blocking on file I/O.
        """
        self._emergency_shutdown = True
        self._emergency_event.set()

    def is_shutdown(self) -> bool:
        """Check if emergency shutdown has been triggered."""
        return self._emergency_shutdown

    async def log(self, level: str, message: str) -> None:
        """
        Log a message with timestamp.

        Args:
            level: Log level (INFO, SUCCESS, ERROR, WARNING)
            message: Message to log
        """
        # Skip logging during emergency shutdown
        if self._emergency_shutdown:
            return

        # Sanitize the message to handle problematic unicode
        sanitized_message = self._sanitize_unicode(message)

        # Mask sensitive data to prevent security vulnerabilities
        secure_message = SecurityValidator.mask_sensitive_data(sanitized_message)

        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        log_line = f"{timestamp} [{level}] {secure_message}\n"

        async with self._lock:
            # Double-check shutdown flag after acquiring lock
            if self._emergency_shutdown:
                return

            # Write to file
            await asyncio.to_thread(self._write_to_file, log_line)

            # Print to console if verbose
            if self.verbose:
                print(log_line.rstrip())

    def _sanitize_unicode(self, message: str) -> str:
        """
        Sanitize unicode message to prevent encoding errors.

        Args:
            message: Original message

        Returns:
            Sanitized message safe for UTF-8 encoding
        """
        try:
            # Test if the message can be encoded as UTF-8
            message.encode("utf-8")
            return message
        except UnicodeEncodeError:
            # If encoding fails, replace problematic characters
            try:
                # Try to encode with errors='replace' first
                return message.encode("utf-8", errors="replace").decode("utf-8")
            except Exception:
                # If that still fails, use more aggressive sanitization
                sanitized = []
                for char in message:
                    try:
                        char.encode("utf-8")
                        sanitized.append(char)
                    except UnicodeEncodeError:
                        # Replace problematic character with a placeholder
                        sanitized.append("[?]")
                return "".join(sanitized)
        except Exception:
            # For any other unexpected errors, return a safe fallback
            return "[Unicode encoding error]"

    def _write_to_file(self, line: str) -> None:
        """Synchronous file write (called via to_thread)."""
        with open(self.log_file, "a", encoding="utf-8") as f:
            f.write(line)

        # Check if rotation is needed (thread-safe)
        self._rotate_if_needed_thread_safe()

    def _rotate_if_needed_thread_safe(self) -> None:
        """Thread-safe version of _rotate_if_needed() to prevent race conditions."""
        with self._rotation_lock:
            self._rotate_if_needed()

    def _rotate_if_needed(self) -> None:
        """
        Rotate log file if it exceeds max size.

        Note: This method should only be called:
        - During __init__ (single-threaded, safe before logger is shared)
        - From within _rotate_if_needed_thread_safe() when rotation lock is held
        """
        if not self.log_file.exists():
            return

        # Double-check file size with lock held
        try:
            file_size = self.log_file.stat().st_size
        except (OSError, IOError):
            # File might have been moved or deleted by another thread
            return

        if file_size > self.MAX_LOG_SIZE_BYTES:
            # Create a temporary file to ensure atomic rotation
            temp_backup = self.log_file.with_suffix(".log.tmp")

            try:
                # Atomically move current log to temporary backup
                shutil.move(str(self.log_file), str(temp_backup))

                # Rotate backups in reverse order
                for i in range(self.MAX_BACKUP_FILES - 1, 0, -1):
                    old_backup = self.log_file.with_suffix(f".log.{i}")
                    new_backup = self.log_file.with_suffix(f".log.{i + 1}")
                    if old_backup.exists():
                        if new_backup.exists():
                            new_backup.unlink()
                        shutil.move(str(old_backup), str(new_backup))

                # Move temporary backup to .1
                backup = self.log_file.with_suffix(".log.1")
                if backup.exists():
                    backup.unlink()
                shutil.move(str(temp_backup), str(backup))

            except (OSError, IOError):
                # If rotation fails, try to restore from temporary backup
                if temp_backup.exists() and not self.log_file.exists():
                    try:
                        shutil.move(str(temp_backup), str(self.log_file))
                    except (OSError, IOError):
                        # If we can't restore, at least remove the temp file
                        if temp_backup.exists():
                            temp_backup.unlink()

    async def log_info(self, message: str) -> None:
        """Log info message."""
        await self.log("INFO", message)

    async def log_success(self, message: str) -> None:
        """Log success message."""
        await self.log("SUCCESS", message)

    async def log_error(self, message: str) -> None:
        """Log error message."""
        await self.log("ERROR", message)

    async def log_warning(self, message: str) -> None:
        """Log warning message."""
        await self.log("WARNING", message)

    def __del__(self):
        """Destructor to warn about unretrieved coroutines."""
        try:
            # Check if Python is shutting down
            import sys

            if sys.meta_path is None:
                # Python is shutting down, skip cleanup to avoid errors
                return
        except Exception:
            # Silently ignore any errors during destructor to avoid crashes
            pass

    # Synchronous wrapper methods for compatibility
    def _log_sync_direct(self, level: str, message: str) -> None:
        """
        Log a message synchronously using threading.Lock (thread-safe).

        This bypasses asyncio entirely for true multi-threaded safety.
        Includes defensive error handling with stderr fallback.
        """
        if self._emergency_shutdown:
            return

        # Sanitize and secure the message
        sanitized_message = self._sanitize_unicode(message)
        secure_message = SecurityValidator.mask_sensitive_data(sanitized_message)

        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        log_line = f"{timestamp} [{level}] {secure_message}\n"

        try:
            with self._thread_lock:
                if self._emergency_shutdown:
                    return
                self._write_to_file(log_line)
                if self.verbose:
                    print(log_line.rstrip())
        except (PermissionError, OSError, IOError, FileNotFoundError) as e:
            # Fallback to stderr when file I/O fails
            # Truncate message to 200 chars for stderr output
            truncated_message = secure_message[:200]
            try:
                print(
                    f"[LOGGING ERROR] {type(e).__name__}: {e}\n"
                    f"Original message (truncated): {truncated_message}",
                    file=sys.stderr
                )
            except Exception:
                # If stderr also fails, silently ignore to prevent cascading failures
                pass

    def log_info_sync(self, message: str) -> None:
        """Log info message synchronously (thread-safe)."""
        self._log_sync_direct("INFO", message)

    def log_success_sync(self, message: str) -> None:
        """Log success message synchronously (thread-safe)."""
        self._log_sync_direct("SUCCESS", message)

    def log_error_sync(self, message: str) -> None:
        """Log error message synchronously (thread-safe)."""
        self._log_sync_direct("ERROR", message)

    def log_warning_sync(self, message: str) -> None:
        """Log warning message synchronously (thread-safe)."""
        self._log_sync_direct("WARNING", message)

    # Standard logging interface methods for compatibility
    def info(self, message: str) -> None:
        """Standard logging interface - log info message synchronously."""
        self.log_info_sync(message)

    def debug(self, message: str) -> None:
        """Standard logging interface - log debug message synchronously (maps to info)."""
        self.log_info_sync(message)

    def warning(self, message: str) -> None:
        """Standard logging interface - log warning message synchronously."""
        self.log_warning_sync(message)

    def error(self, message: str) -> None:
        """Standard logging interface - log error message synchronously."""
        self.log_error_sync(message)

    def critical(self, message: str) -> None:
        """Standard logging interface - log critical message synchronously (maps to error)."""
        self.log_error_sync(message)

    def get_stats(self) -> dict[str, int | str | None]:
        """
        Get statistics from log file.

        Returns:
            Dict with success_count, error_count, start_time
        """
        if not self.log_file.exists():
            return {"success_count": 0, "error_count": 0, "start_time": None}

        success_count = 0
        error_count = 0
        start_time = None

        with open(self.log_file, encoding="utf-8") as f:
            lines = f.readlines()

            if lines:
                # Extract start time from first line
                first_line = lines[0]
                start_time = first_line.split(" [")[0] if " [" in first_line else None

            # Count successes and errors
            for line in lines:
                if "Iteration" in line and "completed successfully" in line:
                    success_count += 1
                elif "Iteration" in line and "failed" in line:
                    error_count += 1

        return {
            "success_count": success_count,
            "error_count": error_count,
            "start_time": start_time,
        }

    def get_recent_lines(self, count: Optional[int] = None) -> list[str]:
        """
        Get recent log lines.

        Args:
            count: Number of recent lines to return (default: DEFAULT_RECENT_LINES_COUNT)

        Returns:
            List of recent log lines
        """
        if count is None:
            count = self.DEFAULT_RECENT_LINES_COUNT

        if not self.log_file.exists():
            return []

        with open(self.log_file, encoding="utf-8") as f:
            lines = f.readlines()
            return [line.rstrip() for line in lines[-count:]]

    def count_pattern(self, pattern: str) -> int:
        """
        Count occurrences of pattern in log file.

        Args:
            pattern: Pattern to search for

        Returns:
            Number of occurrences
        """
        if not self.log_file.exists():
            return 0

        with open(self.log_file, encoding="utf-8") as f:
            content = f.read()

        return content.count(pattern)

    def get_start_time(self) -> Optional[str]:
        """
        Get start time from first log entry.

        Returns:
            Start time string or None if no logs
        """
        if not self.log_file.exists():
            return None

        with open(self.log_file, encoding="utf-8") as f:
            first_line = f.readline()

        if not first_line:
            return None

        # Extract timestamp from first line (format: "YYYY-MM-DD HH:MM:SS [LEVEL] message")
        parts = first_line.split(" ", 2)
        if len(parts) >= 2:
            return f"{parts[0]} {parts[1]}"

        return None
