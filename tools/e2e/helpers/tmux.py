"""Tmux session management for E2E testing."""

import asyncio
import subprocess
from dataclasses import dataclass
from typing import Optional


@dataclass
class TmuxSession:
    """Manages a tmux session for controlled terminal testing.

    Provides async context manager for automatic cleanup.
    """

    name: str
    width: int = 100
    height: int = 30
    _created: bool = False

    async def create(self) -> None:
        """Create a new tmux session with fixed dimensions."""
        cmd = [
            "tmux", "new-session",
            "-d",  # detached
            "-s", self.name,
            "-x", str(self.width),
            "-y", str(self.height),
        ]
        proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        _, stderr = await proc.communicate()

        if proc.returncode != 0:
            raise RuntimeError(f"Failed to create tmux session: {stderr.decode()}")

        self._created = True

    async def send_keys(self, keys: str, enter: bool = True) -> None:
        """Send keys to the tmux session.

        Args:
            keys: The keys/command to send
            enter: Whether to send Enter after the keys
        """
        if not self._created:
            raise RuntimeError("Session not created. Call create() first.")

        cmd = ["tmux", "send-keys", "-t", self.name, keys]
        if enter:
            cmd.append("Enter")

        proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()

    async def capture_pane(self, preserve_ansi: bool = True) -> str:
        """Capture the current visible pane content.

        Note on alternate screen: TUI apps using EnterAlternateScreen (like ratatui)
        render to what tmux considers the "current" visible screen. The -a flag
        captures the alternate buffer (what was shown BEFORE entering alternate screen).
        So for TUI apps, we capture WITHOUT -a to get the actual TUI content.

        Args:
            preserve_ansi: Whether to preserve ANSI escape sequences

        Returns:
            The captured pane content as a string
        """
        if not self._created:
            raise RuntimeError("Session not created. Call create() first.")

        # Capture the current visible content (works for both normal and alternate screen TUIs)
        return await self._capture_with_flags(preserve_ansi, use_alternate=False)

    async def _capture_with_flags(self, preserve_ansi: bool, use_alternate: bool) -> str:
        """Internal helper to capture pane with specific flags."""
        cmd = ["tmux", "capture-pane", "-p", "-t", self.name]
        if preserve_ansi:
            cmd.insert(2, "-e")  # -e preserves escape sequences
        if use_alternate:
            cmd.insert(2, "-a")  # -a captures alternate screen

        proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, _ = await proc.communicate()
        return stdout.decode()

    async def wait_for_alternate_screen(self, timeout: float = 30.0, poll_interval: float = 0.5) -> bool:
        """Wait for a TUI app to start rendering content.

        Detects TUI startup by looking for TUI patterns like [iter N/M] in the
        visible pane content. This works because ratatui TUIs render to the
        current visible screen (not the alternate buffer that -a captures).

        Args:
            timeout: Maximum time to wait in seconds
            poll_interval: How often to check in seconds

        Returns:
            True if TUI content is detected, False if timeout
        """
        import time
        import re
        start = time.time()
        while (time.time() - start) < timeout:
            content = await self.capture_pane(preserve_ansi=False)
            # Look for TUI patterns that indicate Hats TUI is running
            # - [iter N/M] - iteration counter in header
            # - [LIVE] or [REVIEW] - mode indicator
            # - Content that's clearly TUI output (not shell prompt)
            if re.search(r'\[iter\s+\d+(?:/\d+)?\]', content):
                return True
            if re.search(r'\[(LIVE|REVIEW)\]', content):
                return True
            await asyncio.sleep(poll_interval)
        return False

    async def kill(self) -> None:
        """Kill the tmux session."""
        if not self._created:
            return

        cmd = ["tmux", "kill-session", "-t", self.name]
        proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()
        self._created = False

    async def __aenter__(self) -> "TmuxSession":
        """Async context manager entry."""
        await self.create()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        """Async context manager exit - ensures cleanup."""
        await self.kill()

    @staticmethod
    def is_available() -> bool:
        """Check if tmux is available on the system."""
        try:
            result = subprocess.run(
                ["tmux", "-V"],
                capture_output=True,
                text=True,
            )
            return result.returncode == 0
        except FileNotFoundError:
            return False
