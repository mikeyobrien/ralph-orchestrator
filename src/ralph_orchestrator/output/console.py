# ABOUTME: Colored terminal output utilities using Rich
# ABOUTME: Provides DiffFormatter, DiffStats, and RalphConsole for enhanced CLI output

"""Colored terminal output utilities using Rich."""

import re
from dataclasses import dataclass, field
from typing import Optional

# Try to import Rich components with fallback
try:
    from rich.console import Console
    from rich.markdown import Markdown
    from rich.markup import escape
    from rich.panel import Panel
    from rich.syntax import Syntax
    from rich.table import Table

    RICH_AVAILABLE = True
except ImportError:
    RICH_AVAILABLE = False
    Console = None  # type: ignore
    Markdown = None  # type: ignore
    Panel = None  # type: ignore
    Syntax = None  # type: ignore
    Table = None  # type: ignore

    def escape(x: str) -> str:
        """Fallback escape function."""
        return str(x).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


@dataclass
class DiffStats:
    """Statistics for diff content."""

    additions: int = 0
    deletions: int = 0
    files: int = 0
    files_changed: dict[str, tuple[int, int]] = field(
        default_factory=dict
    )  # filename -> (additions, deletions)


class DiffFormatter:
    """Formatter for enhanced diff visualization."""

    # Diff display constants
    MAX_CONTEXT_LINES = 3  # Maximum context lines to show before/after changes
    LARGE_DIFF_THRESHOLD = 100  # Lines count for "large diff" detection
    SEPARATOR_WIDTH = 60  # Width of visual separators
    LINE_NUM_WIDTH = 6  # Width for line number display

    # Binary file patterns
    BINARY_EXTENSIONS = {
        ".png",
        ".jpg",
        ".jpeg",
        ".gif",
        ".pdf",
        ".zip",
        ".tar",
        ".gz",
        ".so",
        ".pyc",
        ".exe",
        ".dll",
    }

    def __init__(self, console: "Console") -> None:
        """
        Initialize diff formatter.

        Args:
            console: Rich console for output
        """
        self.console = console

    def format_and_print(self, text: str) -> None:
        """
        Print diff with enhanced visualization and file path highlighting.

        Features:
        - Color-coded diff lines (additions, deletions, context)
        - File path highlighting with improved contrast
        - Diff statistics summary (+X/-Y lines) with per-file breakdown
        - Visual separation between file changes with subtle styling
        - Enhanced hunk headers with line range info and context highlighting
        - Smart context line limiting for large diffs
        - Improved spacing for better readability
        - Binary file detection and special handling
        - Empty diff detection with clear messaging

        Args:
            text: Diff text to render
        """
        if not RICH_AVAILABLE:
            print(text)
            return

        lines = text.split("\n")

        # Calculate diff statistics
        stats = self._calculate_stats(lines)

        # Handle empty diffs
        if stats.additions == 0 and stats.deletions == 0 and stats.files == 0:
            self.console.print("[dim italic]No changes detected[/dim italic]")
            return

        # Print summary if we have changes
        self._print_summary(stats)

        current_file = None
        current_file_name = None
        context_line_count = 0
        in_change_section = False

        for line in lines:
            # File headers - highlight with bold cyan
            if line.startswith("diff --git"):
                # Add visual separator between files (except first)
                if current_file is not None:
                    # Print per-file stats before separator
                    if current_file_name is not None:
                        self._print_file_stats(current_file_name, stats)
                    self.console.print()
                    self.console.print(f"[dim]{'─' * self.SEPARATOR_WIDTH}[/dim]")
                    self.console.print()

                current_file = line
                # Extract filename for stats tracking
                current_file_name = self._extract_filename(line)
                # Check for binary files
                if self._is_binary_file(line):
                    self.console.print(
                        f"[bold magenta]{line} [dim](binary)[/dim][/bold magenta]"
                    )
                else:
                    self.console.print(f"[bold cyan]{line}[/bold cyan]")
                context_line_count = 0
            elif line.startswith("Binary files"):
                # Binary file indicator
                self.console.print(f"[yellow]📦 {line}[/yellow]")
                continue
            elif line.startswith("---") or line.startswith("+++"):
                # File paths - extract and highlight
                if line.startswith("---"):
                    self.console.print(f"[bold red]{line}[/bold red]")
                else:  # +++
                    self.console.print(f"[bold green]{line}[/bold green]")
            # Hunk headers - enhanced with context
            elif line.startswith("@@"):
                # Add subtle spacing before hunk for better visual separation
                if context_line_count > 0:
                    self.console.print()

                # Extract line ranges for better readability
                hunk_info = self._format_hunk_header(line)
                self.console.print(f"[bold magenta]{hunk_info}[/bold magenta]")
                context_line_count = 0
                in_change_section = False
            # Added lines - enhanced with bold for better contrast
            elif line.startswith("+"):
                self.console.print(f"[bold green]{line}[/bold green]")
                in_change_section = True
                context_line_count = 0
            # Removed lines - enhanced with bold for better contrast
            elif line.startswith("-"):
                self.console.print(f"[bold red]{line}[/bold red]")
                in_change_section = True
                context_line_count = 0
            # Context lines
            else:
                # Only show limited context lines for large diffs
                if stats.additions + stats.deletions > self.LARGE_DIFF_THRESHOLD:
                    # Show context around changes only
                    if in_change_section:
                        if context_line_count < self.MAX_CONTEXT_LINES:
                            self.console.print(f"[dim]{line}[/dim]")
                            context_line_count += 1
                        elif context_line_count == self.MAX_CONTEXT_LINES:
                            self.console.print(
                                "[dim italic]  ⋮  (context lines omitted for readability)[/dim italic]"
                            )
                            context_line_count += 1
                    else:
                        # Leading context - always show up to limit
                        if context_line_count < self.MAX_CONTEXT_LINES:
                            self.console.print(f"[dim]{line}[/dim]")
                            context_line_count += 1
                else:
                    # Small diff - show all context
                    self.console.print(f"[dim]{line}[/dim]")

        # Print final file stats
        if current_file_name:
            self._print_file_stats(current_file_name, stats)

        # Add spacing after diff for better separation from next content
        self.console.print()

    def _calculate_stats(self, lines: list[str]) -> DiffStats:
        """
        Calculate statistics from diff lines including per-file breakdown.

        Args:
            lines: List of diff lines

        Returns:
            DiffStats with additions, deletions, files count, and per-file breakdown
        """
        stats = DiffStats()
        current_file = None

        for line in lines:
            if line.startswith("diff --git"):
                stats.files += 1
                current_file = self._extract_filename(line)
                if current_file and current_file not in stats.files_changed:
                    stats.files_changed[current_file] = (0, 0)
            elif line.startswith("+") and not line.startswith("+++"):
                stats.additions += 1
                if current_file and current_file in stats.files_changed:
                    adds, dels = stats.files_changed[current_file]
                    stats.files_changed[current_file] = (adds + 1, dels)
            elif line.startswith("-") and not line.startswith("---"):
                stats.deletions += 1
                if current_file and current_file in stats.files_changed:
                    adds, dels = stats.files_changed[current_file]
                    stats.files_changed[current_file] = (adds, dels + 1)

        return stats

    def _print_summary(self, stats: DiffStats) -> None:
        """
        Print diff statistics summary.

        Args:
            stats: Diff statistics
        """
        if stats.additions == 0 and stats.deletions == 0:
            return

        summary = "[bold cyan]📊 Changes:[/bold cyan] "
        if stats.additions > 0:
            summary += f"[green]+{stats.additions}[/green]"
        if stats.additions > 0 and stats.deletions > 0:
            summary += " "
        if stats.deletions > 0:
            summary += f"[red]-{stats.deletions}[/red]"
        if stats.files > 1:
            summary += f" [dim]({stats.files} files)[/dim]"
        self.console.print(summary)
        self.console.print()

    def _is_binary_file(self, diff_header: str) -> bool:
        """
        Check if diff is for a binary file based on extension.

        Args:
            diff_header: Diff header line (e.g., "diff --git a/file.png b/file.png")

        Returns:
            True if file appears to be binary
        """
        from pathlib import Path

        # Extract file path from diff header
        parts = diff_header.split()
        if len(parts) >= 3:
            file_path = parts[2]  # e.g., "a/file.png"
            ext = Path(file_path).suffix.lower()
            return ext in self.BINARY_EXTENSIONS
        return False

    def _extract_filename(self, diff_header: str) -> Optional[str]:
        """
        Extract filename from diff header line.

        Args:
            diff_header: Diff header line (e.g., "diff --git a/file.py b/file.py")

        Returns:
            Filename or None if not found
        """
        parts = diff_header.split()
        if len(parts) >= 3:
            # Extract from "a/file.py" or "b/file.py"
            file_path = parts[2]
            if file_path.startswith("a/") or file_path.startswith("b/"):
                return file_path[2:]
            return file_path
        return None

    def _print_file_stats(self, filename: str, stats: DiffStats) -> None:
        """
        Print per-file statistics with visual bar.

        Args:
            filename: Name of the file
            stats: DiffStats containing per-file breakdown
        """
        if filename and filename in stats.files_changed:
            adds, dels = stats.files_changed[filename]
            if adds > 0 or dels > 0:
                # Calculate visual bar proportions (max 30 chars)
                total_changes = adds + dels
                bar_width = min(30, total_changes)

                if total_changes > 0:
                    add_width = int((adds / total_changes) * bar_width)
                    del_width = bar_width - add_width

                    # Create visual bar
                    bar = ""
                    if add_width > 0:
                        bar += f"[bold green]{'▓' * add_width}[/bold green]"
                    if del_width > 0:
                        bar += f"[bold red]{'▓' * del_width}[/bold red]"

                    # Print stats with bar
                    summary = f"  {bar} "
                    if adds > 0:
                        summary += f"[bold green]+{adds}[/bold green]"
                    if adds > 0 and dels > 0:
                        summary += " "
                    if dels > 0:
                        summary += f"[bold red]-{dels}[/bold red]"
                    self.console.print(summary)

    def _format_hunk_header(self, hunk: str) -> str:
        """
        Format hunk header with enhanced readability and context highlighting.

        Transforms: @@ -140,7 +140,7 @@ class RalphConsole:
        Into: @@ Lines 140-147 → 140-147 @@ class RalphConsole:
        With context (function/class name) highlighted in cyan.

        Args:
            hunk: Original hunk header line

        Returns:
            Formatted hunk header with improved readability
        """
        # Extract line ranges using regex
        pattern = r"@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@(.*)$"
        match = re.search(pattern, hunk)

        if not match:
            return hunk

        old_start = int(match.group(1))
        old_count = int(match.group(2)) if match.group(2) else 1
        new_start = int(match.group(3))
        new_count = int(match.group(4)) if match.group(4) else 1
        context = match.group(5).strip()

        # Calculate end lines
        old_end = old_start + old_count - 1
        new_end = new_start + new_count - 1

        # Format with readable line ranges
        header = f"@@ Lines {old_start}-{old_end} → {new_start}-{new_end} @@"

        # Highlight context (function/class name) if present
        if context:
            # Highlight the context in cyan for better visibility
            header += f" [cyan]{context}[/cyan]"

        return header


class RalphConsole:
    """Rich console wrapper for Ralph output."""

    # Display constants
    CLEAR_LINE_WIDTH = 80  # Characters to clear when clearing a line
    PROGRESS_BAR_WIDTH = 30  # Width of progress bar in characters
    COUNTDOWN_COLOR_CHANGE_THRESHOLD_HIGH = 5  # Seconds remaining for yellow
    COUNTDOWN_COLOR_CHANGE_THRESHOLD_LOW = 2  # Seconds remaining for red
    MARKDOWN_INDICATOR_THRESHOLD = 2  # Minimum markdown patterns to consider as markdown
    DIFF_SCAN_LINE_LIMIT = 5  # Number of lines to scan for diff indicators
    DIFF_HUNK_SCAN_CHARS = 100  # Characters to scan for diff hunk markers

    # Regex patterns for content detection and formatting
    CODE_BLOCK_PATTERN = r"```(\w+)?\n(.*?)\n```"
    FILE_REF_PATTERN = r"(\S+\.[a-zA-Z0-9]+):(\d+)"
    INLINE_CODE_PATTERN = r"`([^`\n]+)`"
    HUNK_HEADER_PATTERN = r"@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@(.*)$"
    TABLE_SEPARATOR_PATTERN = r"^\s*\|[\s\-:|]+\|\s*$"
    MARKDOWN_HEADING_PATTERN = r"^#{1,6}\s+.+"
    MARKDOWN_UNORDERED_LIST_PATTERN = r"^[\*\-]\s+.+"
    MARKDOWN_ORDERED_LIST_PATTERN = r"^\d+\.\s+.+"
    MARKDOWN_BOLD_PATTERN = r"\*\*.+?\*\*"
    MARKDOWN_ITALIC_PATTERN = r"\*.+?\*"
    MARKDOWN_BLOCKQUOTE_PATTERN = r"^>\s+.+"
    MARKDOWN_TASK_LIST_PATTERN = r"^[\*\-]\s+\[([ xX])\]\s+.+"
    MARKDOWN_HORIZONTAL_RULE_PATTERN = r"^(\-{3,}|\*{3,}|_{3,})\s*$"

    def __init__(self) -> None:
        """Initialize Rich console."""
        if RICH_AVAILABLE:
            self.console = Console()
            self.diff_formatter = DiffFormatter(self.console)
        else:
            self.console = None
            self.diff_formatter = None

    def print_status(self, message: str, style: str = "cyan") -> None:
        """Print status message."""
        if self.console:
            # Use markup escaping to prevent Rich from parsing brackets in the icon
            self.console.print(f"[{style}][[*]] {message}[/{style}]")
        else:
            print(f"[*] {message}")

    def print_success(self, message: str) -> None:
        """Print success message."""
        if self.console:
            self.console.print(f"[green]✓[/green] {message}")
        else:
            print(f"✓ {message}")

    def print_error(self, message: str, severity: str = "error") -> None:
        """
        Print error message with severity-based formatting.

        Args:
            message: Error message to print
            severity: Error severity level ("critical", "error", "warning")
        """
        severity_styles = {
            "critical": ("[red bold]⛔[/red bold]", "red bold"),
            "error": ("[red]✗[/red]", "red"),
            "warning": ("[yellow]⚠[/yellow]", "yellow"),
        }

        icon, style = severity_styles.get(severity, severity_styles["error"])
        if self.console:
            self.console.print(f"{icon} [{style}]{message}[/{style}]")
        else:
            print(f"✗ {message}")

    def print_warning(self, message: str) -> None:
        """Print warning message."""
        if self.console:
            self.console.print(f"[yellow]⚠[/yellow] {message}")
        else:
            print(f"⚠ {message}")

    def print_info(self, message: str) -> None:
        """Print info message."""
        if self.console:
            self.console.print(f"[blue]ℹ[/blue] {message}")
        else:
            print(f"ℹ {message}")

    def print_header(self, title: str) -> None:
        """Print section header."""
        if self.console and Panel:
            self.console.print(
                Panel(title, style="green bold", border_style="green"),
                justify="left",
            )
        else:
            print(f"\n=== {title} ===\n")

    def print_iteration_header(self, iteration: int) -> None:
        """Print iteration header."""
        if self.console:
            self.console.print(
                f"\n[cyan bold]=== RALPH ITERATION {iteration} ===[/cyan bold]\n"
            )
        else:
            print(f"\n=== RALPH ITERATION {iteration} ===\n")

    def print_stats(
        self,
        iteration: int,
        success_count: int,
        error_count: int,
        start_time: str,
        prompt_file: str,
        recent_lines: list[str],
    ) -> None:
        """
        Print statistics table.

        Args:
            iteration: Current iteration number
            success_count: Number of successful iterations
            error_count: Number of failed iterations
            start_time: Start time string
            prompt_file: Prompt file name
            recent_lines: Recent log entries
        """
        if not self.console or not Table:
            # Plain text fallback
            print("\nRALPH STATISTICS")
            print(f"  Iteration: {iteration}")
            print(f"  Successful: {success_count}")
            print(f"  Failed: {error_count}")
            print(f"  Started: {start_time}")
            print(f"  Prompt: {prompt_file}")
            return

        # Create stats table with better formatting
        table = Table(
            title="🤖 RALPH STATISTICS",
            show_header=True,
            header_style="bold yellow",
            border_style="cyan",
        )
        table.add_column("Metric", style="cyan bold", no_wrap=True, width=20)
        table.add_column("Value", style="white", width=40)

        # Calculate success rate
        total = success_count + error_count
        success_rate = (success_count / total * 100) if total > 0 else 0

        table.add_row("🔄 Current Iteration", str(iteration))
        table.add_row("✅ Successful", f"[green bold]{success_count}[/green bold]")
        table.add_row("❌ Failed", f"[red bold]{error_count}[/red bold]")

        # Determine success rate color based on percentage
        if success_rate > 80:
            rate_color = "green"
        elif success_rate > 50:
            rate_color = "yellow"
        else:
            rate_color = "red"
        table.add_row("📊 Success Rate", f"[{rate_color}]{success_rate:.1f}%[/]")

        table.add_row("🕐 Started", start_time or "Unknown")
        table.add_row("📝 Prompt", prompt_file)

        self.console.print(table)

        # Show recent activity with better formatting
        if recent_lines:
            self.console.print("\n[yellow bold]📋 RECENT ACTIVITY[/yellow bold]")
            for line in recent_lines:
                # Clean up log lines for display and escape Rich markup
                clean_line = escape(line.strip())
                if "[SUCCESS]" in clean_line:
                    self.console.print(f"  [green]▸[/green] {clean_line}")
                elif "[ERROR]" in clean_line:
                    self.console.print(f"  [red]▸[/red] {clean_line}")
                elif "[WARNING]" in clean_line:
                    self.console.print(f"  [yellow]▸[/yellow] {clean_line}")
                else:
                    self.console.print(f"  [blue]▸[/blue] {clean_line}")
            self.console.print()

    def print_countdown(self, remaining: int, total: int) -> None:
        """
        Print countdown timer with progress bar.

        Args:
            remaining: Seconds remaining
            total: Total delay seconds
        """
        # Calculate progress
        progress = (total - remaining) / total
        filled = int(self.PROGRESS_BAR_WIDTH * progress)
        bar = "█" * filled + "░" * (self.PROGRESS_BAR_WIDTH - filled)

        # Color based on time remaining (using constants)
        if remaining > self.COUNTDOWN_COLOR_CHANGE_THRESHOLD_HIGH:
            color = "green"
        elif remaining > self.COUNTDOWN_COLOR_CHANGE_THRESHOLD_LOW:
            color = "yellow"
        else:
            color = "red"

        if self.console:
            self.console.print(
                f"\r[{color}]⏳ [{bar}] {remaining}s / {total}s remaining[/{color}]",
                end="",
            )
        else:
            print(f"\r⏳ [{bar}] {remaining}s / {total}s remaining", end="")

    def clear_line(self) -> None:
        """Clear current line."""
        if self.console:
            self.console.print("\r" + " " * self.CLEAR_LINE_WIDTH + "\r", end="")
        else:
            print("\r" + " " * self.CLEAR_LINE_WIDTH + "\r", end="")

    def print_separator(self) -> None:
        """Print visual separator."""
        if self.console:
            self.console.print("\n[cyan]---[/cyan]\n")
        else:
            print("\n---\n")

    def clear_screen(self) -> None:
        """Clear screen."""
        if self.console:
            self.console.clear()
        else:
            print("\033[2J\033[H", end="")

    def print_message(self, text: str) -> None:
        """
        Print message with intelligent formatting and improved visual hierarchy.

        Detects and formats:
        - Code blocks (```language) with syntax highlighting
        - Diffs (lines starting with +, -, @@) with enhanced visualization
        - Markdown tables with proper rendering
        - Markdown headings, lists, emphasis with spacing
        - Inline code (`code`) with highlighting
        - Plain text with file path detection

        Args:
            text: Message text to print
        """
        if not self.console:
            print(text)
            return

        # Check if text contains code blocks
        if "```" in text:
            # Split text by code blocks and process each part
            parts = re.split(self.CODE_BLOCK_PATTERN, text, flags=re.DOTALL)

            for i, part in enumerate(parts):
                if i % 3 == 0:  # Regular text between code blocks
                    if part.strip():
                        self._print_formatted_text(part)
                        # Add subtle spacing after text before code block
                        if i + 1 < len(parts):
                            self.console.print()
                elif i % 3 == 1:  # Language identifier
                    language = part or "text"
                    code = parts[i + 1] if i + 1 < len(parts) else ""
                    if code.strip() and Syntax:
                        # Use syntax highlighting for code blocks with enhanced features
                        syntax = Syntax(
                            code,
                            language,
                            theme="monokai",
                            line_numbers=True,
                            word_wrap=True,
                            indent_guides=True,
                            padding=(1, 2),
                        )
                        self.console.print(syntax)
                        # Add spacing after code block if more content follows
                        if i + 2 < len(parts) and parts[i + 2].strip():
                            self.console.print()
        elif self._is_diff_content(text):
            # Format as diff with enhanced visualization
            if self.diff_formatter:
                self.diff_formatter.format_and_print(text)
            else:
                print(text)
        elif self._is_markdown_table(text):
            # Render markdown tables nicely
            self._print_markdown_table(text)
            # Add spacing after table
            self.console.print()
        elif self._is_markdown_content(text):
            # Render rich markdown with headings, lists, emphasis
            self._print_markdown(text)
        else:
            # Regular text - check for inline code and format accordingly
            self._print_formatted_text(text)

    def _is_diff_content(self, text: str) -> bool:
        """
        Check if text appears to be diff content.

        Args:
            text: Text to check

        Returns:
            True if text looks like diff output
        """
        diff_indicators = [
            text.startswith("diff --git"),
            text.startswith("--- "),
            text.startswith("+++ "),
            "@@" in text[: self.DIFF_HUNK_SCAN_CHARS],  # Diff hunk markers
            any(
                line.startswith(("+", "-", "@@"))
                for line in text.split("\n")[: self.DIFF_SCAN_LINE_LIMIT]
            ),
        ]
        return any(diff_indicators)

    def _is_markdown_table(self, text: str) -> bool:
        """
        Check if text appears to be a markdown table.

        Args:
            text: Text to check

        Returns:
            True if text looks like a markdown table
        """
        lines = text.strip().split("\n")
        if len(lines) < 2:
            return False

        # Check for table separator line (e.g., |---|---|)
        for line in lines[:3]:
            if re.match(self.TABLE_SEPARATOR_PATTERN, line):
                return True
        return False

    def _print_markdown_table(self, text: str) -> None:
        """
        Print markdown table with Rich formatting.

        Args:
            text: Markdown table text
        """
        if Markdown:
            # Use Rich's Markdown renderer for tables
            md = Markdown(text)
            self.console.print(md)
        else:
            print(text)

    def _is_markdown_content(self, text: str) -> bool:
        """
        Check if text appears to contain rich markdown (headings, lists, etc.).

        Args:
            text: Text to check

        Returns:
            True if text looks like markdown with formatting
        """
        markdown_indicators = [
            re.search(self.MARKDOWN_HEADING_PATTERN, text, re.MULTILINE),  # Headings
            re.search(
                self.MARKDOWN_UNORDERED_LIST_PATTERN, text, re.MULTILINE
            ),  # Unordered lists
            re.search(
                self.MARKDOWN_ORDERED_LIST_PATTERN, text, re.MULTILINE
            ),  # Ordered lists
            re.search(self.MARKDOWN_BOLD_PATTERN, text),  # Bold
            re.search(self.MARKDOWN_ITALIC_PATTERN, text),  # Italic
            re.search(
                self.MARKDOWN_BLOCKQUOTE_PATTERN, text, re.MULTILINE
            ),  # Blockquotes
            re.search(
                self.MARKDOWN_TASK_LIST_PATTERN, text, re.MULTILINE
            ),  # Task lists
            re.search(
                self.MARKDOWN_HORIZONTAL_RULE_PATTERN, text, re.MULTILINE
            ),  # Horizontal rules
        ]
        # Return true if at least MARKDOWN_INDICATOR_THRESHOLD markdown indicators present
        threshold = self.MARKDOWN_INDICATOR_THRESHOLD
        return sum(bool(indicator) for indicator in markdown_indicators) >= threshold

    def _preprocess_markdown(self, text: str) -> str:
        """
        Preprocess markdown text for better rendering.

        Handles:
        - Task lists with checkboxes (- [ ] and - [x])
        - Horizontal rules with visual enhancement
        - Code blocks with language hints

        Args:
            text: Raw markdown text

        Returns:
            Preprocessed markdown text
        """
        lines = text.split("\n")
        processed_lines = []

        for line in lines:
            # Enhanced task lists with visual indicators
            if re.match(self.MARKDOWN_TASK_LIST_PATTERN, line):
                # Replace [ ] with ☐ and [x]/[X] with ☑
                line = re.sub(r"\[\s\]", "☐", line)
                line = re.sub(r"\[[xX]\]", "☑", line)

            # Enhanced horizontal rules - make them more visible
            if re.match(self.MARKDOWN_HORIZONTAL_RULE_PATTERN, line):
                line = f"\n{'─' * 60}\n"

            processed_lines.append(line)

        return "\n".join(processed_lines)

    def _print_markdown(self, text: str) -> None:
        """
        Print markdown content with Rich formatting and improved spacing.

        Args:
            text: Markdown text to render
        """
        if not Markdown:
            print(text)
            return

        # Add subtle spacing before markdown for visual separation
        has_heading = re.search(self.MARKDOWN_HEADING_PATTERN, text, re.MULTILINE)
        if has_heading:
            self.console.print()

        # Preprocess markdown for enhanced features
        processed_text = self._preprocess_markdown(text)

        md = Markdown(processed_text)
        self.console.print(md)

        # Add spacing after markdown blocks for better separation from next content
        self.console.print()

    def _print_formatted_text(self, text: str) -> None:
        """
        Print text with basic formatting, inline code, file path highlighting, and error detection.

        Args:
            text: Text to print
        """
        if not self.console:
            print(text)
            return

        # Check for error/exception patterns and apply special formatting
        if self._is_error_traceback(text):
            self._print_error_traceback(text)
            return

        # First, highlight file paths with line numbers (e.g., "file.py:123")
        text = re.sub(
            self.FILE_REF_PATTERN,
            lambda m: (
                f"[bold yellow]{m.group(1)}[/bold yellow]:"
                f"[bold blue]{m.group(2)}[/bold blue]"
            ),
            text,
        )

        # Check for inline code (single backticks)
        if "`" in text and "```" not in text:
            # Replace inline code with Rich markup - improved visibility
            formatted_text = re.sub(
                self.INLINE_CODE_PATTERN,
                lambda m: f"[cyan on grey23]{m.group(1)}[/cyan on grey23]",
                text,
            )
            self.console.print(formatted_text, highlight=True)
        else:
            # Enable markup for file paths and highlighting for URLs
            self.console.print(text, markup=True, highlight=True)

    def _is_error_traceback(self, text: str) -> bool:
        """
        Check if text appears to be an error traceback.

        Args:
            text: Text to check

        Returns:
            True if text looks like an error traceback
        """
        error_indicators = [
            "Traceback (most recent call last):" in text,
            re.search(r'^\s*File ".*", line \d+', text, re.MULTILINE),
            re.search(
                r"^(Error|Exception|ValueError|TypeError|RuntimeError):",
                text,
                re.MULTILINE,
            ),
        ]
        return any(error_indicators)

    def _print_error_traceback(self, text: str) -> None:
        """
        Print error traceback with enhanced formatting.

        Args:
            text: Error traceback text
        """
        if not Syntax:
            print(text)
            return

        # Use Python syntax highlighting for tracebacks
        try:
            syntax = Syntax(
                text,
                "python",
                theme="monokai",
                line_numbers=False,
                word_wrap=True,
                background_color="grey11",
            )
            self.console.print("\n[red bold]⚠ Error Traceback:[/red bold]")
            self.console.print(syntax)
            self.console.print()
        except Exception:
            # Fallback to simple red text if syntax highlighting fails
            self.console.print(f"[red]{text}[/red]")
