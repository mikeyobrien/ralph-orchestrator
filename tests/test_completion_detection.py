# ABOUTME: Tests for completion marker detection feature
# ABOUTME: Validates checkbox-style TASK_COMPLETE marker parsing

"""Tests for completion marker detection in Ralph Orchestrator."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

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


if __name__ == "__main__":
    unittest.main()
