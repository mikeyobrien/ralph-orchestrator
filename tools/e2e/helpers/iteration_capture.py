"""Iteration state capture helper for E2E testing.

Provides utilities to capture TUI state at iteration boundaries
by polling for [iter N/M] pattern changes.
"""

import asyncio
import re
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional

from .tmux import TmuxSession


@dataclass
class IterationState:
    """Captured state at a specific iteration."""

    iteration: int
    content: str
    elapsed_time: Optional[str] = None
    mode: Optional[str] = None
    captured_at: datetime = field(default_factory=datetime.now)

    @classmethod
    def from_content(cls, content: str, expected_iter: int) -> "IterationState":
        """Parse iteration state from captured content.

        Args:
            content: Raw TUI content
            expected_iter: Expected iteration number

        Returns:
            IterationState with parsed fields
        """
        # Extract iteration number from [iter N/M] pattern (e.g., [iter 1/3])
        iter_match = re.search(r'\[iter\s+(\d+)(?:/\d+)?\]', content)
        iteration = int(iter_match.group(1)) if iter_match else expected_iter

        # Extract elapsed time (MM:SS format)
        time_match = re.search(r'(\d{1,2}:\d{2})', content)
        elapsed_time = time_match.group(1) if time_match else None

        # Extract mode (auto, interactive, etc.)
        mode_match = re.search(r'[▶►]\s*(auto|interactive|observe)', content, re.IGNORECASE)
        mode = mode_match.group(1).lower() if mode_match else None

        return cls(
            iteration=iteration,
            content=content,
            elapsed_time=elapsed_time,
            mode=mode,
        )


@dataclass
class CaptureSequenceResult:
    """Result of capturing multiple iterations."""

    states: list[IterationState] = field(default_factory=list)
    final_exit_code: Optional[int] = None
    timed_out: bool = False

    @property
    def iterations_captured(self) -> int:
        """Number of iterations successfully captured."""
        return len(self.states)

    def get_iteration(self, n: int) -> Optional[IterationState]:
        """Get state for specific iteration number."""
        for state in self.states:
            if state.iteration == n:
                return state
        return None


class IterationCapture:
    """Helper to capture TUI state at iteration boundaries.

    Polls the tmux session for [iter N/M] pattern changes
    and captures state at each transition.
    """

    def __init__(
        self,
        session: TmuxSession,
        poll_interval: float = 0.5,
        capture_delay: float = 0.2,
    ):
        """Initialize the capture helper.

        Args:
            session: TmuxSession to capture from
            poll_interval: How often to poll for changes (seconds)
            capture_delay: Delay after detecting iteration change (seconds)
        """
        self.session = session
        self.poll_interval = poll_interval
        self.capture_delay = capture_delay
        self._last_iteration: Optional[int] = None

    async def wait_for_iteration(
        self,
        n: int,
        timeout: float = 60.0,
        debug: bool = False,
    ) -> Optional[IterationState]:
        """Wait until TUI shows [iter N] and capture state.

        Args:
            n: Target iteration number
            timeout: Maximum time to wait (seconds)
            debug: If True, print debug info about captured content

        Returns:
            IterationState if found within timeout, None otherwise
        """
        start_time = asyncio.get_event_loop().time()
        capture_count = 0

        while (asyncio.get_event_loop().time() - start_time) < timeout:
            content = await self.session.capture_pane()
            capture_count += 1

            if debug and capture_count <= 3:
                # Print first few captures for debugging (show first 3 lines)
                lines = content.split('\n')[:3] if content else ["(empty)"]
                for i, line in enumerate(lines):
                    print(f"[DEBUG] Capture #{capture_count}, line {i}: {line[:100]}")

            # Check for iteration pattern [iter N/M] (e.g., [iter 1/3])
            match = re.search(r'\[iter\s+(\d+)(?:/\d+)?\]', content)
            if match:
                current_iter = int(match.group(1))
                if debug:
                    print(f"[DEBUG] Found iteration {current_iter}, target {n}")

                if current_iter >= n:
                    # Wait a bit for TUI to stabilize
                    await asyncio.sleep(self.capture_delay)
                    # Re-capture after stabilization
                    content = await self.session.capture_pane()
                    return IterationState.from_content(content, n)

            await asyncio.sleep(self.poll_interval)

        if debug:
            print(f"[DEBUG] Timeout after {capture_count} captures. Last content first line:")
            first_line = content.split('\n')[0] if content else "(empty)"
            print(f"[DEBUG] {first_line[:80]}")

        return None

    async def capture_sequence(
        self,
        max_iter: int,
        timeout_per_iter: float = 60.0,
        total_timeout: float = 300.0,
    ) -> CaptureSequenceResult:
        """Capture TUI state for iterations 1 through max_iter.

        Args:
            max_iter: Maximum iteration to capture
            timeout_per_iter: Timeout for each iteration (seconds)
            total_timeout: Total timeout for entire sequence (seconds)

        Returns:
            CaptureSequenceResult with all captured states
        """
        result = CaptureSequenceResult()
        start_time = asyncio.get_event_loop().time()

        for target_iter in range(1, max_iter + 1):
            # Check total timeout
            elapsed = asyncio.get_event_loop().time() - start_time
            if elapsed >= total_timeout:
                result.timed_out = True
                break

            # Calculate remaining time for this iteration
            remaining = min(timeout_per_iter, total_timeout - elapsed)

            state = await self.wait_for_iteration(target_iter, timeout=remaining)
            if state:
                result.states.append(state)
                self._last_iteration = target_iter
            else:
                # Couldn't capture this iteration - might be done
                break

        return result

    async def wait_for_termination(
        self,
        timeout: float = 120.0,
        poll_interval: float = 1.0,
    ) -> tuple[Optional[str], bool]:
        """Wait for Hats process to terminate.

        Captures the last TUI content (from alternate screen) before exit,
        rather than returning the shell prompt after exit.

        Args:
            timeout: Maximum time to wait (seconds)
            poll_interval: How often to check (seconds)

        Returns:
            Tuple of (final_tui_content, terminated)
        """
        start_time = asyncio.get_event_loop().time()
        last_tui_content = ""  # Track last TUI content (alternate screen)
        last_content = ""
        stable_count = 0
        required_stable = 3  # Require 3 consecutive identical captures

        while (asyncio.get_event_loop().time() - start_time) < timeout:
            content = await self.session.capture_pane()

            # If we got meaningful TUI content (not "no alternate screen" and not shell prompt),
            # save it as the last TUI content
            if content.strip() and not content.strip() == "no alternate screen":
                if not re.search(r'\$\s*$', content.strip()):
                    last_tui_content = content

            # Check for shell prompt (indicates process ended - TUI has exited)
            if re.search(r'\$\s*$', content.strip()):
                # Return the last TUI content we captured, not the shell prompt
                return last_tui_content if last_tui_content else content, True

            # Check for termination messages in TUI
            termination_patterns = [
                r'max iterations',
                r'Max iterations reached',
                r'Loop terminated',
                r'Session completed',
            ]
            for pattern in termination_patterns:
                if re.search(pattern, content, re.IGNORECASE):
                    return content, True

            # Check for stability (no changes)
            if content == last_content:
                stable_count += 1
                if stable_count >= required_stable:
                    return content, True
            else:
                stable_count = 0
                last_content = content

            await asyncio.sleep(poll_interval)

        return last_tui_content if last_tui_content else last_content, False

    async def wait_for_process_exit(
        self,
        timeout: float = 120.0,
        check_interval: float = 1.0,
    ) -> tuple[bool, str]:
        """Wait for Hats process to exit.

        Alias for wait_for_termination with swapped return order for compatibility.

        Args:
            timeout: Maximum time to wait (seconds)
            check_interval: How often to check (seconds)

        Returns:
            Tuple of (exited, final_content)
        """
        content, terminated = await self.wait_for_termination(timeout, check_interval)
        return terminated, content or ""

    def extract_exit_code(self, content: str) -> Optional[int]:
        """Extract exit code from terminal content.

        Looks for common patterns like "exit code: N" or "$? = N".

        Args:
            content: Terminal content to search

        Returns:
            Exit code if found, None otherwise
        """
        # Look for explicit exit code mentions
        patterns = [
            r'exit\s+code[:\s]+(\d+)',
            r'exited?\s+with\s+(?:code\s+)?(\d+)',
            r'\$\?\s*[=:]\s*(\d+)',
            r'return(?:ed)?\s+(\d+)',
        ]

        for pattern in patterns:
            match = re.search(pattern, content, re.IGNORECASE)
            if match:
                return int(match.group(1))

        return None
