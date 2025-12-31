# ABOUTME: Test suite for Ralph Orchestrator adapters
# ABOUTME: Validates that adapters can be initialized and checked for availability

"""Tests for Ralph Orchestrator adapters."""

import unittest
from unittest.mock import patch, MagicMock

from ralph_orchestrator.adapters.base import ToolAdapter, ToolResponse
from ralph_orchestrator.adapters.claude import ClaudeAdapter
from ralph_orchestrator.adapters.qchat import QChatAdapter
from ralph_orchestrator.adapters.gemini import GeminiAdapter


class TestToolResponse(unittest.TestCase):
    """Test ToolResponse dataclass."""
    
    def test_tool_response_creation(self):
        """Test creating a tool response."""
        response = ToolResponse(
            success=True,
            output="Test output",
            tokens_used=100,
            cost=0.001
        )
        
        self.assertTrue(response.success)
        self.assertEqual(response.output, "Test output")
        self.assertEqual(response.tokens_used, 100)
        self.assertEqual(response.cost, 0.001)
    
    def test_tool_response_with_error(self):
        """Test creating an error response."""
        response = ToolResponse(
            success=False,
            output="",
            error="Command failed"
        )
        
        self.assertFalse(response.success)
        self.assertEqual(response.error, "Command failed")


class TestClaudeAdapter(unittest.TestCase):
    """Test Claude adapter."""
    
    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_check_availability_success(self):
        """Test Claude availability check when SDK is available."""
        adapter = ClaudeAdapter()
        self.assertTrue(adapter.available)
    
    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_verbose_parameter(self):
        """Test verbose parameter initialization."""
        adapter = ClaudeAdapter(verbose=True)
        self.assertTrue(adapter.verbose)
        
        adapter_quiet = ClaudeAdapter(verbose=False)
        self.assertFalse(adapter_quiet.verbose)
    
    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', False)
    def test_check_availability_no_sdk(self):
        """Test Claude availability check when SDK not available."""
        adapter = ClaudeAdapter()
        self.assertFalse(adapter.available)
    
    
    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    @patch('ralph_orchestrator.adapters.claude.query')
    def test_execute_success(self, mock_query):
        """Test successful Claude execution."""
        # Mock async iterator
        async def mock_async_gen():
            yield "Claude response"
        
        mock_query.return_value = mock_async_gen()
        
        adapter = ClaudeAdapter()
        response = adapter.execute("Test prompt")
        
        self.assertTrue(response.success)
        self.assertEqual(response.output, "Claude response")
    
    def test_estimate_cost(self):
        """Test cost estimation."""
        adapter = ClaudeAdapter()
        
        # Test with 1000 character prompt (roughly 250 tokens)
        cost = adapter.estimate_cost("x" * 1000)
        self.assertGreater(cost, 0)
    
    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    def test_configure(self):
        """Test adapter configuration."""
        adapter = ClaudeAdapter()
        adapter.configure(
            system_prompt="Test system prompt",
            allowed_tools=["Read", "Write"],
            disallowed_tools=["Bash"]
        )

        self.assertEqual(adapter._system_prompt, "Test system prompt")
        # Note: WebSearch is added by default when enable_web_search=True (default)
        self.assertEqual(adapter._allowed_tools, ["Read", "Write", "WebSearch"])
        self.assertEqual(adapter._disallowed_tools, ["Bash"])


class TestQChatAdapter(unittest.TestCase):
    """Test Q Chat adapter."""
    
    @patch('subprocess.run')
    def test_check_availability_success(self, mock_run):
        """Test Q Chat availability check when available."""
        mock_run.return_value = MagicMock(returncode=0)
        
        QChatAdapter()
        # Note: availability check uses 'which q'
        mock_run.assert_called_with(
            ["which", "q"],
            capture_output=True,
            timeout=5,
            text=True
        )
    
    @patch('subprocess.run')
    @patch('subprocess.Popen')
    def test_execute_success(self, mock_popen, mock_run):
        """Test successful Q Chat execution."""
        mock_run.return_value = MagicMock(returncode=0)  # availability check
        
        # Mock the Popen process with proper file descriptor support
        mock_process = MagicMock()
        # poll() should return None while running, then 0 when complete
        # We need enough None values for the reading phase, then 0 for completion
        # and finally 0 again for the returncode check
        poll_returns = [None, None, 0, 0]  # Extra 0 for final returncode check
        mock_process.poll.side_effect = poll_returns
        
        # Mock stdout and stderr with fileno() support
        mock_stdout = MagicMock()
        mock_stdout.fileno.return_value = 3  # Valid file descriptor
        # The _read_available method will be called multiple times
        # Return data on first read, then empty strings
        # Also need a value for the final read when process completes
        mock_stdout.read.side_effect = ["Q Chat response", "", "", "", ""]
        
        mock_stderr = MagicMock()
        mock_stderr.fileno.return_value = 4  # Valid file descriptor
        mock_stderr.read.side_effect = ["", "", "", "", ""]
        
        mock_process.stdout = mock_stdout
        mock_process.stderr = mock_stderr
        mock_popen.return_value = mock_process
        
        adapter = QChatAdapter()
        response = adapter.execute("Test prompt")
        
        # Debug output to understand the failure
        if not response.success:
            print(f"Response failed with error: {response.error}")
            print(f"Response output: {response.output}")
        
        self.assertTrue(response.success)
        self.assertEqual(response.output, "Q Chat response")
    
    def test_estimate_cost(self):
        """Test Q Chat cost estimation (should be free)."""
        adapter = QChatAdapter()
        cost = adapter.estimate_cost("Any prompt")
        self.assertEqual(cost, 0.0)


class TestGeminiAdapter(unittest.TestCase):
    """Test Gemini adapter."""
    
    @patch('subprocess.run')
    def test_check_availability_success(self, mock_run):
        """Test Gemini availability check when available."""
        mock_run.return_value = MagicMock(returncode=0)
        
        adapter = GeminiAdapter()
        self.assertTrue(adapter.available)
    
    @patch('subprocess.run')
    def test_execute_with_model(self, mock_run):
        """Test Gemini execution with custom model."""
        mock_run.side_effect = [
            MagicMock(returncode=0),  # availability check
            MagicMock(
                returncode=0,
                stdout="Gemini response",
                stderr=""
            )  # execution
        ]
        
        adapter = GeminiAdapter()
        response = adapter.execute("Test prompt", model="gemini-pro")
        
        self.assertTrue(response.success)
        self.assertEqual(response.output, "Gemini response")
        self.assertEqual(response.metadata["model"], "gemini-pro")
    
    def test_free_tier_cost(self):
        """Test Gemini free tier cost calculation."""
        adapter = GeminiAdapter()
        
        # Under 1M tokens should be free
        cost = adapter._calculate_cost(500000)
        self.assertEqual(cost, 0.0)
        
        # Over 1M tokens should have cost
        cost = adapter._calculate_cost(2000000)
        self.assertGreater(cost, 0)


class TestAsyncClaudeAdapter(unittest.IsolatedAsyncioTestCase):
    """Test async functionality of Claude adapter."""
    
    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    @patch('ralph_orchestrator.adapters.claude.query')
    async def test_aexecute_success(self, mock_query):
        """Test successful async execution."""
        # Mock async iterator
        async def mock_async_gen():
            yield "Test async response"
        
        mock_query.return_value = mock_async_gen()
        
        adapter = ClaudeAdapter()
        response = await adapter.aexecute("Test prompt")
        
        self.assertTrue(response.success)
        self.assertEqual(response.output, "Test async response")
    
    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    @patch('ralph_orchestrator.adapters.claude.query')
    async def test_aexecute_with_tokens(self, mock_query):
        """Test async execution with token counting."""
        # Mock TextBlock for content - the adapter checks hasattr(content_block, 'text')
        class TextBlock:
            def __init__(self):
                self.text = "Response with tokens"

        # Mock AssistantMessage - type().__name__ must be 'AssistantMessage'
        class AssistantMessage:
            def __init__(self):
                self.content = [TextBlock()]

        # Mock ResultMessage with usage stats - this is where tokens come from
        class ResultMessage:
            def __init__(self):
                self.result = "Response with tokens"
                self.usage = MagicMock()
                self.usage.total_tokens = 100

        async def mock_async_gen():
            # AssistantMessage first with content, then ResultMessage with usage
            yield AssistantMessage()
            yield ResultMessage()

        mock_query.return_value = mock_async_gen()

        adapter = ClaudeAdapter()
        response = await adapter.aexecute("Test prompt")

        self.assertTrue(response.success)
        self.assertEqual(response.output, "Response with tokens")
        self.assertEqual(response.tokens_used, 100)
        self.assertIsNotNone(response.cost)

    @patch('ralph_orchestrator.adapters.claude.CLAUDE_SDK_AVAILABLE', True)
    @patch('ralph_orchestrator.adapters.claude.query')
    async def test_aexecute_sigint_cancellation(self, mock_query):
        """Test that SIGINT cancellation is handled gracefully without error logging."""
        async def mock_async_gen():
            # Yield nothing then raise - simulates SIGINT during execution
            if False:
                yield  # Make this an async generator
            raise Exception("Command failed with exit code -2 (exit code: -2)")

        mock_query.return_value = mock_async_gen()

        adapter = ClaudeAdapter()
        response = await adapter.aexecute("Test prompt")

        self.assertFalse(response.success)
        self.assertEqual(response.error, "Execution cancelled by user")
        self.assertEqual(response.output, "")


class TestToolAdapterBase(unittest.IsolatedAsyncioTestCase):
    """Test base ToolAdapter class."""

    async def test_aexecute_with_file_uses_asyncio_to_thread(self):
        """Test that aexecute_with_file uses asyncio.to_thread for non-blocking I/O."""
        import tempfile
        from pathlib import Path

        # Create a concrete adapter for testing
        class ConcreteAdapter(ToolAdapter):
            def check_availability(self):
                return True

            def execute(self, prompt, **kwargs):
                return ToolResponse(success=True, output=prompt)

            async def aexecute(self, prompt, **kwargs):
                return ToolResponse(success=True, output=prompt)

        adapter = ConcreteAdapter("test")

        # Create a temp file with test content
        with tempfile.NamedTemporaryFile(mode='w', suffix='.txt', delete=False) as f:
            f.write("Test prompt content")
            temp_path = Path(f.name)

        try:
            # Verify the file can be read asynchronously
            response = await adapter.aexecute_with_file(temp_path)
            self.assertTrue(response.success)
            self.assertEqual(response.output, "Test prompt content")
        finally:
            temp_path.unlink()

    async def test_aexecute_with_file_file_not_found(self):
        """Test that aexecute_with_file handles missing files correctly."""
        from pathlib import Path

        class ConcreteAdapter(ToolAdapter):
            def check_availability(self):
                return True

            def execute(self, prompt, **kwargs):
                return ToolResponse(success=True, output=prompt)

        adapter = ConcreteAdapter("test")
        response = await adapter.aexecute_with_file(Path("/nonexistent/path.txt"))

        self.assertFalse(response.success)
        self.assertIn("not found", response.error)


if __name__ == "__main__":
    unittest.main()