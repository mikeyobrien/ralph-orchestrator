# ABOUTME: Tests for the validation feature opt-in and user confirmation flow
# ABOUTME: Validates that validation is opt-in, Claude-only, and user-collaborative

"""Tests for validation feature - opt-in, Claude-only, user-collaborative."""

import unittest
from unittest.mock import patch, MagicMock, AsyncMock
import tempfile
from pathlib import Path
import asyncio

from ralph_orchestrator.orchestrator import RalphOrchestrator


class TestValidationOptIn(unittest.TestCase):
    """Test that validation is opt-in (disabled by default)."""

    def test_validation_disabled_by_default(self):
        """Validation should be disabled by default."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(prompt_file_or_config=f.name)

                # enable_validation should default to False
                self.assertFalse(orchestrator.enable_validation)

    def test_validation_can_be_enabled(self):
        """Validation can be explicitly enabled."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True
                )

                self.assertTrue(orchestrator.enable_validation)

    def test_validation_disabled_does_not_affect_orchestration(self):
        """When validation disabled, orchestration works normally."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                # Default behavior - validation disabled
                orchestrator = RalphOrchestrator(prompt_file_or_config=f.name)

                # Should NOT have validation_proposal attribute populated
                self.assertIsNone(getattr(orchestrator, 'validation_proposal', None))


class TestValidationClaudeOnly(unittest.TestCase):
    """Test that validation only works with Claude adapter."""

    def test_validation_with_claude_succeeds(self):
        """Validation with Claude adapter should succeed."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                # Should not raise
                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    primary_tool="claude",
                    enable_validation=True
                )

                self.assertTrue(orchestrator.enable_validation)

    def test_validation_with_non_claude_raises_error(self):
        """Validation with non-Claude adapter should raise ValueError."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                with patch('ralph_orchestrator.orchestrator.GeminiAdapter') as mock_gemini:
                    mock_gemini_adapter = MagicMock()
                    mock_gemini_adapter.available = True
                    mock_gemini.return_value = mock_gemini_adapter

                    # Should raise ValueError when trying to enable validation with Gemini
                    with self.assertRaises(ValueError) as context:
                        RalphOrchestrator(
                            prompt_file_or_config=f.name,
                            primary_tool="gemini",
                            enable_validation=True
                        )

                    self.assertIn("Claude", str(context.exception))
                    self.assertIn("validation", str(context.exception).lower())

    def test_validation_with_qchat_raises_error(self):
        """Validation with QChat adapter should raise ValueError."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                with patch('ralph_orchestrator.orchestrator.QChatAdapter') as mock_qchat:
                    mock_qchat_adapter = MagicMock()
                    mock_qchat_adapter.available = True
                    mock_qchat.return_value = mock_qchat_adapter

                    # Should raise ValueError
                    with self.assertRaises(ValueError) as context:
                        RalphOrchestrator(
                            prompt_file_or_config=f.name,
                            primary_tool="qchat",
                            enable_validation=True
                        )

                    self.assertIn("Claude", str(context.exception))


class TestValidationInteractiveMode(unittest.TestCase):
    """Test validation interactive mode (user confirmation required)."""

    def test_validation_interactive_default_true(self):
        """Validation interactive mode should default to True."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True
                )

                # Interactive mode should default to True
                self.assertTrue(orchestrator.validation_interactive)

    def test_validation_interactive_can_be_disabled(self):
        """Validation interactive mode can be disabled for CI/CD."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True,
                    validation_interactive=False
                )

                self.assertFalse(orchestrator.validation_interactive)


class TestValidationProposal(unittest.TestCase):
    """Test validation proposal flow (not auto-generate)."""

    def test_validation_proposal_attribute_exists(self):
        """Orchestrator should have validation_proposal attribute."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True
                )

                # Should have validation_proposal (None until populated)
                self.assertTrue(hasattr(orchestrator, 'validation_proposal'))
                self.assertIsNone(orchestrator.validation_proposal)

    def test_validation_approved_attribute_exists(self):
        """Orchestrator should have validation_approved attribute."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True
                )

                # Should have validation_approved (False until user confirms)
                self.assertTrue(hasattr(orchestrator, 'validation_approved'))
                self.assertFalse(orchestrator.validation_approved)


class TestValidationProposalFlow(unittest.TestCase):
    """Test the validation proposal flow during orchestration."""

    def test_propose_validation_strategy_method_exists(self):
        """Orchestrator should have _propose_validation_strategy method."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True
                )

                # Should have the method
                self.assertTrue(hasattr(orchestrator, '_propose_validation_strategy'))
                self.assertTrue(callable(getattr(orchestrator, '_propose_validation_strategy')))

    def test_get_user_confirmation_method_exists(self):
        """Orchestrator should have _get_user_confirmation method."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True
                )

                # Should have the method
                self.assertTrue(hasattr(orchestrator, '_get_user_confirmation'))
                self.assertTrue(callable(getattr(orchestrator, '_get_user_confirmation')))

    def test_load_proposal_prompt_method_exists(self):
        """Orchestrator should have _load_proposal_prompt method."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True
                )

                # Should have the method
                self.assertTrue(hasattr(orchestrator, '_load_proposal_prompt'))
                self.assertTrue(callable(getattr(orchestrator, '_load_proposal_prompt')))

    def test_load_proposal_prompt_returns_content(self):
        """_load_proposal_prompt should return prompt content."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as f:
            f.write("# Test Prompt\nDo something")
            f.flush()

            with patch('ralph_orchestrator.orchestrator.ClaudeAdapter') as mock_claude:
                mock_adapter = MagicMock()
                mock_adapter.available = True
                mock_claude.return_value = mock_adapter

                orchestrator = RalphOrchestrator(
                    prompt_file_or_config=f.name,
                    enable_validation=True
                )

                prompt_content = orchestrator._load_proposal_prompt()

                # Should return non-empty string with key proposal elements
                self.assertIsInstance(prompt_content, str)
                self.assertGreater(len(prompt_content), 0)
                self.assertIn("propose", prompt_content.lower())


class TestValidationProposalPrompt(unittest.TestCase):
    """Test that validation proposal prompt exists and is collaborative."""

    def test_proposal_prompt_exists(self):
        """VALIDATION_PROPOSAL_PROMPT.md should exist."""
        prompt_path = Path(__file__).parent.parent / "prompts" / "VALIDATION_PROPOSAL_PROMPT.md"
        self.assertTrue(prompt_path.exists(), f"Missing {prompt_path}")

    def test_proposal_prompt_is_collaborative(self):
        """Proposal prompt should ask for user confirmation."""
        prompt_path = Path(__file__).parent.parent / "prompts" / "VALIDATION_PROPOSAL_PROMPT.md"

        if not prompt_path.exists():
            self.skipTest("Prompt file not yet created")

        content = prompt_path.read_text()

        # Should ask for user confirmation
        self.assertIn("confirm", content.lower())
        self.assertIn("propose", content.lower())
        # Should mention user approval requirement
        self.assertIn("user approval", content.lower())
        # Should have "do not" instructions (collaborative, not prescriptive)
        self.assertIn("do not", content.lower())


if __name__ == "__main__":
    unittest.main()
