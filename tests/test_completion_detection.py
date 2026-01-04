# ABOUTME: Tests for completion marker detection feature
# ABOUTME: Validates checkbox-style TASK_COMPLETE marker parsing

"""Tests for completion marker detection in Ralph Orchestrator."""

import tempfile
import unittest
from pathlib import Path

from ralph_orchestrator.orchestrator import RalphOrchestrator


class TestCompletionMarkerDetection(unittest.TestCase):
    """Test completion marker detection functionality."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()

    def test_completion_marker_checkbox_with_dash(self):
        """Test detection of checkbox-style completion marker with dash."""
        prompt_content = """# Task

## Progress
- [x] Step 1 complete
- [x] Step 2 complete
- [x] TASK_COMPLETE

Done!
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertTrue(orchestrator._check_completion_marker())

    def test_completion_marker_checkbox_without_dash(self):
        """Test detection of checkbox completion marker without leading dash."""
        prompt_content = """# Task

## Status
[x] TASK_COMPLETE
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertTrue(orchestrator._check_completion_marker())

    def test_no_completion_marker(self):
        """Test that incomplete tasks don't trigger completion."""
        prompt_content = """# Task

## Progress
- [ ] Step 1
- [ ] Step 2
- [ ] TASK_COMPLETE

Still working...
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertFalse(orchestrator._check_completion_marker())

    def test_completion_marker_case_sensitive(self):
        """Test that completion marker is case-sensitive."""
        prompt_content = """# Task

- [x] task_complete
- [x] Task_Complete
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        # Should NOT match lowercase or mixed case
        self.assertFalse(orchestrator._check_completion_marker())

    def test_completion_marker_with_whitespace(self):
        """Test completion marker detection with surrounding whitespace."""
        prompt_content = """# Task

    - [x] TASK_COMPLETE

End of file
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertTrue(orchestrator._check_completion_marker())

    def test_completion_marker_not_in_text(self):
        """Test that TASK_COMPLETE in regular text doesn't trigger."""
        prompt_content = """# Task

Remember to add TASK_COMPLETE marker when done.
The TASK_COMPLETE should be in a checkbox.
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        # Plain text mentions shouldn't trigger
        self.assertFalse(orchestrator._check_completion_marker())

    def test_completion_marker_nonexistent_file(self):
        """Test handling of nonexistent prompt file."""
        orchestrator = RalphOrchestrator("/nonexistent/path/PROMPT.md")
        # Should return False, not raise exception
        self.assertFalse(orchestrator._check_completion_marker())

    def test_completion_marker_empty_file(self):
        """Test handling of empty prompt file."""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text("")

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertFalse(orchestrator._check_completion_marker())

    def test_completion_marker_among_other_checkboxes(self):
        """Test that marker is found among other checkbox items."""
        prompt_content = """# Task: Build Feature

## Requirements
- [x] Design architecture
- [x] Implement core logic
- [x] Write tests
- [x] Update documentation
- [x] TASK_COMPLETE

## Notes
Feature is ready for review.
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertTrue(orchestrator._check_completion_marker())


    # =========================================================================
    # Flexible completion marker tests (Option C fix for root cause bug)
    # The agent was writing **TASK_COMPLETE** but orchestrator only checked
    # for checkbox format - [x] TASK_COMPLETE
    # =========================================================================

    def test_completion_marker_bold_markdown(self):
        """Test detection of bold markdown completion marker.

        This is what the agent actually wrote during the self-improvement run,
        but the orchestrator didn't detect it, causing 27 wasted iterations.
        """
        prompt_content = """# Task

## Status
All work is done.

**TASK_COMPLETE**
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertTrue(orchestrator._check_completion_marker())

    def test_completion_marker_bold_with_description(self):
        """Test bold marker with trailing description (common pattern)."""
        prompt_content = """# Task

**TASK_COMPLETE** - No outstanding work items. Feature is production-ready.
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertTrue(orchestrator._check_completion_marker())

    def test_completion_marker_standalone_on_line(self):
        """Test standalone TASK_COMPLETE on its own line."""
        prompt_content = """# Task

## Final Status

TASK_COMPLETE

End of document.
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertTrue(orchestrator._check_completion_marker())

    def test_completion_marker_colon_format(self):
        """Test 'Status: TASK_COMPLETE' format."""
        prompt_content = """# Task

**Status**: TASK_COMPLETE
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        self.assertTrue(orchestrator._check_completion_marker())

    def test_completion_marker_in_sentence_still_rejected(self):
        """Ensure TASK_COMPLETE mid-sentence is still rejected.

        We want flexible detection but not false positives.
        """
        prompt_content = """# Task

Remember to mark TASK_COMPLETE when all items are done.
Don't forget to add the TASK_COMPLETE marker at the end.
"""
        prompt_file = Path(self.temp_dir) / "PROMPT.md"
        prompt_file.write_text(prompt_content)

        orchestrator = RalphOrchestrator(str(prompt_file))
        # Should still reject - these are instructions, not completion signals
        self.assertFalse(orchestrator._check_completion_marker())


if __name__ == "__main__":
    unittest.main()
