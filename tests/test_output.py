# ABOUTME: Tests for the output module
# ABOUTME: Tests DiffStats, DiffFormatter, and RalphConsole classes

"""Tests for the output module."""

import pytest
from unittest.mock import patch

from ralph_orchestrator.output import (
    DiffStats,
    DiffFormatter,
    RalphConsole,
    RICH_AVAILABLE,
)


class TestDiffStats:
    """Tests for DiffStats dataclass."""

    def test_default_values(self):
        """Test that DiffStats has sensible defaults."""
        stats = DiffStats()
        assert stats.additions == 0
        assert stats.deletions == 0
        assert stats.files == 0
        assert stats.files_changed == {}

    def test_custom_values(self):
        """Test that DiffStats can be initialized with custom values."""
        stats = DiffStats(additions=10, deletions=5, files=3)
        assert stats.additions == 10
        assert stats.deletions == 5
        assert stats.files == 3

    def test_files_changed_tracking(self):
        """Test that files_changed dict works properly."""
        stats = DiffStats()
        stats.files_changed["test.py"] = (5, 2)
        stats.files_changed["main.py"] = (10, 0)
        assert stats.files_changed["test.py"] == (5, 2)
        assert stats.files_changed["main.py"] == (10, 0)


@pytest.mark.skipif(not RICH_AVAILABLE, reason="Rich not installed")
class TestDiffFormatter:
    """Tests for DiffFormatter class."""

    def test_init(self):
        """Test DiffFormatter initialization."""
        from rich.console import Console

        console = Console()
        formatter = DiffFormatter(console)
        assert formatter.console is console

    def test_calculate_stats_empty(self):
        """Test stats calculation with empty diff."""
        from rich.console import Console

        console = Console()
        formatter = DiffFormatter(console)
        stats = formatter._calculate_stats([])
        assert stats.additions == 0
        assert stats.deletions == 0
        assert stats.files == 0

    def test_calculate_stats_with_changes(self):
        """Test stats calculation with actual diff lines."""
        from rich.console import Console

        console = Console()
        formatter = DiffFormatter(console)
        lines = [
            "diff --git a/test.py b/test.py",
            "--- a/test.py",
            "+++ b/test.py",
            "@@ -1,3 +1,4 @@",
            "+# New comment",
            " def hello():",
            "-    pass",
            "+    print('hello')",
        ]
        stats = formatter._calculate_stats(lines)
        assert stats.additions == 2
        assert stats.deletions == 1
        assert stats.files == 1

    def test_extract_filename(self):
        """Test filename extraction from diff header."""
        from rich.console import Console

        console = Console()
        formatter = DiffFormatter(console)
        filename = formatter._extract_filename("diff --git a/src/test.py b/src/test.py")
        assert filename == "src/test.py"

    def test_extract_filename_simple(self):
        """Test filename extraction with simple path."""
        from rich.console import Console

        console = Console()
        formatter = DiffFormatter(console)
        filename = formatter._extract_filename("diff --git a/test.py b/test.py")
        assert filename == "test.py"

    def test_is_binary_file(self):
        """Test binary file detection."""
        from rich.console import Console

        console = Console()
        formatter = DiffFormatter(console)
        assert formatter._is_binary_file("diff --git a/image.png b/image.png")
        assert formatter._is_binary_file("diff --git a/archive.zip b/archive.zip")
        assert not formatter._is_binary_file("diff --git a/test.py b/test.py")
        assert not formatter._is_binary_file("diff --git a/README.md b/README.md")

    def test_format_hunk_header(self):
        """Test hunk header formatting."""
        from rich.console import Console

        console = Console()
        formatter = DiffFormatter(console)
        result = formatter._format_hunk_header("@@ -140,7 +140,8 @@ class Foo:")
        assert "Lines 140-" in result
        assert "class Foo:" in result


class TestRalphConsole:
    """Tests for RalphConsole class."""

    def test_init(self):
        """Test RalphConsole initialization."""
        rc = RalphConsole()
        if RICH_AVAILABLE:
            assert rc.console is not None
            assert rc.diff_formatter is not None
        else:
            assert rc.console is None
            assert rc.diff_formatter is None

    def test_print_status(self, capsys):
        """Test status message printing."""
        rc = RalphConsole()
        rc.print_status("Test message")
        # Output varies depending on Rich availability

    def test_print_success(self, capsys):
        """Test success message printing."""
        rc = RalphConsole()
        rc.print_success("Success message")

    def test_print_error(self, capsys):
        """Test error message printing."""
        rc = RalphConsole()
        rc.print_error("Error message")
        rc.print_error("Critical error", severity="critical")
        rc.print_error("Warning message", severity="warning")

    def test_print_warning(self, capsys):
        """Test warning message printing."""
        rc = RalphConsole()
        rc.print_warning("Warning message")

    def test_print_info(self, capsys):
        """Test info message printing."""
        rc = RalphConsole()
        rc.print_info("Info message")

    def test_is_diff_content_detection(self):
        """Test diff content detection."""
        rc = RalphConsole()

        # Should detect as diff
        diff_text = """diff --git a/test.py b/test.py
--- a/test.py
+++ b/test.py
@@ -1,3 +1,4 @@
+# Comment
 def hello():
"""
        assert rc._is_diff_content(diff_text)

        # Should not detect as diff
        assert not rc._is_diff_content("Hello world")
        assert not rc._is_diff_content("Just a normal string")

    def test_is_markdown_table(self):
        """Test markdown table detection."""
        rc = RalphConsole()

        table_text = """| Column 1 | Column 2 |
|----------|----------|
| Value 1  | Value 2  |"""
        assert rc._is_markdown_table(table_text)

        assert not rc._is_markdown_table("Not a table")

    def test_is_markdown_content(self):
        """Test markdown content detection."""
        rc = RalphConsole()

        markdown_text = """# Heading

- List item 1
- List item 2

**Bold text**
"""
        assert rc._is_markdown_content(markdown_text)

        # Single indicator should not trigger (threshold is 2)
        assert not rc._is_markdown_content("# Just a heading")

    def test_is_error_traceback(self):
        """Test error traceback detection."""
        rc = RalphConsole()

        traceback_text = """Traceback (most recent call last):
  File "test.py", line 10, in <module>
    raise ValueError("test")
ValueError: test"""
        assert rc._is_error_traceback(traceback_text)

        assert not rc._is_error_traceback("Not an error")

    def test_preprocess_markdown(self):
        """Test markdown preprocessing."""
        rc = RalphConsole()

        # Test task list conversion
        result = rc._preprocess_markdown("- [ ] Todo item")
        assert "☐" in result

        result = rc._preprocess_markdown("- [x] Done item")
        assert "☑" in result

    def test_countdown_bar_calculation(self):
        """Test countdown progress bar calculation."""
        rc = RalphConsole()

        # Test that progress calculation works
        remaining = 5
        total = 10
        progress = (total - remaining) / total
        filled = int(rc.PROGRESS_BAR_WIDTH * progress)

        assert filled == 15  # Half of 30 width bar

    def test_countdown_bar_zero_total(self):
        """Test countdown bar handles zero total gracefully (no ZeroDivisionError)."""
        rc = RalphConsole()

        # Should not raise ZeroDivisionError
        rc.print_countdown(remaining=0, total=0)
        rc.print_countdown(remaining=5, total=0)
        rc.print_countdown(remaining=0, total=-1)


class TestRalphConsoleWithoutRich:
    """Tests for RalphConsole fallback behavior."""

    @patch("ralph_orchestrator.output.console.RICH_AVAILABLE", False)
    @patch("ralph_orchestrator.output.console.Console", None)
    def test_init_without_rich(self):
        """Test initialization without Rich available."""
        # Re-import to get patched version
        from ralph_orchestrator.output import console

        # Create a mock module state
        original_rich = console.RICH_AVAILABLE
        console.RICH_AVAILABLE = False

        try:
            RalphConsole()
            # Should have fallback behavior
        finally:
            console.RICH_AVAILABLE = original_rich
