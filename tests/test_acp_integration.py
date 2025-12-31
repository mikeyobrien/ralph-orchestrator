# ABOUTME: Integration tests for ACP adapter with real Gemini CLI
# ABOUTME: Requires GOOGLE_API_KEY environment variable; skipped when not available

"""Integration tests for ACP adapter with Gemini CLI.

These tests require:
1. GOOGLE_API_KEY environment variable set
2. Gemini CLI installed and accessible as 'gemini' command

Run with: pytest tests/test_acp_integration.py -v -m integration
Skip integration tests: pytest -m "not integration"
"""

import shutil
from unittest.mock import patch, AsyncMock, MagicMock

import pytest

from src.ralph_orchestrator.adapters.acp import ACPAdapter
from src.ralph_orchestrator.adapters.acp_models import ACPAdapterConfig


# ============================================================================
# Test Fixtures
# ============================================================================


@pytest.fixture
def gemini_available():
    """Check if Gemini CLI is available."""
    if not shutil.which("gemini"):
        pytest.skip("Gemini CLI not installed")
    return True


@pytest.fixture
def integration_workspace(tmp_path):
    """Create a workspace for integration tests."""
    workspace = tmp_path / "acp_integration"
    workspace.mkdir()
    return workspace


@pytest.fixture
def acp_adapter():
    """Create an ACP adapter configured for Gemini."""
    return ACPAdapter(
        agent_command="gemini",
        agent_args=[],
        timeout=60,
        permission_mode="auto_approve"
    )


@pytest.fixture
def acp_adapter_deny():
    """Create an ACP adapter with deny_all permission mode."""
    return ACPAdapter(
        agent_command="gemini",
        agent_args=[],
        timeout=60,
        permission_mode="deny_all"
    )


# ============================================================================
# Unit Tests (Always run - mock external dependencies)
# ============================================================================


class TestACPIntegrationUnit:
    """Unit tests for ACP integration (no external dependencies)."""

    def test_adapter_creation(self):
        """Test adapter can be created with default values."""
        adapter = ACPAdapter()
        assert adapter.agent_command == "gemini"
        assert adapter.permission_mode == "auto_approve"
        assert adapter.timeout == 300

    def test_adapter_with_custom_config(self):
        """Test adapter respects custom configuration."""
        adapter = ACPAdapter(
            agent_command="custom-agent",
            agent_args=["--verbose"],
            timeout=120,
            permission_mode="deny_all",
            permission_allowlist=["fs/read_text_file"]
        )
        assert adapter.agent_command == "custom-agent"
        assert adapter.agent_args == ["--verbose"]
        assert adapter.timeout == 120
        assert adapter.permission_mode == "deny_all"
        assert adapter.permission_allowlist == ["fs/read_text_file"]

    def test_adapter_from_config(self):
        """Test adapter creation from config dataclass."""
        config = ACPAdapterConfig(
            agent_command="test-agent",
            agent_args=["arg1"],
            timeout=60,
            permission_mode="allowlist",
            permission_allowlist=["fs/*"]
        )
        adapter = ACPAdapter.from_config(config)
        assert adapter.agent_command == "test-agent"
        assert adapter.agent_args == ["arg1"]
        assert adapter.timeout == 60
        assert adapter.permission_mode == "allowlist"

    def test_availability_check_with_mock(self):
        """Test availability check uses shutil.which."""
        with patch("shutil.which") as mock_which:
            mock_which.return_value = "/usr/bin/gemini"
            adapter = ACPAdapter()
            assert adapter.check_availability() is True
            # Note: which() may be called multiple times (init + explicit check)
            mock_which.assert_called_with("gemini")

    def test_availability_check_missing(self):
        """Test availability check when binary missing."""
        with patch("shutil.which") as mock_which:
            mock_which.return_value = None
            adapter = ACPAdapter(agent_command="nonexistent-agent")
            assert adapter.check_availability() is False

    def test_execute_when_unavailable(self):
        """Test execute returns error when adapter unavailable."""
        adapter = ACPAdapter(agent_command="nonexistent-agent")
        with patch.object(adapter, "check_availability", return_value=False):
            response = adapter.execute("Test prompt")
            assert response.success is False
            assert "not available" in response.error.lower()

    def test_adapter_name(self):
        """Test adapter name property."""
        adapter = ACPAdapter()
        assert adapter.name == "acp"


class TestACPMockedIntegration:
    """Integration-style tests with mocked subprocess."""

    @pytest.mark.asyncio
    async def test_initialize_flow_mocked(self):
        """Test initialization sequence with mocked client."""
        adapter = ACPAdapter()

        # Mock the ACPClient
        mock_client = AsyncMock()
        mock_client.start = AsyncMock()
        mock_client.send_request = AsyncMock()
        mock_client.on_notification = MagicMock()
        mock_client.on_request = MagicMock()
        mock_client.stop = AsyncMock()

        # Mock initialize response (using camelCase as per ACP spec)
        mock_client.send_request.side_effect = [
            # initialize response
            {
                "protocolVersion": "2024-01",
                "capabilities": {"streaming": True},
                "agentInfo": {"name": "gemini"}
            },
            # session/new response
            {"sessionId": "test-session-123"}
        ]

        with patch("src.ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            await adapter._initialize()

        assert adapter._initialized is True
        assert adapter._session_id == "test-session-123"

    @pytest.mark.asyncio
    async def test_execute_prompt_mocked(self):
        """Test prompt execution with mocked client."""
        adapter = ACPAdapter()

        mock_client = AsyncMock()
        mock_client.start = AsyncMock()
        mock_client.send_request = AsyncMock()
        mock_client.on_notification = MagicMock()
        mock_client.on_request = MagicMock()
        mock_client.stop = AsyncMock()

        # Mock responses (camelCase per ACP spec)
        mock_client.send_request.side_effect = [
            {"protocolVersion": "2024-01", "capabilities": {}, "agentInfo": {}},
            {"sessionId": "session-1"},
            {"stop_reason": "end_turn"}
        ]

        with patch("src.ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            # Simulate message chunks via notification handler
            response = await adapter.aexecute("Hello, world!")

        assert response.success is True

    @pytest.mark.asyncio
    async def test_permission_handling_auto_approve(self):
        """Test permission requests are auto-approved (no init needed)."""
        adapter = ACPAdapter(permission_mode="auto_approve")

        # Test permission handler directly - doesn't need initialization
        result = adapter._handle_permission_request({
            "operation": "fs/read_text_file",
            "params": {"path": "/tmp/test.txt"},
            "reason": "Read file for analysis",
            "options": [{"id": "allow", "type": "allow"}]
        })

        assert result["outcome"]["outcome"] == "selected"
        assert result["outcome"]["optionId"] == "allow"

    @pytest.mark.asyncio
    async def test_permission_handling_deny_all(self):
        """Test permission requests are denied in deny_all mode."""
        adapter = ACPAdapter(permission_mode="deny_all")

        # Test permission handler directly - doesn't need initialization
        result = adapter._handle_permission_request({
            "operation": "fs/write_text_file",
            "params": {"path": "/tmp/test.txt", "content": "data"},
            "reason": "Write test file",
            "options": [{"id": "deny", "type": "deny"}]
        })

        assert result["outcome"]["outcome"] == "cancelled"

    @pytest.mark.asyncio
    async def test_permission_handling_allowlist(self):
        """Test allowlist permission mode."""
        adapter = ACPAdapter(
            permission_mode="allowlist",
            permission_allowlist=["fs/read_text_file", "terminal/*"]
        )

        # Should approve read
        result = adapter._handle_permission_request({
            "operation": "fs/read_text_file",
            "params": {"path": "/tmp/test.txt"},
            "reason": "Read file",
            "options": [{"id": "allow", "type": "allow"}]
        })
        assert result["outcome"]["outcome"] == "selected"

        # Should deny write
        result = adapter._handle_permission_request({
            "operation": "fs/write_text_file",
            "params": {"path": "/tmp/test.txt", "content": "data"},
            "reason": "Write file",
            "options": [{"id": "deny", "type": "deny"}]
        })
        assert result["outcome"]["outcome"] == "cancelled"

        # Should approve terminal
        result = adapter._handle_permission_request({
            "operation": "terminal/execute",
            "params": {"command": ["ls"]},
            "reason": "List files",
            "options": [{"id": "allow", "type": "allow"}]
        })
        assert result["outcome"]["outcome"] == "selected"


class TestACPFileOperationsMocked:
    """Test file operations with mocked filesystem."""

    def test_read_file_handler(self, tmp_path):
        """Test file read handler."""
        adapter = ACPAdapter()

        # Create a test file
        test_file = tmp_path / "test.txt"
        test_file.write_text("Hello, World!")

        result = adapter._handlers.handle_read_file({"path": str(test_file)})
        assert "content" in result
        assert result["content"] == "Hello, World!"

    def test_read_file_not_found(self):
        """Test file read handler with missing file returns null content."""
        adapter = ACPAdapter()
        result = adapter._handlers.handle_read_file({"path": "/nonexistent/file.txt"})
        # Non-existent files return success with null content (allows existence checks)
        assert "error" not in result
        assert result["content"] is None
        assert result["exists"] is False

    def test_write_file_handler(self, tmp_path):
        """Test file write handler."""
        adapter = ACPAdapter()

        test_file = tmp_path / "output.txt"
        result = adapter._handlers.handle_write_file({
            "path": str(test_file),
            "content": "Test content"
        })

        assert "success" in result
        assert result["success"] is True
        assert test_file.read_text() == "Test content"

    def test_write_file_creates_dirs(self, tmp_path):
        """Test file write creates parent directories."""
        adapter = ACPAdapter()

        test_file = tmp_path / "subdir" / "deep" / "file.txt"
        result = adapter._handlers.handle_write_file({
            "path": str(test_file),
            "content": "Nested content"
        })

        assert result["success"] is True
        assert test_file.read_text() == "Nested content"


class TestACPTerminalOperationsMocked:
    """Test terminal operations."""

    def test_terminal_create(self):
        """Test terminal creation."""
        adapter = ACPAdapter()

        result = adapter._handlers.handle_terminal_create({
            "command": ["echo", "test"]
        })

        assert "terminalId" in result
        terminal_id = result["terminalId"]

        # Clean up
        adapter._handlers.handle_terminal_release({"terminalId": terminal_id})

    def test_terminal_workflow(self):
        """Test full terminal workflow: create, output, wait, release."""
        adapter = ACPAdapter()

        # Create terminal
        result = adapter._handlers.handle_terminal_create({
            "command": ["echo", "Hello from terminal"]
        })
        terminal_id = result["terminalId"]

        # Wait for exit
        result = adapter._handlers.handle_terminal_wait_for_exit({
            "terminalId": terminal_id,
            "timeout": 5
        })
        assert "exitCode" in result
        assert result["exitCode"] == 0

        # Read output
        result = adapter._handlers.handle_terminal_output({
            "terminalId": terminal_id
        })
        assert "output" in result
        assert "Hello from terminal" in result["output"]

        # Release terminal
        result = adapter._handlers.handle_terminal_release({
            "terminalId": terminal_id
        })
        assert result["success"] is True

    def test_terminal_not_found(self):
        """Test terminal operations with invalid ID."""
        adapter = ACPAdapter()

        result = adapter._handlers.handle_terminal_output({
            "terminalId": "nonexistent-id"
        })
        assert "error" in result
        assert result["error"]["code"] == -32001


# ============================================================================
# Integration Tests (Require GOOGLE_API_KEY and Gemini CLI)
# ============================================================================


@pytest.mark.integration
class TestACPGeminiIntegration:
    """Real integration tests with Gemini CLI.

    These tests require:
    - GOOGLE_API_KEY environment variable
    - gemini CLI binary installed

    Run with: pytest tests/test_acp_integration.py -v -m integration
    """

    @pytest.fixture(autouse=True)
    def check_prerequisites(self, google_api_key, gemini_available):
        """Ensure prerequisites are met before running tests."""
        pass

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_basic_prompt_response(self, acp_adapter):
        """Test basic prompt-response cycle with Gemini."""
        response = await acp_adapter.aexecute(
            "What is 2 + 2? Reply with just the number."
        )

        assert response.success is True
        assert response.output is not None
        assert "4" in response.output
        assert response.metadata.get("tool") == "acp"

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_streaming_updates(self, acp_adapter):
        """Test that streaming updates are processed correctly."""
        response = await acp_adapter.aexecute(
            "Count from 1 to 5, saying each number on a new line."
        )

        assert response.success is True
        # Check that multiple numbers appear (indicating streaming worked)
        for num in ["1", "2", "3", "4", "5"]:
            assert num in response.output

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_permission_flow_auto_approve(self, acp_adapter, integration_workspace):
        """Test permission requests are handled with auto_approve."""
        test_file = integration_workspace / "test_input.txt"
        test_file.write_text("Test content for reading")

        response = await acp_adapter.aexecute(
            f"Read the file at {test_file} and tell me what it contains."
        )

        assert response.success is True
        # The response should mention the file content
        # Note: This depends on Gemini actually requesting to read the file

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_error_handling(self, acp_adapter):
        """Test error handling with problematic prompts."""
        # Empty prompt should still work (or return sensible error)
        response = await acp_adapter.aexecute("")

        # Should not crash - either success or handled error
        assert response is not None

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_timeout_handling(self):
        """Test timeout is respected."""
        # Very short timeout
        adapter = ACPAdapter(
            agent_command="gemini",
            timeout=1  # 1 second - likely to timeout
        )

        # This might timeout or complete quickly depending on response
        response = await adapter.aexecute("Say hello")

        # Should complete without crashing
        assert response is not None

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_shutdown_cleanup(self, acp_adapter):
        """Test that shutdown properly cleans up resources."""
        # Execute a prompt
        await acp_adapter.aexecute("Say hello")

        # Shutdown
        await acp_adapter._shutdown()

        # Verify state is reset
        assert acp_adapter._client is None
        assert acp_adapter._initialized is False

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_session_persistence(self, acp_adapter):
        """Test that session ID is maintained across prompts."""
        # First prompt
        await acp_adapter.aexecute("Remember the word: banana")
        session_id_1 = acp_adapter._session_id

        # Second prompt (should use same session)
        await acp_adapter.aexecute("What word did I tell you to remember?")
        session_id_2 = acp_adapter._session_id

        # Session ID should be the same
        assert session_id_1 == session_id_2

    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_permission_denied_flow(self, acp_adapter_deny, integration_workspace):
        """Test that deny_all mode properly denies permissions."""
        test_file = integration_workspace / "test.txt"
        test_file.write_text("Should not read this")

        response = await acp_adapter_deny.aexecute(
            f"Try to read the file at {test_file}"
        )

        # The response should complete but any file operations would be denied
        assert response.success is True
        # Check permission history
        history = acp_adapter_deny.get_permission_history()
        # Any permission requests should have been denied
        for entry in history:
            if entry.get("operation") == "fs/read_text_file":
                assert entry.get("approved") is False


# ============================================================================
# Manual Testing Documentation
# ============================================================================


class TestACPManualTestingGuide:
    """Documentation for manual integration testing.

    This class doesn't contain actual tests, but provides documentation
    for manual testing procedures.
    """

    def test_manual_testing_steps(self):
        """Document manual testing steps.

        Manual Testing Procedure:
        ========================

        1. Setup:
           - Install Gemini CLI: Follow instructions at https://gemini.google.com/cli
           - Set GOOGLE_API_KEY: export GOOGLE_API_KEY=your-api-key
           - Verify: gemini --version

        2. Basic Test:
           ```bash
           ralph run -a acp -p "What is the capital of France?"
           ```
           Expected: Response containing "Paris"

        3. File Operations Test:
           ```bash
           echo "Test content" > /tmp/test.txt
           ralph run -a acp -p "Read /tmp/test.txt and tell me what it contains"
           ```
           Expected: Response mentioning "Test content"

        4. Permission Test (deny_all):
           ```bash
           ralph run -a acp --acp-permission-mode deny_all \
               -p "Try to read /tmp/test.txt"
           ```
           Expected: Agent cannot read file, responds accordingly

        5. Multi-iteration Test:
           ```bash
           ralph run -a acp --max-iterations 3 \
               -p "Build a simple Python hello world script"
           ```
           Expected: Multiple iterations with checkpoints

        6. Verbose Mode:
           ```bash
           ralph run -a acp -v -p "Hello"
           ```
           Expected: See streaming updates and tool calls
        """
        pass  # This is documentation only
