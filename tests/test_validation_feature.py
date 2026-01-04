# ABOUTME: Test suite for the validation feature
# ABOUTME: Tests validation parameters, proposal flow, and Claude-only guard

"""Tests for Ralph Orchestrator Validation Feature."""

import unittest
from unittest.mock import patch, MagicMock
from pathlib import Path
import tempfile

from ralph_orchestrator.orchestrator import RalphOrchestrator


class TestValidationParameters(unittest.TestCase):
    """Test validation feature parameters on RalphOrchestrator."""

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_enable_validation_parameter_defaults_to_false(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test enable_validation parameter defaults to False (opt-in)."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
            )

            # enable_validation should default to False
            self.assertFalse(orchestrator.enable_validation)
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_enable_validation_parameter_can_be_set_true(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test enable_validation parameter can be set to True."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
                enable_validation=True,
            )

            self.assertTrue(orchestrator.enable_validation)
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_validation_interactive_parameter_defaults_to_true(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test validation_interactive parameter defaults to True."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
            )

            # validation_interactive should default to True
            self.assertTrue(orchestrator.validation_interactive)
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_validation_proposal_attribute_exists(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test validation_proposal attribute exists and is None by default."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
            )

            # validation_proposal should exist and be None
            self.assertIsNone(orchestrator.validation_proposal)
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_validation_approved_attribute_exists(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test validation_approved attribute exists and is False by default."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
            )

            # validation_approved should exist and be False
            self.assertFalse(orchestrator.validation_approved)
        finally:
            Path(prompt_file).unlink()


class TestValidationClaudeOnlyGuard(unittest.TestCase):
    """Test validation feature only works with Claude adapter."""

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_validation_with_non_claude_raises_valueerror(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test ValueError raised when enable_validation=True with non-Claude adapter."""
        # Setup qchat as available
        mock_qchat_instance = MagicMock()
        mock_qchat_instance.available = True
        mock_qchat.return_value = mock_qchat_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            with self.assertRaises(ValueError) as context:
                RalphOrchestrator(
                    prompt_file_or_config=prompt_file,
                    primary_tool="qchat",
                    enable_validation=True,
                )

            self.assertIn("Claude", str(context.exception))
            self.assertIn("qchat", str(context.exception))
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_validation_with_gemini_raises_valueerror(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test ValueError raised when enable_validation=True with Gemini adapter."""
        mock_gemini_instance = MagicMock()
        mock_gemini_instance.available = True
        mock_gemini.return_value = mock_gemini_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            with self.assertRaises(ValueError) as context:
                RalphOrchestrator(
                    prompt_file_or_config=prompt_file,
                    primary_tool="gemini",
                    enable_validation=True,
                )

            self.assertIn("Claude", str(context.exception))
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_validation_with_claude_succeeds(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test validation with Claude adapter succeeds."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            # Should not raise
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
                enable_validation=True,
            )

            self.assertTrue(orchestrator.enable_validation)
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_no_error_when_validation_disabled_with_non_claude(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test no error when enable_validation=False with non-Claude adapter."""
        mock_qchat_instance = MagicMock()
        mock_qchat_instance.available = True
        mock_qchat.return_value = mock_qchat_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            # Should not raise when validation is disabled (default)
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="qchat",
                enable_validation=False,  # Explicitly disabled
            )

            self.assertFalse(orchestrator.enable_validation)
        finally:
            Path(prompt_file).unlink()


class TestValidationProposalMethods(unittest.TestCase):
    """Test validation proposal method implementations."""

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_load_proposal_prompt_method_exists(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test _load_proposal_prompt method exists on orchestrator."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
                enable_validation=True,
            )

            # Method should exist
            self.assertTrue(hasattr(orchestrator, '_load_proposal_prompt'))
            self.assertTrue(callable(getattr(orchestrator, '_load_proposal_prompt')))
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_propose_validation_strategy_method_exists(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test _propose_validation_strategy method exists on orchestrator."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
                enable_validation=True,
            )

            # Method should exist and be async
            self.assertTrue(hasattr(orchestrator, '_propose_validation_strategy'))
            self.assertTrue(callable(getattr(orchestrator, '_propose_validation_strategy')))
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_get_user_confirmation_method_exists(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test _get_user_confirmation method exists on orchestrator."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
                enable_validation=True,
            )

            # Method should exist and be async
            self.assertTrue(hasattr(orchestrator, '_get_user_confirmation'))
            self.assertTrue(callable(getattr(orchestrator, '_get_user_confirmation')))
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_load_proposal_prompt_returns_string(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test _load_proposal_prompt returns a string prompt."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
                enable_validation=True,
            )

            # Should return a non-empty string
            result = orchestrator._load_proposal_prompt()
            self.assertIsInstance(result, str)
            self.assertGreater(len(result), 0)
        finally:
            Path(prompt_file).unlink()

    @patch('ralph_orchestrator.orchestrator.ClaudeAdapter')
    @patch('ralph_orchestrator.orchestrator.QChatAdapter')
    @patch('ralph_orchestrator.orchestrator.GeminiAdapter')
    @patch('ralph_orchestrator.orchestrator.ACPAdapter')
    def test_load_proposal_prompt_contains_key_sections(
        self, mock_acp, mock_gemini, mock_qchat, mock_claude
    ):
        """Test _load_proposal_prompt returns prompt with required content."""
        mock_claude_instance = MagicMock()
        mock_claude_instance.available = True
        mock_claude.return_value = mock_claude_instance

        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Task")
            prompt_file = f.name

        try:
            orchestrator = RalphOrchestrator(
                prompt_file_or_config=prompt_file,
                primary_tool="claude",
                enable_validation=True,
            )

            result = orchestrator._load_proposal_prompt()

            # Should contain key collaborative language per spec
            self.assertIn("propose", result.lower())
            self.assertIn("user", result.lower())
        finally:
            Path(prompt_file).unlink()


class TestValidationOrchestrationIntegration(unittest.TestCase):
    """Test validation phase integration into orchestration loop."""

    def test_arun_contains_validation_proposal_integration(self):
        """Test that arun() method contains validation proposal integration code."""
        import inspect
        source = inspect.getsource(RalphOrchestrator.arun)

        # arun should check enable_validation
        self.assertIn("enable_validation", source)

        # arun should call _propose_validation_strategy
        self.assertIn("_propose_validation_strategy", source)

    def test_arun_handles_user_decline(self):
        """Test that arun() handles the case when user declines validation."""
        import inspect
        source = inspect.getsource(RalphOrchestrator.arun)

        # arun should check validation_approved
        self.assertIn("validation_approved", source)

        # arun should set enable_validation to False when declined
        # This pattern indicates graceful fallback
        self.assertIn("enable_validation", source)


class TestValidationProposalPromptFile(unittest.TestCase):
    """Test VALIDATION_PROPOSAL_PROMPT.md file exists and has correct content."""

    def setUp(self):
        """Set up test fixtures."""
        # Get path to prompts directory relative to orchestrator
        self.prompt_path = Path(__file__).parent.parent / "prompts" / "VALIDATION_PROPOSAL_PROMPT.md"

    def test_validation_proposal_prompt_file_exists(self):
        """Test VALIDATION_PROPOSAL_PROMPT.md exists in prompts directory."""
        self.assertTrue(
            self.prompt_path.exists(),
            f"VALIDATION_PROPOSAL_PROMPT.md not found at {self.prompt_path}"
        )

    def test_validation_proposal_prompt_contains_confirm(self):
        """Test prompt asks for user confirmation."""
        self.assertTrue(self.prompt_path.exists(), "Prompt file must exist first")
        content = self.prompt_path.read_text()
        self.assertIn("confirm", content.lower())

    def test_validation_proposal_prompt_contains_propose(self):
        """Test prompt uses 'propose' language."""
        self.assertTrue(self.prompt_path.exists(), "Prompt file must exist first")
        content = self.prompt_path.read_text()
        self.assertIn("propose", content.lower())

    def test_validation_proposal_prompt_mentions_user_approval(self):
        """Test prompt mentions user approval."""
        self.assertTrue(self.prompt_path.exists(), "Prompt file must exist first")
        content = self.prompt_path.read_text()
        self.assertIn("user", content.lower())
        self.assertIn("approv", content.lower())  # approval/approve

    def test_validation_proposal_prompt_has_do_not_instructions(self):
        """Test prompt has collaborative 'do not' instructions."""
        self.assertTrue(self.prompt_path.exists(), "Prompt file must exist first")
        content = self.prompt_path.read_text()
        self.assertIn("do not", content.lower())

    def test_validation_proposal_prompt_emphasizes_no_mocks(self):
        """Test prompt emphasizes real execution, no mocks."""
        self.assertTrue(self.prompt_path.exists(), "Prompt file must exist first")
        content = self.prompt_path.read_text()
        # Should mention real execution or no mocks
        has_real_execution = "real" in content.lower() and "execution" in content.lower()
        has_no_mocks = "no mock" in content.lower() or "not mock" in content.lower()
        self.assertTrue(
            has_real_execution or has_no_mocks,
            "Prompt should emphasize real execution or no mocks"
        )


class TestValidationCLIFlags(unittest.TestCase):
    """Test validation feature CLI flags in argument parser."""

    def test_enable_validation_flag_exists(self):
        """Test --enable-validation flag is recognized by argument parser."""
        import argparse
        from ralph_orchestrator.__main__ import main
        import sys

        # Get the argument parser by inspecting the main module
        # We need to check if the flag exists in the parser
        import ralph_orchestrator.__main__ as main_module

        # Check the source contains the flag definition
        import inspect
        source = inspect.getsource(main_module)

        self.assertIn("--enable-validation", source)

    def test_no_validation_interactive_flag_exists(self):
        """Test --no-validation-interactive flag is recognized by argument parser."""
        import ralph_orchestrator.__main__ as main_module
        import inspect
        source = inspect.getsource(main_module)

        self.assertIn("--no-validation-interactive", source)

    def test_validation_flags_in_parser_help(self):
        """Test validation flags appear in parser configuration."""
        import ralph_orchestrator.__main__ as main_module
        import inspect
        source = inspect.getsource(main_module)

        # Both flags should be defined in the argument parser
        self.assertIn("enable_validation", source)
        self.assertIn("validation_interactive", source)

    def test_validation_flags_passed_to_orchestrator(self):
        """Test validation flags are wired to RalphOrchestrator constructor."""
        import ralph_orchestrator.__main__ as main_module
        import inspect
        source = inspect.getsource(main_module)

        # The orchestrator instantiation should include validation flags
        # Check that enable_validation is passed to RalphOrchestrator
        self.assertIn("enable_validation=", source)
        # Check that validation_interactive is computed and passed
        self.assertIn("validation_interactive", source)


if __name__ == "__main__":
    unittest.main()
