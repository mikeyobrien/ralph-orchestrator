# ABOUTME: Content type detection for smart output formatting
# ABOUTME: Detects diffs, code blocks, markdown, tables, and tracebacks

"""Content type detection for intelligent output formatting."""

import re
from enum import Enum
from typing import Optional


class ContentType(Enum):
    """Types of content that can be detected."""

    PLAIN_TEXT = "plain_text"
    DIFF = "diff"
    CODE_BLOCK = "code_block"
    MARKDOWN = "markdown"
    MARKDOWN_TABLE = "markdown_table"
    ERROR_TRACEBACK = "error_traceback"


class ContentDetector:
    """Detects content types for smart formatting.

    Analyzes text content to determine the most appropriate rendering method.
    Detection priority: code_block > diff > traceback > table > markdown > plain
    """

    # Detection constants
    DIFF_HUNK_SCAN_CHARS = 100
    DIFF_SCAN_LINE_LIMIT = 5
    MARKDOWN_INDICATOR_THRESHOLD = 2

    # Regex patterns
    CODE_BLOCK_PATTERN = re.compile(r"```(\w+)?\n.*?\n```", re.DOTALL)
    MARKDOWN_HEADING_PATTERN = re.compile(r"^#{1,6}\s+.+", re.MULTILINE)
    MARKDOWN_UNORDERED_LIST_PATTERN = re.compile(r"^[\*\-]\s+.+", re.MULTILINE)
    MARKDOWN_ORDERED_LIST_PATTERN = re.compile(r"^\d+\.\s+.+", re.MULTILINE)
    MARKDOWN_BOLD_PATTERN = re.compile(r"\*\*.+?\*\*")
    MARKDOWN_ITALIC_PATTERN = re.compile(r"(?<!\*)\*(?!\*)[^*\n]+\*(?!\*)")
    MARKDOWN_BLOCKQUOTE_PATTERN = re.compile(r"^>\s+.+", re.MULTILINE)
    MARKDOWN_TASK_LIST_PATTERN = re.compile(r"^[\*\-]\s+\[([ xX])\]\s+.+", re.MULTILINE)
    MARKDOWN_HORIZONTAL_RULE_PATTERN = re.compile(r"^(\-{3,}|\*{3,}|_{3,})\s*$", re.MULTILINE)
    TABLE_SEPARATOR_PATTERN = re.compile(r"^\s*\|[\s\-:|]+\|\s*$", re.MULTILINE)
    TRACEBACK_FILE_LINE_PATTERN = re.compile(r'^\s*File ".*", line \d+', re.MULTILINE)
    TRACEBACK_ERROR_PATTERN = re.compile(
        r"^(Error|Exception|ValueError|TypeError|RuntimeError|KeyError|AttributeError|"
        r"IndexError|ImportError|FileNotFoundError|NameError|ZeroDivisionError):",
        re.MULTILINE,
    )

    def detect(self, text: str) -> ContentType:
        """Detect the primary content type of text.

        Detection priority ensures more specific types are matched first:
        1. Code blocks (```...```) - highest priority
        2. Diffs (git diff format)
        3. Error tracebacks (Python exceptions)
        4. Markdown tables (|...|)
        5. Rich markdown (headings, lists, etc.)
        6. Plain text (fallback)

        Args:
            text: Text content to analyze

        Returns:
            The detected ContentType
        """
        if not text or not text.strip():
            return ContentType.PLAIN_TEXT

        # Check in priority order
        if self.is_code_block(text):
            return ContentType.CODE_BLOCK

        if self.is_diff(text):
            return ContentType.DIFF

        if self.is_error_traceback(text):
            return ContentType.ERROR_TRACEBACK

        if self.is_markdown_table(text):
            return ContentType.MARKDOWN_TABLE

        if self.is_markdown(text):
            return ContentType.MARKDOWN

        return ContentType.PLAIN_TEXT

    def is_diff(self, text: str) -> bool:
        """Check if text is diff content.

        Detects git diff format including:
        - diff --git headers
        - --- and +++ file markers (as diff markers, not markdown hr)
        - @@ hunk markers

        Note: We avoid matching lines that merely start with + or - as those
        could be markdown list items. Diff detection requires more specific
        markers like @@ hunks or diff --git headers.

        Args:
            text: Text to check

        Returns:
            True if text appears to be diff content
        """
        if not text:
            return False

        # Check for definitive diff markers
        diff_indicators = [
            text.startswith("diff --git"),
            "@@" in text[: self.DIFF_HUNK_SCAN_CHARS],
        ]

        if any(diff_indicators):
            return True

        # Check for --- a/ and +++ b/ patterns (file markers in unified diff)
        # These are more specific than just --- or +++ which could be markdown hr
        lines = text.split("\n")[: self.DIFF_SCAN_LINE_LIMIT]
        has_file_markers = (
            any(line.startswith("--- a/") or line.startswith("--- /") for line in lines)
            and any(line.startswith("+++ b/") or line.startswith("+++ /") for line in lines)
        )
        if has_file_markers:
            return True

        # Check for @@ hunk pattern specifically
        return any(line.startswith("@@") for line in lines)

    def is_code_block(self, text: str) -> bool:
        """Check if text contains fenced code blocks.

        Detects markdown-style code blocks with triple backticks.

        Args:
            text: Text to check

        Returns:
            True if text contains code blocks
        """
        if not text:
            return False
        return "```" in text and self.CODE_BLOCK_PATTERN.search(text) is not None

    def is_markdown(self, text: str) -> bool:
        """Check if text contains rich markdown formatting.

        Requires at least MARKDOWN_INDICATOR_THRESHOLD indicators to avoid
        false positives on text that happens to contain a single markdown element.

        Detected elements:
        - Headings (# Title)
        - Lists (- item, 1. item)
        - Emphasis (**bold**, *italic*)
        - Blockquotes (> quote)
        - Task lists (- [ ] task)
        - Horizontal rules (---)

        Args:
            text: Text to check

        Returns:
            True if text appears to be markdown content
        """
        if not text:
            return False

        markdown_indicators = [
            self.MARKDOWN_HEADING_PATTERN.search(text),
            self.MARKDOWN_UNORDERED_LIST_PATTERN.search(text),
            self.MARKDOWN_ORDERED_LIST_PATTERN.search(text),
            self.MARKDOWN_BOLD_PATTERN.search(text),
            self.MARKDOWN_ITALIC_PATTERN.search(text),
            self.MARKDOWN_BLOCKQUOTE_PATTERN.search(text),
            self.MARKDOWN_TASK_LIST_PATTERN.search(text),
            self.MARKDOWN_HORIZONTAL_RULE_PATTERN.search(text),
        ]

        return sum(bool(indicator) for indicator in markdown_indicators) >= self.MARKDOWN_INDICATOR_THRESHOLD

    def is_markdown_table(self, text: str) -> bool:
        """Check if text is a markdown table.

        Detects tables with pipe separators and header dividers.

        Args:
            text: Text to check

        Returns:
            True if text appears to be a markdown table
        """
        if not text:
            return False

        lines = text.strip().split("\n")
        if len(lines) < 2:
            return False

        # Check for table separator line (|---|---|) in first few lines
        for line in lines[:3]:
            if self.TABLE_SEPARATOR_PATTERN.match(line):
                return True
        return False

    def is_error_traceback(self, text: str) -> bool:
        """Check if text is an error traceback.

        Detects Python exception tracebacks.

        Args:
            text: Text to check

        Returns:
            True if text appears to be an error traceback
        """
        if not text:
            return False

        error_indicators = [
            "Traceback (most recent call last):" in text,
            self.TRACEBACK_FILE_LINE_PATTERN.search(text),
            self.TRACEBACK_ERROR_PATTERN.search(text),
        ]
        return any(error_indicators)

    def extract_code_blocks(self, text: str) -> list[tuple[Optional[str], str]]:
        """Extract code blocks from text.

        Args:
            text: Text containing code blocks

        Returns:
            List of (language, code) tuples. Language may be None.
        """
        if not text:
            return []

        blocks = []
        pattern = re.compile(r"```(\w+)?\n(.*?)\n```", re.DOTALL)
        for match in pattern.finditer(text):
            language = match.group(1)
            code = match.group(2)
            blocks.append((language, code))
        return blocks
