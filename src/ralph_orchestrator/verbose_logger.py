# ABOUTME: Enhanced verbose logging utilities for Ralph Orchestrator
# ABOUTME: Provides session metrics, emergency shutdown, re-entrancy protection, Rich output

"""Enhanced verbose logging utilities for Ralph."""

import asyncio
import json
import sys
import threading
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional, TextIO, cast

try:
    from rich.console import Console
    from rich.markdown import Markdown
    from rich.syntax import Syntax
    from rich.table import Table
    from rich.panel import Panel
    # Unused imports commented out but kept for future reference:
    # from rich.progress import Progress, SpinnerColumn, TextColumn

    RICH_AVAILABLE = True
except ImportError:
    RICH_AVAILABLE = False
    Console = None  # type: ignore
    Markdown = None  # type: ignore
    Syntax = None  # type: ignore

# Import DiffFormatter for enhanced diff output
try:
    from ralph_orchestrator.output import DiffFormatter
except ImportError:
    DiffFormatter = None  # type: ignore


class TextIOProxy:
    """TextIO proxy that captures Rich console output to a file."""

    def __init__(self, file_path: Path) -> None:
        """
        Initialize TextIO proxy.

        Args:
            file_path: Path to output file
        """
        self.file_path = file_path
        self._file: Optional[TextIO] = None
        self._closed = False
        self._lock = threading.Lock()

    def _ensure_open(self) -> Optional[TextIO]:
        """Ensure file is open, opening lazily if needed."""
        if self._closed:
            return None
        if self._file is None:
            try:
                self._file = open(self.file_path, "a", encoding="utf-8")
            except (OSError, IOError):
                self._closed = True
                return None
        return self._file

    def write(self, text: str) -> int:
        """
        Write text to file.

        Args:
            text: Text to write

        Returns:
            Number of characters written
        """
        with self._lock:
            if self._closed:
                return 0
            try:
                f = self._ensure_open()
                if f is None:
                    return 0
                return f.write(text)
            except (ValueError, OSError, AttributeError):
                return 0

    def flush(self) -> None:
        """Flush file buffer."""
        with self._lock:
            if not self._closed and self._file:
                try:
                    self._file.flush()
                except (ValueError, OSError):
                    pass

    def close(self) -> None:
        """Close file."""
        with self._lock:
            if not self._closed and self._file:
                try:
                    self._file.close()
                except (ValueError, OSError):
                    pass
                finally:
                    self._closed = True
                    self._file = None

    def __del__(self) -> None:
        """Cleanup on deletion."""
        self.close()


class VerboseLogger:
    """
    Enhanced verbose logger that captures detailed output to log files.

    Features:
    - Session metrics tracking in JSON format
    - Emergency shutdown capability
    - Re-entrancy protection (prevent logging loops)
    - Console output with Rich library integration
    - Thread-safe operations

    This logger captures all verbose output including:
    - Claude SDK messages with full content
    - Tool calls and results
    - Console output with formatting preserved
    - System events and status updates
    - Error details and tracebacks
    """

    _metrics: Dict[str, Any]

    def __init__(self, log_dir: Optional[str] = None) -> None:
        """
        Initialize verbose logger with thread safety.

        Args:
            log_dir: Directory to store verbose log files (defaults to .agent in cwd)
        """
        if log_dir is None:
            # Find repository root by looking for .git directory
            current_dir = Path.cwd()
            repo_root = current_dir

            # Walk up to find .git directory or stop at filesystem root
            while repo_root.parent != repo_root:
                if (repo_root / ".git").exists():
                    break
                repo_root = repo_root.parent

            # Create .agent directory in repository root
            log_dir = str(repo_root / ".agent")

        self.log_dir = Path(log_dir)
        self.log_dir.mkdir(parents=True, exist_ok=True)

        # Create timestamped log file for current session
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        self.verbose_log_file = self.log_dir / f"ralph_verbose_{timestamp}.log"
        self.raw_output_file = self.log_dir / f"ralph_raw_{timestamp}.log"
        self.metrics_file = self.log_dir / f"ralph_metrics_{timestamp}.json"

        # Thread safety: Use both asyncio and threading locks
        self._lock = asyncio.Lock()
        self._thread_lock = threading.RLock()  # Re-entrant lock for thread safety

        # Initialize Rich console or fallback
        self._text_io_proxy = TextIOProxy(self.verbose_log_file)
        if RICH_AVAILABLE:
            self._console = Console(file=cast(TextIO, self._text_io_proxy), width=120)
            self._live_console = Console()  # For live terminal output
        else:
            self._console = None
            self._live_console = None

        # Initialize DiffFormatter for enhanced diff output
        if RICH_AVAILABLE and DiffFormatter and self._console:
            self._diff_formatter: Optional[DiffFormatter] = DiffFormatter(self._console)
        else:
            self._diff_formatter = None

        self._raw_file_handle: Optional[TextIO] = None

        # Emergency shutdown state
        self._emergency_shutdown = False
        self._emergency_event = threading.Event()

        # Re-entrancy protection: Track if we're already logging
        self._logging_depth = 0
        self._logging_thread_ids: set = set()
        self._max_logging_depth = 3  # Prevent deep nesting

        # Session metrics tracking
        self._metrics = {
            "session_start": datetime.now().isoformat(),
            "session_end": None,
            "messages": [],
            "tool_calls": [],
            "errors": [],
            "iterations": [],
            "total_tokens": 0,
            "total_cost": 0.0,
        }

    def _can_log_safely(self) -> bool:
        """
        Check if logging is safe to perform (re-entrancy and thread safety check).

        Returns:
            True if logging is safe, False otherwise
        """
        # Check emergency shutdown first
        if self._emergency_event.is_set():
            return False

        # Get current thread ID
        current_thread_id = threading.current_thread().ident

        # Use thread lock for safe access to shared state
        with self._thread_lock:
            # Check if this thread is already in the middle of logging
            if current_thread_id in self._logging_thread_ids:
                # Check nesting depth
                if self._logging_depth >= self._max_logging_depth:
                    return False  # Too deeply nested
                return True  # Allow some nesting

            # Check if any other thread is logging (to prevent excessive blocking)
            if len(self._logging_thread_ids) > 0:
                # Another thread is logging - we can still log but need to be careful
                pass

            # Check async lock state (non-blocking)
            if self._lock.locked():
                return False  # Async lock is held, skip logging

        return True

    def _enter_logging_context(self) -> bool:
        """
        Enter a logging context safely.

        Returns:
            True if we successfully entered the context, False otherwise
        """
        current_thread_id = threading.current_thread().ident

        with self._thread_lock:
            if self._logging_depth >= self._max_logging_depth:
                return False

            if current_thread_id in self._logging_thread_ids:
                self._logging_depth += 1
                return True  # Re-entrancy in same thread is okay with depth tracking

            self._logging_thread_ids.add(current_thread_id)
            self._logging_depth = 1
            return True

    def _exit_logging_context(self) -> None:
        """Exit a logging context safely."""
        current_thread_id = threading.current_thread().ident

        with self._thread_lock:
            self._logging_depth = max(0, self._logging_depth - 1)

            if self._logging_depth == 0:
                self._logging_thread_ids.discard(current_thread_id)

    def emergency_shutdown(self) -> None:
        """Signal emergency shutdown to make logging operations non-blocking."""
        self._emergency_shutdown = True
        self._emergency_event.set()

    def is_shutdown(self) -> bool:
        """Check if emergency shutdown has been triggered."""
        return self._emergency_event.is_set()

    def _print_to_file(self, text: str) -> None:
        """Print text to the log file (Rich or plain)."""
        if self._console and RICH_AVAILABLE:
            self._console.print(text)
        else:
            self._text_io_proxy.write(text + "\n")
            self._text_io_proxy.flush()

    def _print_to_terminal(self, text: str) -> None:
        """Print text to the live terminal (Rich or plain)."""
        if self._live_console and RICH_AVAILABLE:
            self._live_console.print(text)
        else:
            print(text)

    async def log_message(
        self,
        message_type: str,
        content: Any,
        iteration: int = 0,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> None:
        """
        Log a detailed message with rich formatting preserved (thread-safe).

        Args:
            message_type: Type of message (system, assistant, user, tool, etc.)
            content: Message content (text, dict, object)
            iteration: Current iteration number
            metadata: Additional metadata about the message
        """
        # Check if logging is safe (thread safety + re-entrancy)
        if not self._can_log_safely():
            return

        # Enter logging context safely
        if not self._enter_logging_context():
            return

        try:
            # Use non-blocking lock acquisition
            if self._lock.locked():
                return

            async with self._lock:
                timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]

                # Create log entry
                log_entry = {
                    "timestamp": timestamp,
                    "iteration": iteration,
                    "type": message_type,
                    "content": self._serialize_content(content),
                    "metadata": metadata or {},
                }

                # Write to verbose log with rich formatting
                self._print_to_file(f"\n{'='*80}")
                self._print_to_file(
                    f"[{timestamp}] Iteration {iteration} - {message_type}"
                )

                if metadata:
                    self._print_to_file(f"Metadata: {json.dumps(metadata, indent=2)}")

                self._print_to_file(f"{'='*80}\n")

                # Format content based on type
                if isinstance(content, str):
                    if len(content) > 2000:
                        preview = content[:1000]
                        self._print_to_file(preview)
                        self._print_to_file(
                            f"\n[Content truncated ({len(content)} chars total)]"
                        )
                    else:
                        self._print_to_file(content)
                elif isinstance(content, dict):
                    json_str = json.dumps(content, indent=2)
                    self._print_to_file(json_str)
                else:
                    self._print_to_file(str(content))

                # Write to raw log (complete content)
                await self._write_raw_log(log_entry)

                # Update metrics
                await self._update_metrics("message", log_entry)

        except Exception as e:
            try:
                print(f"Logging error in log_message: {e}", file=sys.stderr)
            except Exception:
                pass
        finally:
            self._exit_logging_context()

    def log_message_sync(
        self,
        message_type: str,
        content: Any,
        iteration: int = 0,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> None:
        """Synchronous wrapper for log_message."""
        if self._emergency_event.is_set():
            return

        try:
            asyncio.get_running_loop()  # Check if loop exists (raises RuntimeError if not)
            asyncio.create_task(
                self.log_message(message_type, content, iteration, metadata)
            )
        except RuntimeError:
            # No running loop, run directly
            asyncio.run(self.log_message(message_type, content, iteration, metadata))

    async def log_tool_call(
        self,
        tool_name: str,
        input_data: Any,
        result: Any,
        iteration: int,
        duration_ms: Optional[int] = None,
    ) -> None:
        """
        Log a detailed tool call with input and output (thread-safe).

        Args:
            tool_name: Name of the tool that was called
            input_data: Tool input parameters
            result: Tool execution result
            iteration: Current iteration number
            duration_ms: Tool execution duration in milliseconds
        """
        if not self._can_log_safely():
            return

        if not self._enter_logging_context():
            return

        try:
            if self._lock.locked():
                return

            async with self._lock:
                timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]

                tool_entry = {
                    "timestamp": timestamp,
                    "iteration": iteration,
                    "tool_name": tool_name,
                    "input": self._serialize_content(input_data),
                    "result": self._serialize_content(result),
                    "duration_ms": duration_ms,
                    "success": result is not None,
                }

                # Write formatted tool call to verbose log
                duration_text = f"{duration_ms}ms" if duration_ms else "unknown"
                self._print_to_file(f"\n{'-'*60}")
                self._print_to_file(f"TOOL CALL: {tool_name} ({duration_text})")

                # Format input
                if input_data:
                    self._print_to_file("\nInput:")
                    if isinstance(input_data, (dict, list)):
                        input_json = json.dumps(input_data, indent=2)
                        if len(input_json) > 1000:
                            input_json = (
                                input_json[:500]
                                + "\n  ... [truncated] ...\n"
                                + input_json[-400:]
                            )
                        self._print_to_file(input_json)
                    else:
                        self._print_to_file(str(input_data)[:500])

                # Format result
                if result:
                    self._print_to_file("\nResult:")
                    result_str = self._serialize_content(result)

                    # Check if result is diff content and format with DiffFormatter
                    if (
                        isinstance(result_str, str)
                        and self._is_diff_content(result_str)
                        and self._diff_formatter
                    ):
                        self._print_to_file(
                            "[Detected diff content - formatting with enhanced visualization]"
                        )
                        self._diff_formatter.format_and_print(result_str)
                    elif isinstance(result_str, str) and len(result_str) > 1500:
                        preview = (
                            result_str[:750]
                            + "\n  ... [truncated] ...\n"
                            + result_str[-500:]
                        )
                        self._print_to_file(preview)
                    else:
                        self._print_to_file(str(result_str))

                self._print_to_file(f"{'-'*60}\n")

                await self._write_raw_log(tool_entry)
                await self._update_metrics("tool_call", tool_entry)

        except Exception as e:
            try:
                print(f"Logging error in log_tool_call: {e}", file=sys.stderr)
            except Exception:
                pass
        finally:
            self._exit_logging_context()

    async def log_error(
        self, error: Exception, iteration: int, context: Optional[str] = None
    ) -> None:
        """
        Log detailed error information with traceback (thread-safe).

        Args:
            error: Exception that occurred
            iteration: Current iteration number
            context: Additional context about when the error occurred
        """
        if not self._can_log_safely():
            return

        if not self._enter_logging_context():
            return

        try:
            if self._lock.locked():
                return

            async with self._lock:
                timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]

                error_entry = {
                    "timestamp": timestamp,
                    "iteration": iteration,
                    "error_type": type(error).__name__,
                    "error_message": str(error),
                    "context": context,
                    "traceback": self._get_traceback(error),
                }

                self._print_to_file(f"\n{'!'*20} ERROR DETAILS {'!'*20}")
                self._print_to_file(f"[{timestamp}] Iteration {iteration}")
                self._print_to_file(f"Error Type: {type(error).__name__}")

                if context:
                    self._print_to_file(f"Context: {context}")

                self._print_to_file(f"Message: {str(error)}")

                traceback_str = self._get_traceback(error)
                if traceback_str:
                    self._print_to_file("\nTraceback:")
                    self._print_to_file(traceback_str)

                self._print_to_file(f"{'!'*20} END ERROR {'!'*20}\n")

                await self._write_raw_log(error_entry)
                await self._update_metrics("error", error_entry)

        except Exception as e:
            try:
                print(f"Logging error in log_error: {e}", file=sys.stderr)
            except Exception:
                pass
        finally:
            self._exit_logging_context()

    async def log_iteration_summary(
        self,
        iteration: int,
        duration: int,
        success: bool,
        message_count: int,
        stats: Dict[str, int],
        tokens_used: int = 0,
        cost: float = 0.0,
    ) -> None:
        """
        Log a detailed iteration summary.

        Args:
            iteration: Iteration number
            duration: Duration in seconds
            success: Whether iteration was successful
            message_count: Number of messages exchanged
            stats: Message type statistics
            tokens_used: Number of tokens used
            cost: Cost of this iteration
        """
        if self._emergency_event.is_set():
            return

        if self._lock.locked():
            return

        async with self._lock:
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

            summary_entry = {
                "timestamp": timestamp,
                "iteration": iteration,
                "duration_seconds": duration,
                "success": success,
                "message_count": message_count,
                "stats": stats,
                "tokens_used": tokens_used,
                "cost": cost,
            }

            status_icon = "SUCCESS" if success else "FAILED"

            self._print_to_file(f"\n{'#'*15} ITERATION SUMMARY {'#'*15}")
            self._print_to_file(f"{status_icon} - Iteration {iteration} - {duration}s")
            self._print_to_file(f"Timestamp: {timestamp}")
            self._print_to_file(f"Messages: {message_count}")
            self._print_to_file(f"Tokens: {tokens_used}")
            self._print_to_file(f"Cost: ${cost:.4f}")

            if stats:
                self._print_to_file("\nMessage Statistics:")
                for msg_type, count in stats.items():
                    if count > 0:
                        self._print_to_file(f"  {msg_type}: {count}")

            self._print_to_file(f"{'#'*42}\n")

            await self._write_raw_log(summary_entry)
            await self._update_metrics("iteration", summary_entry)

            # Update total metrics
            self._metrics["total_tokens"] += tokens_used
            self._metrics["total_cost"] += cost

    def _serialize_content(
        self, content: Any
    ) -> str | Dict[Any, Any] | List[Any] | int | float:
        """
        Serialize content to JSON-serializable format.

        Args:
            content: Content to serialize

        Returns:
            Serialized content
        """
        try:
            if isinstance(content, str):
                return content
            elif hasattr(content, "__dict__"):
                if hasattr(content, "text"):
                    return {"text": content.text, "type": type(content).__name__}
                elif hasattr(content, "content"):
                    return {"content": content.content, "type": type(content).__name__}
                else:
                    return {"repr": str(content), "type": type(content).__name__}
            elif isinstance(content, (dict, list)):
                return content
            elif isinstance(content, (int, float, bool)):
                return content
            else:
                return str(content)
        except Exception:
            return f"<unserializable: {type(content).__name__}>"

    def _is_diff_content(self, text: str) -> bool:
        """
        Check if text appears to be diff content.

        Args:
            text: Text to check

        Returns:
            True if text looks like diff output
        """
        if not text or not isinstance(text, str):
            return False

        lines = text.split("\n")
        # Check first few lines for diff indicators
        diff_indicators = [
            any(line.startswith("diff --git") for line in lines[:5]),
            any(line.startswith("--- ") for line in lines[:5]),
            any(line.startswith("+++ ") for line in lines[:5]),
            any(line.startswith("@@") for line in lines[:10]),
            any(
                line.startswith(("+", "-"))
                and not line.startswith(("+++", "---"))
                for line in lines[:10]
            ),
        ]
        return any(diff_indicators)

    async def _write_raw_log(self, entry: Dict[str, Any]) -> None:
        """
        Write entry to raw log file.

        Args:
            entry: Log entry to write
        """
        if self._emergency_event.is_set():
            return

        try:
            if self._raw_file_handle is None:
                try:
                    # Use asyncio.to_thread to avoid blocking the event loop
                    self._raw_file_handle = await asyncio.to_thread(
                        open, self.raw_output_file, "a", encoding="utf-8"
                    )
                except (OSError, IOError):
                    return

            json_line = json.dumps(entry, default=str, ensure_ascii=False) + "\n"

            try:
                await asyncio.wait_for(
                    asyncio.to_thread(self._raw_file_handle.write, json_line),
                    timeout=0.1,
                )
                await asyncio.wait_for(
                    asyncio.to_thread(self._raw_file_handle.flush), timeout=0.1
                )
            except asyncio.TimeoutError:
                return
            except (OSError, IOError):
                if self._raw_file_handle:
                    try:
                        self._raw_file_handle.close()
                    except Exception:
                        pass
                    self._raw_file_handle = None
                return

        except Exception:
            pass

    async def _update_metrics(self, entry_type: str, entry: Dict[str, Any]) -> None:
        """
        Update metrics tracking.

        Args:
            entry_type: Type of entry (message, tool_call, error, iteration)
            entry: The entry data
        """
        try:
            if entry_type == "message":
                self._metrics["messages"].append(entry)
            elif entry_type == "tool_call":
                self._metrics["tool_calls"].append(entry)
            elif entry_type == "error":
                self._metrics["errors"].append(entry)
            elif entry_type == "iteration":
                self._metrics["iterations"].append(entry)

            # Periodically save metrics (every 10 messages)
            if len(self._metrics["messages"]) % 10 == 0:
                await self._save_metrics()

        except Exception:
            pass

    async def _save_metrics(self) -> None:
        """Save metrics to file."""
        if self._emergency_event.is_set():
            return

        try:
            metrics_data = {
                **self._metrics,
                "session_last_update": datetime.now().isoformat(),
                "total_messages": len(self._metrics["messages"]),
                "total_tool_calls": len(self._metrics["tool_calls"]),
                "total_errors": len(self._metrics["errors"]),
                "total_iterations": len(self._metrics["iterations"]),
            }

            if self._lock.locked():
                return

            async with self._lock:
                try:
                    await asyncio.wait_for(
                        asyncio.to_thread(
                            lambda: self.metrics_file.write_text(
                                json.dumps(metrics_data, indent=2, default=str)
                            )
                        ),
                        timeout=0.5,
                    )
                except asyncio.TimeoutError:
                    pass
        except Exception:
            pass

    def _get_traceback(self, error: Exception) -> str:
        """
        Get formatted traceback from exception.

        Args:
            error: Exception to get traceback from

        Returns:
            Formatted traceback string
        """
        import traceback

        try:
            return "".join(
                traceback.format_exception(type(error), error, error.__traceback__)
            )
        except Exception:
            return f"Could not extract traceback: {str(error)}"

    def get_session_metrics(self) -> Dict[str, Any]:
        """
        Get current session metrics.

        Returns:
            Dictionary containing session metrics
        """
        return {
            "session_start": self._metrics["session_start"],
            "session_end": self._metrics.get("session_end"),
            "total_messages": len(self._metrics["messages"]),
            "total_tool_calls": len(self._metrics["tool_calls"]),
            "total_errors": len(self._metrics["errors"]),
            "total_iterations": len(self._metrics["iterations"]),
            "total_tokens": self._metrics["total_tokens"],
            "total_cost": self._metrics["total_cost"],
            "log_files": {
                "verbose": str(self.verbose_log_file),
                "raw": str(self.raw_output_file),
                "metrics": str(self.metrics_file),
            },
        }

    def print_to_console(
        self, message: str, style: Optional[str] = None, panel: bool = False
    ) -> None:
        """
        Print a message to the live console with Rich formatting.

        Args:
            message: Message to print
            style: Rich style string (e.g., "bold red", "green")
            panel: Whether to wrap in a Rich panel
        """
        if self._emergency_event.is_set():
            return

        if self._live_console and RICH_AVAILABLE:
            if panel:
                self._live_console.print(Panel(message))
            elif style:
                self._live_console.print(f"[{style}]{message}[/{style}]")
            else:
                self._live_console.print(message)
        else:
            print(message)

    def print_table(
        self, title: str, columns: List[str], rows: List[List[str]]
    ) -> None:
        """
        Print a formatted table to the console.

        Args:
            title: Table title
            columns: Column headers
            rows: Table data rows
        """
        if self._emergency_event.is_set():
            return

        if self._live_console and RICH_AVAILABLE:
            table = Table(title=title)
            for col in columns:
                table.add_column(col)
            for row in rows:
                table.add_row(*row)
            self._live_console.print(table)
        else:
            # Plain text fallback
            print(f"\n{title}")
            print("-" * 40)
            print(" | ".join(columns))
            print("-" * 40)
            for row in rows:
                print(" | ".join(row))
            print("-" * 40)

    async def close(self) -> None:
        """Close log files and save final metrics."""
        try:
            self._emergency_shutdown = True
            self._emergency_event.set()

            # Update session end time
            self._metrics["session_end"] = datetime.now().isoformat()

            # Save final metrics with timeout
            try:
                await asyncio.wait_for(self._save_metrics(), timeout=1.0)
            except asyncio.TimeoutError:
                pass

            # Close raw file handle
            if self._raw_file_handle:
                try:
                    await asyncio.wait_for(
                        asyncio.to_thread(self._raw_file_handle.close), timeout=0.5
                    )
                except asyncio.TimeoutError:
                    pass
                finally:
                    self._raw_file_handle = None

            # Write session summary
            try:
                if not self._lock.locked():
                    async with self._lock:
                        session_start = self._metrics["session_start"]
                        total_duration = (
                            datetime.now() - datetime.fromisoformat(session_start)
                        ).total_seconds()

                        self._print_to_file(f"\n{'='*80}")
                        self._print_to_file("SESSION SUMMARY")
                        self._print_to_file(f"Duration: {total_duration:.1f} seconds")
                        self._print_to_file(
                            f"Messages: {len(self._metrics['messages'])}"
                        )
                        self._print_to_file(
                            f"Tool Calls: {len(self._metrics['tool_calls'])}"
                        )
                        self._print_to_file(f"Errors: {len(self._metrics['errors'])}")
                        self._print_to_file(
                            f"Iterations: {len(self._metrics['iterations'])}"
                        )
                        self._print_to_file(
                            f"Total Tokens: {self._metrics['total_tokens']}"
                        )
                        self._print_to_file(
                            f"Total Cost: ${self._metrics['total_cost']:.4f}"
                        )
                        self._print_to_file(f"Verbose log: {self.verbose_log_file}")
                        self._print_to_file(f"Raw log: {self.raw_output_file}")
                        self._print_to_file(f"Metrics: {self.metrics_file}")
                        self._print_to_file(f"{'='*80}\n")
            except (RuntimeError, asyncio.TimeoutError):
                pass

            # Close text IO proxy
            self._text_io_proxy.close()

        except Exception as e:
            print(f"Error closing verbose logger: {e}", file=sys.stderr)

    def close_sync(self) -> None:
        """Synchronous close method."""
        try:
            asyncio.get_running_loop()  # Check if loop exists (raises RuntimeError if not)
            asyncio.create_task(self.close())
        except RuntimeError:
            asyncio.run(self.close())
