# ABOUTME: ACP handlers for permission requests and file/terminal operations
# ABOUTME: Provides permission_mode handling (auto_approve, deny_all, allowlist, interactive)
# ABOUTME: Implements fs/read_text_file and fs/write_text_file handlers with security
# ABOUTME: Implements terminal/* handlers for command execution

"""ACP Handlers for permission requests and agent-to-host operations.

This module provides the ACPHandlers class which manages permission requests
from ACP-compliant agents and handles file operations. It supports:

Permission modes:
- auto_approve: Approve all requests automatically
- deny_all: Deny all requests
- allowlist: Only approve requests matching configured patterns
- interactive: Prompt user for each request (requires terminal)

File operations:
- fs/read_text_file: Read file content with security validation
- fs/write_text_file: Write file content with security validation

Terminal operations:
- terminal/create: Create a new terminal with command
- terminal/output: Read output from a terminal
- terminal/wait_for_exit: Wait for terminal process to exit
- terminal/kill: Kill a terminal process
- terminal/release: Release terminal resources
"""

import fnmatch
import logging
import re
import subprocess
import sys
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Optional

logger = logging.getLogger(__name__)


@dataclass
class Terminal:
    """Represents a terminal subprocess.

    Attributes:
        id: Unique identifier for the terminal.
        process: The subprocess.Popen instance.
        output_buffer: Accumulated output from stdout/stderr.
    """

    id: str
    process: subprocess.Popen
    output_buffer: str = ""

    @property
    def is_running(self) -> bool:
        """Check if the process is still running."""
        return self.process.poll() is None

    @property
    def exit_code(self) -> Optional[int]:
        """Get the exit code if process has exited."""
        return self.process.poll()

    def read_output(self) -> str:
        """Read any available output without blocking.

        Returns:
            New output since last read.
        """
        import select

        new_output = ""

        # Try to read from stdout and stderr
        for stream in [self.process.stdout, self.process.stderr]:
            if stream is None:
                continue

            try:
                # Non-blocking read using select
                while True:
                    ready, _, _ = select.select([stream], [], [], 0)
                    if not ready:
                        break
                    chunk = stream.read(4096)
                    if chunk:
                        new_output += chunk
                    else:
                        break
            except (OSError, IOError) as e:
                logger.debug("Error reading terminal output: %s", e)
                break

        self.output_buffer += new_output
        return new_output

    def kill(self) -> None:
        """Kill the subprocess."""
        if self.is_running:
            self.process.terminate()
            try:
                self.process.wait(timeout=1.0)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()

    def wait(self, timeout: Optional[float] = None) -> int:
        """Wait for the process to exit.

        Args:
            timeout: Maximum time to wait in seconds.

        Returns:
            Exit code of the process.

        Raises:
            subprocess.TimeoutExpired: If timeout is reached.
        """
        return self.process.wait(timeout=timeout)


@dataclass
class PermissionRequest:
    """Represents a permission request from an agent.

    Attributes:
        operation: The operation being requested (e.g., 'fs/read_text_file').
        path: Optional path for filesystem operations.
        command: Optional command for terminal operations.
        arguments: Full arguments dict from the request.
    """

    operation: str
    path: Optional[str] = None
    command: Optional[str] = None
    arguments: dict = field(default_factory=dict)

    @classmethod
    def from_params(cls, params: dict) -> "PermissionRequest":
        """Create PermissionRequest from request parameters.

        Args:
            params: Permission request parameters from agent.

        Returns:
            Parsed PermissionRequest instance.
        """
        return cls(
            operation=params.get("operation", ""),
            path=params.get("path"),
            command=params.get("command"),
            arguments=params,
        )


@dataclass
class PermissionResult:
    """Result of a permission decision.

    Attributes:
        approved: Whether the request was approved.
        reason: Optional reason for the decision.
        mode: Permission mode that made the decision.
    """

    approved: bool
    reason: Optional[str] = None
    mode: str = "unknown"

    def to_dict(self) -> dict:
        """Convert to ACP response format.

        Returns:
            Dict with 'approved' key for ACP response.
        """
        return {"approved": self.approved}


class ACPHandlers:
    """Handles ACP permission requests with configurable modes.

    Supports four permission modes:
    - auto_approve: Always approve (useful for trusted environments)
    - deny_all: Always deny (useful for testing)
    - allowlist: Only approve operations matching configured patterns
    - interactive: Prompt user for each request

    Attributes:
        permission_mode: Current permission mode.
        allowlist: List of allowed operation patterns.
        on_permission_log: Optional callback for logging decisions.
    """

    # Valid permission modes
    VALID_MODES = ("auto_approve", "deny_all", "allowlist", "interactive")

    def __init__(
        self,
        permission_mode: str = "auto_approve",
        permission_allowlist: Optional[list[str]] = None,
        on_permission_log: Optional[Callable[[str], None]] = None,
    ) -> None:
        """Initialize ACPHandlers.

        Args:
            permission_mode: Permission handling mode (default: auto_approve).
            permission_allowlist: List of allowed operation patterns for allowlist mode.
            on_permission_log: Optional callback for logging permission decisions.

        Raises:
            ValueError: If permission_mode is not valid.
        """
        if permission_mode not in self.VALID_MODES:
            raise ValueError(
                f"Invalid permission_mode: {permission_mode}. "
                f"Must be one of: {', '.join(self.VALID_MODES)}"
            )

        self.permission_mode = permission_mode
        self.allowlist = permission_allowlist or []
        self.on_permission_log = on_permission_log

        # Track permission history for debugging
        self._history: list[tuple[PermissionRequest, PermissionResult]] = []

        # Track active terminals
        self._terminals: dict[str, Terminal] = {}

    def handle_request_permission(self, params: dict) -> dict:
        """Handle a permission request from an agent.

        Returns ACP-compliant response with nested outcome structure.
        The agent handles tool execution after receiving permission.

        Args:
            params: Permission request parameters including options list.

        Returns:
            Dict with result.outcome.outcome (selected/cancelled) and optionId.
        """
        request = PermissionRequest.from_params(params)
        result = self._evaluate_permission(request)

        # Log the decision
        self._log_decision(request, result)

        # Store in history
        self._history.append((request, result))

        # Extract options from params to find the appropriate optionId
        options = params.get("options", [])

        if result.approved:
            # Find first "allow" option to use as optionId
            selected_option_id = None
            for option in options:
                if option.get("type") == "allow":
                    selected_option_id = option.get("id")
                    break

            # Fallback to first option if no "allow" type found
            if not selected_option_id and options:
                selected_option_id = options[0].get("id", "proceed_once")
            elif not selected_option_id:
                # Default if no options provided
                selected_option_id = "proceed_once"

            # Return raw result (client wraps in JSON-RPC response)
            return {
                "outcome": {
                    "outcome": "selected",
                    "optionId": selected_option_id
                }
            }
        else:
            # Permission denied - return cancelled outcome
            return {
                "outcome": {
                    "outcome": "cancelled"
                }
            }

    def _evaluate_permission(self, request: PermissionRequest) -> PermissionResult:
        """Evaluate a permission request based on current mode.

        Args:
            request: The permission request to evaluate.

        Returns:
            PermissionResult with decision and reason.
        """
        if self.permission_mode == "auto_approve":
            return PermissionResult(
                approved=True,
                reason="auto_approve mode",
                mode="auto_approve",
            )

        if self.permission_mode == "deny_all":
            return PermissionResult(
                approved=False,
                reason="deny_all mode",
                mode="deny_all",
            )

        if self.permission_mode == "allowlist":
            return self._evaluate_allowlist(request)

        if self.permission_mode == "interactive":
            return self._evaluate_interactive(request)

        # Fallback - should not reach here
        return PermissionResult(
            approved=False,
            reason="unknown mode",
            mode="unknown",
        )

    def _evaluate_allowlist(self, request: PermissionRequest) -> PermissionResult:
        """Evaluate permission against allowlist patterns.

        Patterns can be:
        - Exact match: 'fs/read_text_file'
        - Glob pattern: 'fs/*' (matches any fs operation)
        - Regex pattern: '/^terminal\\/.*$/' (surrounded by slashes)

        Args:
            request: The permission request to evaluate.

        Returns:
            PermissionResult with decision.
        """
        operation = request.operation

        for pattern in self.allowlist:
            if self._matches_pattern(operation, pattern):
                return PermissionResult(
                    approved=True,
                    reason=f"matches allowlist pattern: {pattern}",
                    mode="allowlist",
                )

        return PermissionResult(
            approved=False,
            reason="no matching allowlist pattern",
            mode="allowlist",
        )

    def _matches_pattern(self, operation: str, pattern: str) -> bool:
        """Check if an operation matches a pattern.

        Args:
            operation: The operation name to check.
            pattern: Pattern to match against.

        Returns:
            True if operation matches pattern.
        """
        # Check for regex pattern (surrounded by slashes)
        if pattern.startswith("/") and pattern.endswith("/"):
            try:
                regex_pattern = pattern[1:-1]
                return bool(re.match(regex_pattern, operation))
            except re.error as e:
                logger.warning("Invalid regex pattern '%s' in permission allowlist: %s", pattern, e)
                return False

        # Check for glob pattern
        if "*" in pattern or "?" in pattern:
            return fnmatch.fnmatch(operation, pattern)

        # Exact match
        return operation == pattern

    def _evaluate_interactive(self, request: PermissionRequest) -> PermissionResult:
        """Evaluate permission interactively by prompting user.

        Falls back to deny_all if no terminal is available.

        Args:
            request: The permission request to evaluate.

        Returns:
            PermissionResult with user's decision.
        """
        # Check if we have a terminal
        if not sys.stdin.isatty():
            return PermissionResult(
                approved=False,
                reason="no terminal available for interactive mode",
                mode="interactive",
            )

        # Format the prompt
        prompt = self._format_interactive_prompt(request)

        try:
            print(prompt, file=sys.stderr)
            response = input("[y/N]: ").strip().lower()

            if response in ("y", "yes"):
                return PermissionResult(
                    approved=True,
                    reason="user approved",
                    mode="interactive",
                )
            else:
                return PermissionResult(
                    approved=False,
                    reason="user denied",
                    mode="interactive",
                )

        except (EOFError, KeyboardInterrupt):
            return PermissionResult(
                approved=False,
                reason="input interrupted",
                mode="interactive",
            )

    def _format_interactive_prompt(self, request: PermissionRequest) -> str:
        """Format an interactive permission prompt.

        Args:
            request: The permission request to display.

        Returns:
            Formatted prompt string.
        """
        lines = [
            "",
            "=" * 60,
            f"Permission Request: {request.operation}",
            "=" * 60,
        ]

        if request.path:
            lines.append(f"  Path: {request.path}")
        if request.command:
            lines.append(f"  Command: {request.command}")

        # Add other arguments
        for key, value in request.arguments.items():
            if key not in ("operation", "path", "command"):
                lines.append(f"  {key}: {value}")

        lines.extend([
            "=" * 60,
            "Approve this operation?",
        ])

        return "\n".join(lines)

    def _log_decision(
        self, request: PermissionRequest, result: PermissionResult
    ) -> None:
        """Log a permission decision.

        Args:
            request: The permission request.
            result: The permission decision.
        """
        if self.on_permission_log:
            status = "APPROVED" if result.approved else "DENIED"
            message = (
                f"Permission {status}: {request.operation} "
                f"[mode={result.mode}, reason={result.reason}]"
            )
            self.on_permission_log(message)

    def get_history(self) -> list[tuple[PermissionRequest, PermissionResult]]:
        """Get permission decision history.

        Returns:
            List of (request, result) tuples.
        """
        return self._history.copy()

    def clear_history(self) -> None:
        """Clear permission decision history."""
        self._history.clear()

    def get_approved_count(self) -> int:
        """Get count of approved permissions.

        Returns:
            Number of approved permission requests.
        """
        return sum(1 for _, result in self._history if result.approved)

    def get_denied_count(self) -> int:
        """Get count of denied permissions.

        Returns:
            Number of denied permission requests.
        """
        return sum(1 for _, result in self._history if not result.approved)

    # =========================================================================
    # File Operation Handlers
    # =========================================================================

    def handle_read_file(self, params: dict) -> dict:
        """Handle fs/read_text_file request from agent.

        Reads file content with security validation to prevent path traversal.

        Args:
            params: Request parameters with 'path' key.

        Returns:
            Dict with 'content' on success, or 'error' on failure.
        """
        path_str = params.get("path")

        if not path_str:
            return {"error": {"code": -32602, "message": "Missing required parameter: path"}}

        try:
            # Resolve the path
            path = Path(path_str)

            # Security: require absolute path
            if not path.is_absolute():
                return {
                    "error": {
                        "code": -32602,
                        "message": f"Path must be absolute: {path_str}",
                    }
                }

            # Resolve symlinks and normalize
            resolved_path = path.resolve()

            # Check if file exists - return null content for non-existent files
            # (this allows agents to check file existence without error)
            if not resolved_path.exists():
                return {"content": None, "exists": False}

            # Check if it's a file (not directory)
            if not resolved_path.is_file():
                return {
                    "error": {
                        "code": -32002,
                        "message": f"Path is not a file: {path_str}",
                    }
                }

            # Read file content
            content = resolved_path.read_text(encoding="utf-8")

            return {"content": content}

        except PermissionError:
            return {
                "error": {
                    "code": -32003,
                    "message": f"Permission denied: {path_str}",
                }
            }
        except UnicodeDecodeError:
            return {
                "error": {
                    "code": -32004,
                    "message": f"File is not valid UTF-8 text: {path_str}",
                }
            }
        except OSError as e:
            return {
                "error": {
                    "code": -32000,
                    "message": f"Failed to read file: {e}",
                }
            }

    def handle_write_file(self, params: dict) -> dict:
        """Handle fs/write_text_file request from agent.

        Writes content to file with security validation.

        Args:
            params: Request parameters with 'path' and 'content' keys.

        Returns:
            Dict with 'success: True' on success, or 'error' on failure.
        """
        path_str = params.get("path")
        content = params.get("content")

        if not path_str:
            return {"error": {"code": -32602, "message": "Missing required parameter: path"}}

        if content is None:
            return {"error": {"code": -32602, "message": "Missing required parameter: content"}}

        try:
            # Resolve the path
            path = Path(path_str)

            # Security: require absolute path
            if not path.is_absolute():
                return {
                    "error": {
                        "code": -32602,
                        "message": f"Path must be absolute: {path_str}",
                    }
                }

            # Resolve symlinks and normalize
            resolved_path = path.resolve()

            # Check if path exists and is a directory (can't write to directory)
            if resolved_path.exists() and resolved_path.is_dir():
                return {
                    "error": {
                        "code": -32002,
                        "message": f"Path is a directory: {path_str}",
                    }
                }

            # Create parent directories if needed
            resolved_path.parent.mkdir(parents=True, exist_ok=True)

            # Write file content
            resolved_path.write_text(content, encoding="utf-8")

            return {"success": True}

        except PermissionError:
            return {
                "error": {
                    "code": -32003,
                    "message": f"Permission denied: {path_str}",
                }
            }
        except OSError as e:
            return {
                "error": {
                    "code": -32000,
                    "message": f"Failed to write file: {e}",
                }
            }

    # =========================================================================
    # Terminal Operation Handlers
    # =========================================================================

    def handle_terminal_create(self, params: dict) -> dict:
        """Handle terminal/create request from agent.

        Creates a new terminal subprocess for command execution.

        Args:
            params: Request parameters with 'command' (list of strings) and
                   optional 'cwd' (working directory).

        Returns:
            Dict with 'terminalId' on success, or 'error' on failure.
        """
        command = params.get("command")

        if command is None:
            return {
                "error": {
                    "code": -32602,
                    "message": "Missing required parameter: command",
                }
            }

        if not isinstance(command, list):
            return {
                "error": {
                    "code": -32602,
                    "message": "command must be a list of strings",
                }
            }

        if len(command) == 0:
            return {
                "error": {
                    "code": -32602,
                    "message": "command list cannot be empty",
                }
            }

        cwd = params.get("cwd")

        try:
            # Create subprocess with pipes for stdout/stderr
            process = subprocess.Popen(
                command,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                stdin=subprocess.DEVNULL,
                cwd=cwd,
                text=True,
                bufsize=0,
            )

            # Generate unique terminal ID
            terminal_id = str(uuid.uuid4())

            # Create terminal instance
            terminal = Terminal(id=terminal_id, process=process)
            self._terminals[terminal_id] = terminal

            return {"terminalId": terminal_id}

        except FileNotFoundError:
            return {
                "error": {
                    "code": -32001,
                    "message": f"Command not found: {command[0]}",
                }
            }
        except PermissionError:
            return {
                "error": {
                    "code": -32003,
                    "message": f"Permission denied executing: {command[0]}",
                }
            }
        except OSError as e:
            return {
                "error": {
                    "code": -32000,
                    "message": f"Failed to create terminal: {e}",
                }
            }

    def handle_terminal_output(self, params: dict) -> dict:
        """Handle terminal/output request from agent.

        Reads available output from a terminal.

        Args:
            params: Request parameters with 'terminalId'.

        Returns:
            Dict with 'output' and 'done' on success, or 'error' on failure.
        """
        terminal_id = params.get("terminalId")

        if not terminal_id:
            return {
                "error": {
                    "code": -32602,
                    "message": "Missing required parameter: terminalId",
                }
            }

        terminal = self._terminals.get(terminal_id)
        if not terminal:
            return {
                "error": {
                    "code": -32001,
                    "message": f"Terminal not found: {terminal_id}",
                }
            }

        # Read any new output
        terminal.read_output()

        return {
            "output": terminal.output_buffer,
            "done": not terminal.is_running,
        }

    def handle_terminal_wait_for_exit(self, params: dict) -> dict:
        """Handle terminal/wait_for_exit request from agent.

        Waits for a terminal process to exit.

        Args:
            params: Request parameters with 'terminalId' and optional 'timeout'.

        Returns:
            Dict with 'exitCode' on success, or 'error' on failure/timeout.
        """
        terminal_id = params.get("terminalId")

        if not terminal_id:
            return {
                "error": {
                    "code": -32602,
                    "message": "Missing required parameter: terminalId",
                }
            }

        terminal = self._terminals.get(terminal_id)
        if not terminal:
            return {
                "error": {
                    "code": -32001,
                    "message": f"Terminal not found: {terminal_id}",
                }
            }

        timeout = params.get("timeout")

        try:
            exit_code = terminal.wait(timeout=timeout)
            # Read any remaining output
            terminal.read_output()
            return {"exitCode": exit_code}

        except subprocess.TimeoutExpired:
            return {
                "error": {
                    "code": -32000,
                    "message": f"Wait timed out after {timeout}s",
                }
            }

    def handle_terminal_kill(self, params: dict) -> dict:
        """Handle terminal/kill request from agent.

        Kills a terminal process.

        Args:
            params: Request parameters with 'terminalId'.

        Returns:
            Dict with 'success: True' on success, or 'error' on failure.
        """
        terminal_id = params.get("terminalId")

        if not terminal_id:
            return {
                "error": {
                    "code": -32602,
                    "message": "Missing required parameter: terminalId",
                }
            }

        terminal = self._terminals.get(terminal_id)
        if not terminal:
            return {
                "error": {
                    "code": -32001,
                    "message": f"Terminal not found: {terminal_id}",
                }
            }

        terminal.kill()
        return {"success": True}

    def handle_terminal_release(self, params: dict) -> dict:
        """Handle terminal/release request from agent.

        Releases terminal resources, killing the process if still running.

        Args:
            params: Request parameters with 'terminalId'.

        Returns:
            Dict with 'success: True' on success, or 'error' on failure.
        """
        terminal_id = params.get("terminalId")

        if not terminal_id:
            return {
                "error": {
                    "code": -32602,
                    "message": "Missing required parameter: terminalId",
                }
            }

        terminal = self._terminals.get(terminal_id)
        if not terminal:
            return {
                "error": {
                    "code": -32001,
                    "message": f"Terminal not found: {terminal_id}",
                }
            }

        # Kill the process if still running before releasing
        if terminal.is_running:
            try:
                terminal.kill()
            except Exception as e:
                logger.warning("Failed to kill terminal %s during release: %s", terminal_id, e)

        # Clean up the terminal
        del self._terminals[terminal_id]
        return {"success": True}
