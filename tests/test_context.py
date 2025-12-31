"""Tests for ContextManager prompt_text functionality."""


from ralph_orchestrator.context import ContextManager


class TestContextManagerPromptText:
    """Test ContextManager prompt_text functionality."""

    def test_prompt_text_overrides_file(self, tmp_path):
        """Test that prompt_text overrides prompt_file."""
        # Create a prompt file with different content
        prompt_file = tmp_path / "PROMPT.md"
        prompt_file.write_text("File prompt content")

        # Create ContextManager with both file and text
        cm = ContextManager(
            prompt_file=prompt_file,
            prompt_text="Direct prompt text",
            cache_dir=tmp_path / "cache"
        )

        # prompt_text should take priority
        assert cm.get_prompt() == "Direct prompt text"

    def test_prompt_text_without_file(self, tmp_path):
        """Test prompt_text when file doesn't exist."""
        prompt_file = tmp_path / "nonexistent.md"

        cm = ContextManager(
            prompt_file=prompt_file,
            prompt_text="Direct prompt",
            cache_dir=tmp_path / "cache"
        )

        assert cm.get_prompt() == "Direct prompt"

    def test_falls_back_to_file_when_no_text(self, tmp_path):
        """Test fallback to prompt_file when prompt_text is None."""
        prompt_file = tmp_path / "PROMPT.md"
        prompt_file.write_text("File content here")

        cm = ContextManager(
            prompt_file=prompt_file,
            prompt_text=None,
            cache_dir=tmp_path / "cache"
        )

        assert cm.get_prompt() == "File content here"

    def test_empty_prompt_when_no_text_no_file(self, tmp_path):
        """Test empty prompt when neither text nor file exists."""
        prompt_file = tmp_path / "nonexistent.md"

        cm = ContextManager(
            prompt_file=prompt_file,
            prompt_text=None,
            cache_dir=tmp_path / "cache"
        )

        assert cm.get_prompt() == ""

    def test_prompt_text_with_headers(self, tmp_path):
        """Test prompt_text with markdown headers extracts stable prefix."""
        prompt_text = """# Task
Build a REST API

## Requirements
- Fast
- Secure
"""
        cm = ContextManager(
            prompt_file=tmp_path / "ignored.md",
            prompt_text=prompt_text,
            cache_dir=tmp_path / "cache"
        )

        assert "# Task" in cm.get_prompt()
        assert cm.stable_prefix is not None
        assert "# Task" in cm.stable_prefix

    def test_prompt_text_multiline(self, tmp_path):
        """Test multiline prompt_text is preserved."""
        prompt_text = """Line 1
Line 2
Line 3"""

        cm = ContextManager(
            prompt_file=tmp_path / "ignored.md",
            prompt_text=prompt_text,
            cache_dir=tmp_path / "cache"
        )

        result = cm.get_prompt()
        assert "Line 1" in result
        assert "Line 2" in result
        assert "Line 3" in result

    def test_prompt_text_optimization(self, tmp_path):
        """Test that large prompt_text gets optimized."""
        # Create a very large prompt
        large_prompt = "# Header\n\n" + "Content " * 2000

        cm = ContextManager(
            prompt_file=tmp_path / "ignored.md",
            prompt_text=large_prompt,
            max_context_size=1000,
            cache_dir=tmp_path / "cache"
        )

        result = cm.get_prompt()
        # Should be optimized (shorter than original)
        assert len(result) <= cm.max_context_size + 100

    def test_prompt_text_with_dynamic_context(self, tmp_path):
        """Test prompt_text with dynamic context updates."""
        cm = ContextManager(
            prompt_file=tmp_path / "ignored.md",
            prompt_text="Base prompt",
            cache_dir=tmp_path / "cache"
        )

        # Update context
        cm.update_context("Success: Task completed")

        result = cm.get_prompt()
        assert "Base prompt" in result

    def test_context_reset_with_prompt_text(self, tmp_path):
        """Test context reset preserves prompt_text."""
        cm = ContextManager(
            prompt_file=tmp_path / "ignored.md",
            prompt_text="Original prompt",
            cache_dir=tmp_path / "cache"
        )

        cm.update_context("Some context")
        cm.add_error_feedback("Some error")
        cm.reset()

        # prompt_text should still work after reset
        assert "Original prompt" in cm.get_prompt()


class TestContextManagerBackwardsCompatibility:
    """Test backwards compatibility without prompt_text."""

    def test_default_file_based_behavior(self, tmp_path):
        """Test default behavior with just prompt_file."""
        prompt_file = tmp_path / "PROMPT.md"
        prompt_file.write_text("File-based prompt")

        # Old-style initialization without prompt_text
        cm = ContextManager(
            prompt_file=prompt_file,
            cache_dir=tmp_path / "cache"
        )

        assert cm.get_prompt() == "File-based prompt"

    def test_stats_with_prompt_text(self, tmp_path):
        """Test stats work with prompt_text."""
        cm = ContextManager(
            prompt_file=tmp_path / "ignored.md",
            prompt_text="Test prompt",
            cache_dir=tmp_path / "cache"
        )

        stats = cm.get_stats()
        assert "stable_prefix_size" in stats
        assert "dynamic_context_items" in stats
