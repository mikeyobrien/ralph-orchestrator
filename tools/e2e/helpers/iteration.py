"""Iteration capture utilities for E2E testing."""

import asyncio
import re
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional

from .tmux import TmuxSession


@dataclass
class IterationCaptureResult:
    """Result of capturing TUI state at an iteration boundary."""

    iteration: int
    content: str
    captured_at: datetime = field(default_factory=datetime.now)
    elapsed_time: Optional[str] = None


class IterationCapture:
    """Helper to capture TUI state at iteration boundaries.

    Polls the tmux session for iteration changes and captures
    the TUI state when specific iterations are reached.
    """

    # Pattern to match iteration display: [iter N/M] (e.g., [iter 1/3])
    # The (?:/\d+)? makes the /M part optional for backward compatibility
    ITER_PATTERN = re.compile(r"\[iter\s+(\d+)(?:/\d+)?\]")

    def __init__(
        self,
        session: TmuxSession,
        poll_interval: float = 0.5,
    ):
        """Initialize iteration capture helper.

        Args:
            session: Tmux session to monitor
            poll_interval: How often to poll for iteration changes (seconds)
        """
        self.session = session
        self.poll_interval = poll_interval
        self._last_seen_iteration: int = 0

    async def wait_for_iteration(
        self,
        target_iteration: int,
        timeout: float = 30.0,
    ) -> IterationCaptureResult:
        """Wait until TUI shows the target iteration and capture state.

        Args:
            target_iteration: The iteration number to wait for
            timeout: Maximum time to wait (seconds)

        Returns:
            IterationCaptureResult with captured content

        Raises:
            asyncio.TimeoutError: If iteration not reached within timeout
        """
        start_time = asyncio.get_event_loop().time()

        while True:
            elapsed = asyncio.get_event_loop().time() - start_time
            if elapsed > timeout:
                raise asyncio.TimeoutError(
                    f"Timeout waiting for iteration {target_iteration}. "
                    f"Last seen: {self._last_seen_iteration}"
                )

            content = await self.session.capture_pane()
            current_iter = self._extract_iteration(content)

            if current_iter is not None:
                self._last_seen_iteration = current_iter

                if current_iter >= target_iteration:
                    elapsed_time = self._extract_elapsed_time(content)
                    return IterationCaptureResult(
                        iteration=current_iter,
                        content=content,
                        elapsed_time=elapsed_time,
                    )

            await asyncio.sleep(self.poll_interval)

    async def capture_sequence(
        self,
        iterations: list[int],
        timeout_per: float = 30.0,
    ) -> list[IterationCaptureResult]:
        """Capture TUI state for a sequence of iterations.

        Args:
            iterations: List of iteration numbers to capture
            timeout_per: Timeout for each iteration

        Returns:
            List of IterationCaptureResult, one per requested iteration
        """
        results = []
        for target in sorted(iterations):
            result = await self.wait_for_iteration(target, timeout=timeout_per)
            results.append(result)
        return results

    async def wait_for_process_exit(
        self,
        timeout: float = 60.0,
        check_interval: float = 1.0,
    ) -> tuple[bool, str]:
        """Wait for the Hats process to exit.

        Detects exit by looking for shell prompt return or
        process termination indicators.

        Args:
            timeout: Maximum time to wait
            check_interval: How often to check

        Returns:
            Tuple of (exited: bool, final_content: str)
        """
        start_time = asyncio.get_event_loop().time()
        last_content = ""

        while True:
            elapsed = asyncio.get_event_loop().time() - start_time
            if elapsed > timeout:
                return (False, last_content)

            content = await self.session.capture_pane()
            last_content = content

            # Check for exit indicators
            if self._detect_exit(content):
                return (True, content)

            await asyncio.sleep(check_interval)

    def _extract_iteration(self, content: str) -> Optional[int]:
        """Extract iteration number from TUI content.

        Args:
            content: TUI content to parse

        Returns:
            Iteration number or None if not found
        """
        match = self.ITER_PATTERN.search(content)
        if match:
            return int(match.group(1))
        return None

    def _extract_elapsed_time(self, content: str) -> Optional[str]:
        """Extract elapsed time from TUI header.

        Looks for MM:SS pattern near iteration display.

        Args:
            content: TUI content to parse

        Returns:
            Elapsed time string or None if not found
        """
        # Look for time pattern: MM:SS or HH:MM:SS
        time_pattern = re.compile(r"(\d{1,2}:\d{2}(?::\d{2})?)")
        match = time_pattern.search(content)
        if match:
            return match.group(1)
        return None

    def _detect_exit(self, content: str) -> bool:
        """Detect if the Hats process has exited.

        Args:
            content: TUI content to check

        Returns:
            True if exit detected
        """
        exit_indicators = [
            # Shell prompt patterns
            r"\$\s*$",  # Unix prompt
            r">\s*$",   # Windows prompt
            # Termination messages
            r"Loop terminated",
            r"Session completed",
            r"exited with",
            r"max iterations",
            r"Max iterations reached",
        ]

        for pattern in exit_indicators:
            if re.search(pattern, content, re.MULTILINE | re.IGNORECASE):
                return True

        return False

    @property
    def last_seen_iteration(self) -> int:
        """Get the last seen iteration number."""
        return self._last_seen_iteration
