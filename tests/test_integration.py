# ABOUTME: Integration tests for Ralph Orchestrator with real CLI tools
# ABOUTME: Tests actual q chat and claude command execution with mocked outputs

"""Integration tests for Ralph Orchestrator with real tools."""

import unittest
import subprocess
import tempfile
import os
from pathlib import Path
from unittest.mock import patch, MagicMock

from ralph_orchestrator.adapters.claude import ClaudeAdapter
from ralph_orchestrator.adapters.qchat import QChatAdapter
from ralph_orchestrator.orchestrator import RalphOrchestrator


@unittest.skip("Q Chat integration tests require manual execution - use 'python -m pytest tests/test_integration.py::TestQChatIntegration -v' with real q CLI")
class TestQChatIntegration(unittest.TestCase):
    """Integration tests for Q Chat adapter - MANUAL ONLY.
    
    To run manually:
    1. Ensure 'q' CLI is installed and configured
    2. Run: python -m pytest tests/test_integration.py::TestQChatIntegration -v --no-skip
    
    WARNING: These tests will make real API calls to Q Chat service.
    """
    
    def setUp(self):
        """Set up test environment."""
        self.test_prompt = "Write a simple hello world function in Python"
        
        # Create isolated temp directory
        self.temp_dir = tempfile.mkdtemp(prefix="qchat_test_")
        self.prompt_file = Path(self.temp_dir).resolve() / "PROMPT.md"
        self.prompt_file.write_text(self.test_prompt)
        
        # Change to temp directory 
        self.original_dir = os.getcwd()
        os.chdir(self.temp_dir)
        
        self.adapter = QChatAdapter()
    
    def tearDown(self):
        """Clean up test environment."""
        os.chdir(self.original_dir)
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)
    
    @patch('ralph_orchestrator.adapters.qchat.subprocess.run')
    def test_qchat_basic_execution(self, mock_run):
        """Test basic q chat execution with mocked response."""
        # Mock availability check
        mock_run.side_effect = [
            MagicMock(returncode=0, stdout="", stderr=""),  # which q
            MagicMock(
                returncode=0,
                stdout='def hello_world():\n    print("Hello, World!")\n',
                stderr=""
            )  # q chat command
        ]
        
        response = self.adapter.execute(self.test_prompt)
        
        self.assertTrue(response.success)
        self.assertIn("hello_world", response.output)
        self.assertIn("Hello, World!", response.output)
        
        # Verify the command was called correctly
        actual_call = mock_run.call_args_list[1]
        self.assertEqual(actual_call[0][0][0:2], ["q", "chat"])
        self.assertEqual(actual_call[0][0][2], self.test_prompt)
    
    @patch('ralph_orchestrator.adapters.qchat.subprocess.run')
    def test_qchat_with_complex_prompt(self, mock_run):
        """Test q chat with complex multi-line prompt."""
        complex_prompt = """Please help me with the following tasks:
1. Create a function to calculate fibonacci numbers
2. Make it efficient using memoization
3. Add proper documentation"""
        
        mock_run.side_effect = [
            MagicMock(returncode=0),  # which q
            MagicMock(
                returncode=0,
                stdout="""def fibonacci(n, memo={}):
    '''Calculate fibonacci number with memoization.
    
    Args:
        n: The position in the fibonacci sequence
        memo: Dictionary for memoization
    
    Returns:
        The fibonacci number at position n
    '''
    if n in memo:
        return memo[n]
    if n <= 1:
        return n
    memo[n] = fibonacci(n-1, memo) + fibonacci(n-2, memo)
    return memo[n]""",
                stderr=""
            )
        ]
        
        response = self.adapter.execute(complex_prompt)
        
        self.assertTrue(response.success)
        self.assertIn("fibonacci", response.output)
        self.assertIn("memoization", response.output.lower())
    
    @patch('ralph_orchestrator.adapters.qchat.subprocess.run')
    def test_qchat_timeout_handling(self, mock_run):
        """Test q chat timeout handling."""
        mock_run.side_effect = [
            MagicMock(returncode=0),  # which q
            subprocess.TimeoutExpired(cmd=["q", "chat"], timeout=300)
        ]
        
        response = self.adapter.execute(self.test_prompt, timeout=1)
        
        self.assertFalse(response.success)
        self.assertIn("timed out", response.error)
    
    @patch('ralph_orchestrator.adapters.qchat.subprocess.run')
    def test_qchat_error_handling(self, mock_run):
        """Test q chat error handling."""
        mock_run.side_effect = [
            MagicMock(returncode=0),  # which q
            MagicMock(
                returncode=1,
                stdout="",
                stderr="Error: Invalid API key or configuration"
            )
        ]
        
        response = self.adapter.execute(self.test_prompt)
        
        self.assertFalse(response.success)
        self.assertIn("Invalid API key", response.error)
    
    def test_qchat_cost_is_free(self):
        """Test that q chat reports zero cost."""
        cost = self.adapter.estimate_cost("Any prompt of any length")
        self.assertEqual(cost, 0.0)


class TestClaudeIntegration(unittest.TestCase):
    """Integration tests for Claude adapter.

    NOTE: The Claude adapter now uses the Claude SDK (claude_agent_sdk) instead of
    subprocess calls. Tests that mock subprocess are skipped as they test the old CLI-based
    implementation.
    """

    def setUp(self):
        """Set up test environment."""
        self.test_prompt = "Explain recursion in one sentence"

        # Create isolated temp directory
        self.temp_dir = tempfile.mkdtemp(prefix="claude_test_")
        self.prompt_file = Path(self.temp_dir).resolve() / "PROMPT.md"
        self.prompt_file.write_text(self.test_prompt)

        # Change to temp directory
        self.original_dir = os.getcwd()
        os.chdir(self.temp_dir)

        self.adapter = ClaudeAdapter()

    def tearDown(self):
        """Clean up test environment."""
        os.chdir(self.original_dir)
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    @unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")
    def test_claude_basic_execution(self):
        """Test basic claude execution with mocked response.

        NOTE: Skipped because ClaudeAdapter now uses claude_agent_sdk, not subprocess.
        """
        pass

    @unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")
    def test_claude_with_model_selection(self):
        """Test claude with specific model selection.

        NOTE: Skipped because ClaudeAdapter now uses claude_agent_sdk, not subprocess.
        """
        pass

    @unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")
    def test_claude_json_output(self):
        """Test claude with JSON output format.

        NOTE: Skipped because ClaudeAdapter now uses claude_agent_sdk, not subprocess.
        """
        pass

    @unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")
    def test_claude_rate_limit_error(self):
        """Test claude rate limit error handling.

        NOTE: Skipped because ClaudeAdapter now uses claude_agent_sdk, not subprocess.
        """
        pass

    @unittest.skip("Claude adapter uses SDK - _extract_token_count method removed")
    def test_claude_token_extraction(self):
        """Test token extraction from various stderr formats.

        NOTE: Skipped because ClaudeAdapter now uses SDK which provides token counts
        directly in the response, so _extract_token_count is not needed.
        """
        pass

    def test_claude_cost_calculation(self):
        """Test Claude cost calculation."""
        # Test with known token counts
        cost_100_tokens = self.adapter._calculate_cost(100)
        cost_1000_tokens = self.adapter._calculate_cost(1000)
        cost_10000_tokens = self.adapter._calculate_cost(10000)

        self.assertGreater(cost_1000_tokens, cost_100_tokens)
        self.assertGreater(cost_10000_tokens, cost_1000_tokens)
        # Opus 4.5 pricing: $5/M input, $25/M output with 30/70 split
        # 1000 tokens: 300 input * $5/M + 700 output * $25/M = $0.019
        self.assertAlmostEqual(cost_1000_tokens, 0.019, places=3)


class TestOrchestratorIntegration(unittest.TestCase):
    """Integration tests for the full orchestrator.

    NOTE: Many tests in this class are outdated as they mock subprocess for the
    Claude adapter, which now uses the SDK. These tests are skipped until they
    can be properly rewritten to mock the SDK.
    """

    def setUp(self):
        """Set up test environment."""
        self.temp_dir = tempfile.mkdtemp(prefix="ralph_test_")
        # Use absolute path to ensure we never touch the root PROMPT.md
        self.prompt_file = Path(self.temp_dir).resolve() / "PROMPT.md"
        self.prompt_file.write_text("Test prompt content")

        # Change to temp directory for git operations
        self.original_dir = os.getcwd()
        os.chdir(self.temp_dir)

        # Initialize git repo
        subprocess.run(["git", "init"], capture_output=True)
        subprocess.run(["git", "config", "user.email", "test@test.com"], capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test User"], capture_output=True)

    def tearDown(self):
        """Clean up test environment."""
        os.chdir(self.original_dir)
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    @unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")
    def test_orchestrator_with_qchat_primary(self):
        """Test orchestrator with q chat as primary tool.

        NOTE: Skipped because ClaudeAdapter now uses claude_agent_sdk, not subprocess.
        """
        pass

    @unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")
    def test_orchestrator_fallback_chain(self):
        """Test orchestrator fallback from q chat to claude.

        NOTE: Skipped because ClaudeAdapter now uses claude_agent_sdk, not subprocess.
        """
        pass

    @unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")
    def test_orchestrator_with_cost_tracking(self):
        """Test orchestrator with cost tracking enabled.

        NOTE: Skipped because ClaudeAdapter now uses claude_agent_sdk, not subprocess.
        """
        pass

    @unittest.skip("Claude adapter uses SDK, not subprocess - test outdated")
    def test_orchestrator_safety_limits(self):
        """Test orchestrator safety limits.

        NOTE: Skipped because ClaudeAdapter now uses claude_agent_sdk, not subprocess.
        """
        pass

    @unittest.skip("_create_checkpoint is async - test needs asyncio support")
    def test_orchestrator_checkpoint_creation(self):
        """Test orchestrator git checkpoint creation.

        NOTE: Skipped because _create_checkpoint is now async and cannot be
        called synchronously. This test needs to be rewritten with pytest-asyncio.
        """
        pass


@unittest.skip("End-to-end tests with Q Chat require manual execution")
class TestEndToEndIntegration(unittest.TestCase):
    """End-to-end integration tests with multiple tools - MANUAL ONLY."""
    
    @patch('ralph_orchestrator.adapters.qchat.subprocess.run')
    def test_complete_workflow_with_all_tools(self, mock_run):
        """Test complete workflow with all three tools."""
        temp_dir = tempfile.mkdtemp(prefix="ralph_test_")
        # Use absolute path to ensure we never touch the root PROMPT.md
        prompt_file = Path(temp_dir).resolve() / "PROMPT.md"
        prompt_file.write_text("Generate a Python function to sort a list")
        
        # Mock all tool responses in sequence
        mock_run.side_effect = [
            # Git init
            MagicMock(returncode=0),
            MagicMock(returncode=0),
            MagicMock(returncode=0),
            
            # Tool availability checks
            MagicMock(returncode=0),  # claude --version
            MagicMock(returncode=0),  # which q
            MagicMock(returncode=0),  # gemini --version
            
            # First iteration - q chat succeeds
            MagicMock(
                returncode=0,
                stdout="def sort_list(lst):\n    return sorted(lst)",
                stderr=""
            ),
            
            # Git checkpoint
            MagicMock(returncode=0),  # git add
            MagicMock(returncode=0),  # git commit
            
            # Orchestrator runs until iteration limit
        ]
        
        os.chdir(temp_dir)
        subprocess.run(["git", "init"], capture_output=True)
        subprocess.run(["git", "config", "user.email", "test@test.com"], capture_output=True)
        subprocess.run(["git", "config", "user.name", "Test"], capture_output=True)
        
        orchestrator = RalphOrchestrator(
            prompt_file_or_config=str(prompt_file.resolve()),  # Use absolute path
            primary_tool="qchat",
            max_iterations=2,
            checkpoint_interval=1
        )
        
        # Orchestrator will run until max_iterations
        
        orchestrator.run()
        
        # Verify successful execution
        self.assertGreater(orchestrator.metrics.iterations, 0)
        
        # Cleanup
        os.chdir("/tmp")
        import shutil
        shutil.rmtree(temp_dir, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()