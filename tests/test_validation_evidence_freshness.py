# ABOUTME: Tests for validation evidence freshness checking
# ABOUTME: Ensures stale evidence from previous runs doesn't pass validation

"""Tests for Ralph Orchestrator validation evidence freshness."""

import unittest
from unittest.mock import patch, MagicMock
from pathlib import Path
import tempfile
import time
import os

from ralph_orchestrator.orchestrator import RalphOrchestrator


class TestValidationEvidenceFreshness(unittest.TestCase):
    """Test that validation evidence must be fresh (created during current run)."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.prompt_file = Path(self.temp_dir) / "test_prompt.md"
        self.prompt_file.write_text("# Test Task\n\nDo something.")

        # Create validation-evidence directory
        self.evidence_dir = Path(self.temp_dir) / "validation-evidence"
        self.evidence_dir.mkdir()
        (self.evidence_dir / "phase-01").mkdir()

    def tearDown(self):
        """Clean up test fixtures."""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_stale_evidence_fails_validation(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test that evidence older than run start time fails validation."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        # Create evidence files with OLD timestamps (1 day ago)
        old_time = time.time() - 86400  # 24 hours ago
        for i, ext in enumerate(['png', 'txt', 'json']):
            evidence_file = self.evidence_dir / "phase-01" / f"test_{i}.{ext}"
            evidence_file.write_text(f"test content {i}")
            os.utime(evidence_file, (old_time, old_time))

        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(self.prompt_file),
            primary_tool="claude",
            enable_validation=True,
        )

        # Set run_start_time to now (after evidence was created)
        orchestrator.run_start_time = time.time()

        has_evidence, message = orchestrator._check_validation_evidence()

        # Should FAIL because evidence is stale
        self.assertFalse(has_evidence, f"Stale evidence should fail validation: {message}")
        self.assertIn("stale", message.lower())

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_fresh_evidence_passes_validation(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test that evidence created after run start passes validation."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(self.prompt_file),
            primary_tool="claude",
            enable_validation=True,
        )

        # Set run_start_time to 1 minute ago
        orchestrator.run_start_time = time.time() - 60

        # Create evidence files NOW (after run_start_time)
        for i, ext in enumerate(['png', 'txt', 'json']):
            evidence_file = self.evidence_dir / "phase-01" / f"test_{i}.{ext}"
            evidence_file.write_text(f"test content {i}")

        has_evidence, message = orchestrator._check_validation_evidence()

        # Should PASS because evidence is fresh
        self.assertTrue(has_evidence, f"Fresh evidence should pass validation: {message}")

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_mixed_fresh_and_stale_evidence_fails(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test that mix of fresh and stale evidence fails (need ALL fresh)."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(self.prompt_file),
            primary_tool="claude",
            enable_validation=True,
        )

        # Set run_start_time
        orchestrator.run_start_time = time.time() - 60

        # Create ONE stale file (before run_start_time)
        old_time = time.time() - 86400
        stale_file = self.evidence_dir / "phase-01" / "stale.png"
        stale_file.write_text("old content")
        os.utime(stale_file, (old_time, old_time))

        # Create fresh files
        for i, ext in enumerate(['txt', 'json']):
            fresh_file = self.evidence_dir / "phase-01" / f"fresh_{i}.{ext}"
            fresh_file.write_text(f"fresh content {i}")

        has_evidence, message = orchestrator._check_validation_evidence()

        # Should FAIL - all evidence must be fresh
        self.assertFalse(has_evidence, f"Mixed fresh/stale evidence should fail: {message}")


class TestValidationEvidenceContent(unittest.TestCase):
    """Test that validation evidence content is checked for errors."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.prompt_file = Path(self.temp_dir) / "test_prompt.md"
        self.prompt_file.write_text("# Test Task\n\nDo something.")

        self.evidence_dir = Path(self.temp_dir) / "validation-evidence"
        self.evidence_dir.mkdir()
        (self.evidence_dir / "phase-01").mkdir()

    def tearDown(self):
        """Clean up test fixtures."""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_txt_evidence_with_network_error_fails(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test that TXT evidence containing 'Network request failed' fails."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(self.prompt_file),
            primary_tool="claude",
            enable_validation=True,
        )
        orchestrator.run_start_time = time.time() - 60

        # Create evidence with error content
        error_file = self.evidence_dir / "phase-01" / "output.txt"
        error_file.write_text("Starting test...\nNetwork request failed\nTest completed.")

        # Create other valid files
        (self.evidence_dir / "phase-01" / "test.png").write_text("fake png")
        (self.evidence_dir / "phase-01" / "test.json").write_text('{"status": "ok"}')

        has_evidence, message = orchestrator._check_validation_evidence()

        self.assertFalse(has_evidence, f"Evidence with errors should fail: {message}")
        self.assertIn("error", message.lower())

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_txt_evidence_with_connection_refused_fails(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test that TXT evidence containing 'connection refused' fails."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(self.prompt_file),
            primary_tool="claude",
            enable_validation=True,
        )
        orchestrator.run_start_time = time.time() - 60

        error_file = self.evidence_dir / "phase-01" / "output.txt"
        error_file.write_text("curl: (7) Failed to connect to localhost port 8085: Connection refused")

        (self.evidence_dir / "phase-01" / "test.png").write_text("fake png")
        (self.evidence_dir / "phase-01" / "test.json").write_text('{"status": "ok"}')

        has_evidence, message = orchestrator._check_validation_evidence()

        self.assertFalse(has_evidence, f"Evidence with connection errors should fail: {message}")

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_valid_evidence_content_passes(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test that evidence without error patterns passes."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(self.prompt_file),
            primary_tool="claude",
            enable_validation=True,
        )
        orchestrator.run_start_time = time.time() - 60

        # Create valid evidence
        valid_txt = self.evidence_dir / "phase-01" / "output.txt"
        valid_txt.write_text("Test started\nAll assertions passed\nTest completed successfully")

        (self.evidence_dir / "phase-01" / "test.png").write_text("fake png")
        (self.evidence_dir / "phase-01" / "test.json").write_text('{"status": "success", "tests_passed": 5}')

        has_evidence, message = orchestrator._check_validation_evidence()

        self.assertTrue(has_evidence, f"Valid evidence should pass: {message}")


if __name__ == '__main__':
    unittest.main()
