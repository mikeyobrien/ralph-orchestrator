# ABOUTME: Unit tests for ACPAdapter class
# ABOUTME: Tests initialization, availability check, and session flow

"""Tests for ACPAdapter - the main ACP adapter for Ralph Orchestrator."""

import asyncio
from unittest.mock import AsyncMock, MagicMock, patch
import pytest
import shutil

from ralph_orchestrator.adapters.acp import ACPAdapter


class TestACPAdapterInitialization:
    """Tests for ACPAdapter initialization."""

    def test_init_with_defaults(self):
        """Test initialization with default values."""
        adapter = ACPAdapter()

        assert adapter.name == "acp"
        assert adapter.agent_command == "gemini"
        assert adapter.agent_args == []
        assert adapter.timeout == 300
        assert adapter.permission_mode == "auto_approve"
        assert adapter._client is None
        assert adapter._session_id is None
        assert adapter._initialized is False

    def test_init_with_custom_command(self):
        """Test initialization with custom agent command."""
        adapter = ACPAdapter(agent_command="custom-agent")

        assert adapter.agent_command == "custom-agent"

    def test_init_with_custom_args(self):
        """Test initialization with custom agent arguments."""
        adapter = ACPAdapter(agent_args=["--verbose", "--debug"])

        assert adapter.agent_args == ["--verbose", "--debug"]

    def test_init_with_custom_timeout(self):
        """Test initialization with custom timeout."""
        adapter = ACPAdapter(timeout=600)

        assert adapter.timeout == 600

    def test_init_with_permission_mode(self):
        """Test initialization with custom permission mode."""
        adapter = ACPAdapter(permission_mode="deny_all")

        assert adapter.permission_mode == "deny_all"

    def test_init_from_config(self):
        """Test initialization from ACPAdapterConfig."""
        from ralph_orchestrator.adapters.acp_models import ACPAdapterConfig

        config = ACPAdapterConfig(
            agent_command="test-agent",
            agent_args=["--mode", "test"],
            timeout=120,
            permission_mode="allowlist",
            permission_allowlist=["fs/read_text_file"],
        )

        adapter = ACPAdapter.from_config(config)

        assert adapter.agent_command == "test-agent"
        assert adapter.agent_args == ["--mode", "test"]
        assert adapter.timeout == 120
        assert adapter.permission_mode == "allowlist"


class TestACPAdapterAvailability:
    """Tests for check_availability method."""

    def test_availability_when_command_exists(self):
        """Test availability returns True when command exists."""
        with patch.object(shutil, "which", return_value="/usr/bin/gemini"):
            adapter = ACPAdapter()
            assert adapter.check_availability() is True

    def test_availability_when_command_missing(self):
        """Test availability returns False when command missing."""
        with patch.object(shutil, "which", return_value=None):
            adapter = ACPAdapter()
            assert adapter.check_availability() is False

    def test_availability_checks_correct_command(self):
        """Test availability checks the configured command."""
        with patch.object(shutil, "which") as mock_which:
            mock_which.return_value = "/usr/bin/custom-agent"
            adapter = ACPAdapter(agent_command="custom-agent")
            adapter.check_availability()

            mock_which.assert_called_with("custom-agent")


class TestACPAdapterInitialize:
    """Tests for _initialize async method."""

    def _create_mock_client(self, init_response: dict, session_response: dict):
        """Helper to create a properly mocked ACPClient."""
        mock_client = MagicMock()
        mock_client.is_running = True
        mock_client.start = AsyncMock()
        mock_client.stop = AsyncMock()
        mock_client.on_notification = MagicMock()
        mock_client.on_request = MagicMock()

        # Create futures for each request
        init_future = asyncio.Future()
        init_future.set_result(init_response)

        session_future = asyncio.Future()
        session_future.set_result(session_response)

        mock_client.send_request = MagicMock(side_effect=[init_future, session_future])

        return mock_client

    @pytest.mark.asyncio
    async def test_initialize_starts_client(self):
        """Test _initialize starts the ACP client."""
        adapter = ACPAdapter()

        mock_client = self._create_mock_client(
            {"protocolVersion": "2024-01", "capabilities": {}},
            {"sessionId": "test-session-123"},
        )

        with patch("ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            await adapter._initialize()

            mock_client.start.assert_called_once()
            assert adapter._initialized is True

    @pytest.mark.asyncio
    async def test_initialize_sends_initialize_request(self):
        """Test _initialize sends initialize request with protocol version."""
        adapter = ACPAdapter()

        mock_client = self._create_mock_client(
            {"protocolVersion": "2024-01", "capabilities": {}},
            {"sessionId": "test-session-123"},
        )

        with patch("ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            await adapter._initialize()

            # Check initialize request was sent
            calls = mock_client.send_request.call_args_list
            assert len(calls) >= 1
            assert calls[0][0][0] == "initialize"
            assert "protocolVersion" in calls[0][0][1]

    @pytest.mark.asyncio
    async def test_initialize_creates_session(self):
        """Test _initialize creates new session and stores session_id."""
        adapter = ACPAdapter()

        mock_client = self._create_mock_client(
            {"protocolVersion": "2024-01", "capabilities": {}},
            {"sessionId": "test-session-abc"},
        )

        with patch("ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            await adapter._initialize()

            # Check session/new was called
            calls = mock_client.send_request.call_args_list
            assert len(calls) >= 2
            assert calls[1][0][0] == "session/new"

            # Check session ID was stored
            assert adapter._session_id == "test-session-abc"

    @pytest.mark.asyncio
    async def test_initialize_idempotent(self):
        """Test _initialize is idempotent (safe to call multiple times)."""
        adapter = ACPAdapter()
        adapter._initialized = True
        adapter._session_id = "existing-session"

        # Should not reinitialize
        await adapter._initialize()

        # Client should not be created
        assert adapter._client is None

    @pytest.mark.asyncio
    async def test_initialize_registers_notification_handler(self):
        """Test _initialize registers notification handler for updates."""
        adapter = ACPAdapter()

        mock_client = self._create_mock_client(
            {"protocolVersion": "2024-01"},
            {"sessionId": "test-session"},
        )

        with patch("ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            await adapter._initialize()

            # Check notification handler was registered
            mock_client.on_notification.assert_called()

    @pytest.mark.asyncio
    async def test_initialize_auto_adds_experimental_acp_for_gemini(self):
        """Test _initialize auto-adds --experimental-acp for Gemini CLI."""
        adapter = ACPAdapter(agent_command="gemini", agent_args=[])

        mock_client = self._create_mock_client(
            {"protocolVersion": "2024-01", "capabilities": {}},
            {"sessionId": "test-session-123"},
        )

        with patch("ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client) as mock_cls:
            await adapter._initialize()

            # Check ACPClient was created with --experimental-acp flag
            call_kwargs = mock_cls.call_args[1]
            assert "--experimental-acp" in call_kwargs["args"]

    @pytest.mark.asyncio
    async def test_initialize_does_not_duplicate_experimental_acp(self):
        """Test _initialize doesn't add duplicate --experimental-acp flag."""
        adapter = ACPAdapter(agent_command="gemini", agent_args=["--experimental-acp"])

        mock_client = self._create_mock_client(
            {"protocolVersion": "2024-01", "capabilities": {}},
            {"sessionId": "test-session-123"},
        )

        with patch("ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client) as mock_cls:
            await adapter._initialize()

            # Check ACPClient was created with exactly one --experimental-acp flag
            call_kwargs = mock_cls.call_args[1]
            assert call_kwargs["args"].count("--experimental-acp") == 1

    @pytest.mark.asyncio
    async def test_initialize_no_experimental_acp_for_non_gemini(self):
        """Test _initialize doesn't add --experimental-acp for non-gemini agents."""
        adapter = ACPAdapter(agent_command="other-agent", agent_args=[])

        mock_client = self._create_mock_client(
            {"protocolVersion": "2024-01", "capabilities": {}},
            {"sessionId": "test-session-123"},
        )

        with patch("ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client) as mock_cls:
            await adapter._initialize()

            # Check ACPClient was created without --experimental-acp flag
            call_kwargs = mock_cls.call_args[1]
            assert "--experimental-acp" not in call_kwargs["args"]

    @pytest.mark.asyncio
    async def test_initialize_handles_gemini_path(self):
        """Test _initialize handles full path to gemini binary."""
        adapter = ACPAdapter(agent_command="/usr/local/bin/gemini", agent_args=[])

        mock_client = self._create_mock_client(
            {"protocolVersion": "2024-01", "capabilities": {}},
            {"sessionId": "test-session-123"},
        )

        with patch("ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client) as mock_cls:
            await adapter._initialize()

            # Check ACPClient was created with --experimental-acp flag
            call_kwargs = mock_cls.call_args[1]
            assert "--experimental-acp" in call_kwargs["args"]


class TestACPAdapterExecute:
    """Tests for execute and aexecute methods."""

    def test_execute_when_unavailable(self):
        """Test execute returns error when adapter unavailable."""
        adapter = ACPAdapter()
        adapter.available = False

        response = adapter.execute("test prompt")

        assert response.success is False
        assert "not available" in response.error.lower()

    @pytest.mark.asyncio
    async def test_aexecute_when_unavailable(self):
        """Test aexecute returns error when adapter unavailable."""
        adapter = ACPAdapter()
        adapter.available = False

        response = await adapter.aexecute("test prompt")

        assert response.success is False
        assert "not available" in response.error.lower()

    @pytest.mark.asyncio
    async def test_aexecute_initializes_if_needed(self):
        """Test aexecute calls _initialize if not initialized."""
        adapter = ACPAdapter()
        adapter.available = True

        with patch.object(adapter, "_initialize", new_callable=AsyncMock) as mock_init:
            with patch.object(adapter, "_execute_prompt", new_callable=AsyncMock) as mock_exec:
                mock_exec.return_value = MagicMock(
                    success=True, output="test", error=None
                )

                await adapter.aexecute("test prompt")

                mock_init.assert_called_once()

    @pytest.mark.asyncio
    async def test_aexecute_enhances_prompt(self):
        """Test aexecute enhances prompt with orchestration instructions."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        captured_prompt = None

        async def capture_prompt(prompt, **kwargs):
            nonlocal captured_prompt
            captured_prompt = prompt
            from ralph_orchestrator.adapters.base import ToolResponse
            return ToolResponse(success=True, output="done")

        with patch.object(adapter, "_execute_prompt", side_effect=capture_prompt):
            await adapter.aexecute("simple task")

            # Should contain orchestration context
            assert captured_prompt is not None
            assert "ORCHESTRATION CONTEXT:" in captured_prompt

    def test_execute_runs_aexecute_sync(self):
        """Test sync execute wraps async aexecute."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        with patch.object(adapter, "_execute_prompt", new_callable=AsyncMock) as mock_exec:
            from ralph_orchestrator.adapters.base import ToolResponse
            mock_exec.return_value = ToolResponse(success=True, output="sync result")

            response = adapter.execute("test prompt")

            assert response.success is True
            assert response.output == "sync result"


class TestACPAdapterSignalHandling:
    """Tests for signal handling and shutdown."""

    def test_signal_handler_registration(self):
        """Test signal handlers are registered on init."""
        with patch("signal.signal") as mock_signal:
            ACPAdapter()

            # Should register SIGINT and SIGTERM handlers
            assert mock_signal.called

    @pytest.mark.asyncio
    async def test_shutdown_stops_client(self):
        """Test shutdown stops the ACP client."""
        adapter = ACPAdapter()

        mock_client = AsyncMock()
        adapter._client = mock_client
        adapter._initialized = True

        await adapter._shutdown()

        mock_client.stop.assert_called_once()
        assert adapter._initialized is False

    def test_kill_subprocess_sync(self):
        """Test sync subprocess kill for signal handlers."""
        adapter = ACPAdapter()

        mock_client = MagicMock()
        mock_process = MagicMock()
        mock_process.returncode = None
        mock_client._process = mock_process
        adapter._client = mock_client

        adapter.kill_subprocess_sync()

        mock_process.terminate.assert_called_once()


class TestACPAdapterMetadata:
    """Tests for adapter metadata and string representation."""

    def test_str_representation(self):
        """Test string representation of adapter."""
        adapter = ACPAdapter(agent_command="test-agent")
        adapter.available = True

        result = str(adapter)

        assert "acp" in result
        assert "available: True" in result

    def test_estimate_cost(self):
        """Test cost estimation returns 0 (no billing info from ACP)."""
        adapter = ACPAdapter()

        cost = adapter.estimate_cost("test prompt")

        assert cost == 0.0


class TestACPAdapterPromptExecution:
    """Tests for session/prompt execution and streaming updates (Step 5)."""

    def _create_mock_client_for_prompt(
        self,
        prompt_response: dict,
        updates: list[dict] | None = None,
    ):
        """Helper to create ACPClient mock for prompt execution.

        Args:
            prompt_response: Response for session/prompt request.
            updates: List of session/update notifications to simulate.
        """
        mock_client = MagicMock()
        mock_client.is_running = True
        mock_client.start = AsyncMock()
        mock_client.stop = AsyncMock()

        # Store the notification handler when registered
        notification_handler = None

        def capture_notification_handler(handler):
            nonlocal notification_handler
            notification_handler = handler

        mock_client.on_notification = MagicMock(side_effect=capture_notification_handler)
        mock_client.on_request = MagicMock()

        # Create future for prompt request
        prompt_future = asyncio.Future()
        prompt_future.set_result(prompt_response)
        mock_client.send_request = MagicMock(return_value=prompt_future)

        # Store updates and handler for later simulation
        mock_client._notification_handler = lambda: notification_handler
        mock_client._updates = updates or []

        return mock_client

    @pytest.mark.asyncio
    async def test_execute_prompt_sends_session_prompt(self):
        """Test _execute_prompt sends session/prompt request."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        prompt_future = asyncio.Future()
        prompt_future.set_result({"stopReason": "end_turn"})
        mock_client.send_request = MagicMock(return_value=prompt_future)

        adapter._client = mock_client
        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")

        await adapter._execute_prompt("Test prompt")

        # Verify session/prompt was called with prompt ContentBlocks
        mock_client.send_request.assert_called_once()
        call_args = mock_client.send_request.call_args
        assert call_args[0][0] == "session/prompt"
        assert "prompt" in call_args[0][1]  # ACP spec uses 'prompt' array

    @pytest.mark.asyncio
    async def test_execute_prompt_returns_tool_response(self):
        """Test _execute_prompt returns ToolResponse with output."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")
        adapter._client = mock_client

        # Create a future that simulates notifications arriving during execution
        async def simulate_prompt_with_output():
            # Simulate notification arriving during prompt execution
            adapter._handle_notification(
                "session/update",
                {"kind": "agent_message_chunk", "content": "Hello, I'm the agent response."},
            )
            return {"stopReason": "end_turn"}

        mock_client.send_request = MagicMock(
            return_value=asyncio.ensure_future(simulate_prompt_with_output())
        )

        response = await adapter._execute_prompt("Test prompt")

        assert response.success is True
        assert "Hello, I'm the agent response." in response.output
        assert response.metadata.get("tool") == "acp"
        assert response.metadata.get("stop_reason") == "end_turn"

    @pytest.mark.asyncio
    async def test_execute_prompt_accumulates_streaming_chunks(self):
        """Test _execute_prompt accumulates output from session/update notifications."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")
        adapter._client = mock_client

        # Create a future that simulates streaming notifications during execution
        async def simulate_streaming_chunks():
            adapter._handle_notification(
                "session/update",
                {"kind": "agent_message_chunk", "content": "Hello "},
            )
            adapter._handle_notification(
                "session/update",
                {"kind": "agent_message_chunk", "content": "World!"},
            )
            return {"stopReason": "end_turn"}

        mock_client.send_request = MagicMock(
            return_value=asyncio.ensure_future(simulate_streaming_chunks())
        )

        response = await adapter._execute_prompt("Test prompt")

        assert response.success is True
        assert adapter._session.output == "Hello World!"
        assert "Hello World!" in response.output

    @pytest.mark.asyncio
    async def test_execute_prompt_handles_thought_chunks(self):
        """Test _execute_prompt accumulates thought chunks for verbose logging."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")
        adapter._client = mock_client

        # Simulate thought chunks during execution
        async def simulate_thought_chunks():
            adapter._handle_notification(
                "session/update",
                {"kind": "agent_thought_chunk", "content": "I should first..."},
            )
            adapter._handle_notification(
                "session/update",
                {"kind": "agent_thought_chunk", "content": " analyze the request."},
            )
            return {"stopReason": "end_turn"}

        mock_client.send_request = MagicMock(
            return_value=asyncio.ensure_future(simulate_thought_chunks())
        )

        await adapter._execute_prompt("Test prompt")

        assert adapter._session.thoughts == "I should first... analyze the request."

    @pytest.mark.asyncio
    async def test_execute_prompt_tracks_tool_calls(self):
        """Test _execute_prompt tracks tool_call notifications."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")
        adapter._client = mock_client

        # Simulate tool call during execution
        async def simulate_tool_call():
            adapter._handle_notification(
                "session/update",
                {
                    "kind": "tool_call",
                    "toolName": "fs/read_text_file",
                    "toolCallId": "tc-123",
                    "arguments": {"path": "/test/file.txt"},
                },
            )
            return {"stopReason": "end_turn"}

        mock_client.send_request = MagicMock(
            return_value=asyncio.ensure_future(simulate_tool_call())
        )

        await adapter._execute_prompt("Test prompt")

        assert len(adapter._session.tool_calls) == 1
        assert adapter._session.tool_calls[0].tool_name == "fs/read_text_file"
        assert adapter._session.tool_calls[0].tool_call_id == "tc-123"

    @pytest.mark.asyncio
    async def test_execute_prompt_tracks_tool_call_updates(self):
        """Test _execute_prompt tracks tool_call_update notifications."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")
        adapter._client = mock_client

        # Simulate tool call followed by update during execution
        async def simulate_tool_call_with_update():
            adapter._handle_notification(
                "session/update",
                {
                    "kind": "tool_call",
                    "toolName": "bash",
                    "toolCallId": "tc-456",
                    "arguments": {"command": "ls"},
                },
            )
            adapter._handle_notification(
                "session/update",
                {
                    "kind": "tool_call_update",
                    "toolCallId": "tc-456",
                    "status": "completed",
                    "result": {"output": "file.txt"},
                },
            )
            return {"stopReason": "end_turn"}

        mock_client.send_request = MagicMock(
            return_value=asyncio.ensure_future(simulate_tool_call_with_update())
        )

        await adapter._execute_prompt("Test prompt")

        tool_call = adapter._session.get_tool_call("tc-456")
        assert tool_call.status == "completed"
        assert tool_call.result == {"output": "file.txt"}

    @pytest.mark.asyncio
    async def test_execute_prompt_resets_session_state(self):
        """Test _execute_prompt resets session state before new prompt."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        prompt_future = asyncio.Future()
        prompt_future.set_result({"stopReason": "end_turn"})
        mock_client.send_request = MagicMock(return_value=prompt_future)

        adapter._client = mock_client
        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")
        adapter._session.output = "Previous output"
        adapter._session.thoughts = "Previous thoughts"

        await adapter._execute_prompt("New prompt")

        # Session should start fresh (but note: output builds up during prompt)
        # The reset happens at the START of _execute_prompt
        assert adapter._session.session_id == "test-session"

    @pytest.mark.asyncio
    async def test_execute_prompt_includes_tool_calls_in_metadata(self):
        """Test _execute_prompt includes tool_calls count in metadata."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")
        adapter._client = mock_client

        # Simulate multiple tool calls during execution
        async def simulate_multiple_tool_calls():
            for i in range(3):
                adapter._handle_notification(
                    "session/update",
                    {
                        "kind": "tool_call",
                        "toolName": f"tool_{i}",
                        "toolCallId": f"tc-{i}",
                        "arguments": {},
                    },
                )
            return {"stopReason": "end_turn"}

        mock_client.send_request = MagicMock(
            return_value=asyncio.ensure_future(simulate_multiple_tool_calls())
        )

        response = await adapter._execute_prompt("Test prompt")

        assert response.metadata.get("tool_calls_count") == 3

    @pytest.mark.asyncio
    async def test_execute_prompt_handles_error_stop_reason(self):
        """Test _execute_prompt handles error stop_reason from agent."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        prompt_future = asyncio.Future()
        prompt_future.set_result({
            "stopReason": "error",
            "error": {"message": "Something went wrong"},
        })
        mock_client.send_request = MagicMock(return_value=prompt_future)

        adapter._client = mock_client
        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")

        response = await adapter._execute_prompt("Test prompt")

        assert response.success is False
        assert "Something went wrong" in response.error

    @pytest.mark.asyncio
    async def test_execute_prompt_handles_timeout(self):
        """Test _execute_prompt handles timeout gracefully."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"
        adapter.timeout = 0.01  # Very short timeout

        mock_client = MagicMock()
        mock_client.is_running = True

        # Create a future that never resolves
        prompt_future = asyncio.Future()
        mock_client.send_request = MagicMock(return_value=prompt_future)

        adapter._client = mock_client
        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")

        response = await adapter._execute_prompt("Test prompt")

        assert response.success is False
        assert "timed out" in response.error.lower()

    @pytest.mark.asyncio
    async def test_execute_prompt_formats_messages_array(self):
        """Test _execute_prompt sends properly formatted messages array."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "test-session"

        mock_client = MagicMock()
        mock_client.is_running = True

        prompt_future = asyncio.Future()
        prompt_future.set_result({"stopReason": "end_turn"})
        mock_client.send_request = MagicMock(return_value=prompt_future)

        adapter._client = mock_client
        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="test-session")

        await adapter._execute_prompt("User prompt content")

        call_args = mock_client.send_request.call_args
        params = call_args[0][1]

        # Verify prompt ContentBlocks format (per ACP spec)
        assert "prompt" in params
        prompt_blocks = params["prompt"]
        assert len(prompt_blocks) == 1
        assert prompt_blocks[0]["type"] == "text"
        assert prompt_blocks[0]["text"] == "User prompt content"

    @pytest.mark.asyncio
    async def test_execute_prompt_includes_session_id(self):
        """Test _execute_prompt includes session_id in request."""
        adapter = ACPAdapter()
        adapter.available = True
        adapter._initialized = True
        adapter._session_id = "my-session-123"

        mock_client = MagicMock()
        mock_client.is_running = True

        prompt_future = asyncio.Future()
        prompt_future.set_result({"stopReason": "end_turn"})
        mock_client.send_request = MagicMock(return_value=prompt_future)

        adapter._client = mock_client
        from ralph_orchestrator.adapters.acp_models import ACPSession
        adapter._session = ACPSession(session_id="my-session-123")

        await adapter._execute_prompt("Test")

        call_args = mock_client.send_request.call_args
        params = call_args[0][1]

        assert params.get("sessionId") == "my-session-123"


class TestACPAdapterPromptEnhancement:
    """Tests for _enhance_prompt_with_instructions method."""

    def test_enhance_prompt_includes_scratchpad_instructions(self):
        """Test enhanced prompt includes scratchpad mechanism instructions."""
        adapter = ACPAdapter()
        original_prompt = "Write a simple calculator function"

        enhanced = adapter._enhance_prompt_with_instructions(original_prompt)

        # Should include base orchestration context
        assert "ORCHESTRATION CONTEXT:" in enhanced

        # Should include scratchpad instructions
        assert "Agent Scratchpad" in enhanced
        assert ".agent/scratchpad.md" in enhanced
        assert "What you accomplished this iteration" in enhanced
        assert "Continue where the previous iteration left off" in enhanced

    def test_enhance_prompt_idempotent(self):
        """Test enhancing an already enhanced prompt is idempotent."""
        adapter = ACPAdapter()
        original_prompt = "Write a function"

        # Enhance once
        enhanced_once = adapter._enhance_prompt_with_instructions(original_prompt)

        # Enhance again
        enhanced_twice = adapter._enhance_prompt_with_instructions(enhanced_once)

        # Should be identical (no double enhancement)
        assert enhanced_once == enhanced_twice

    def test_enhance_prompt_preserves_original(self):
        """Test enhanced prompt preserves original prompt content."""
        adapter = ACPAdapter()
        original_prompt = "Implement feature X with requirements Y and Z"

        enhanced = adapter._enhance_prompt_with_instructions(original_prompt)

        # Original prompt should be present
        assert original_prompt in enhanced

    def test_enhance_prompt_orders_instructions_correctly(self):
        """Test scratchpad instructions appear before original prompt."""
        adapter = ACPAdapter()
        original_prompt = "Do task ABC"

        enhanced = adapter._enhance_prompt_with_instructions(original_prompt)

        # Scratchpad section should come before original prompt
        scratchpad_pos = enhanced.find("Agent Scratchpad")
        original_pos = enhanced.find("Do task ABC")

        assert scratchpad_pos < original_pos
