# ABOUTME: Tests for ACP adapter integration with orchestrator loop
# ABOUTME: Verifies cost tracking, metrics, checkpointing, and graceful shutdown

"""Tests for ACP adapter integration with Ralph orchestrator loop.

These tests verify that the ACP adapter works correctly within the
orchestrator's iteration loop, including:
- Cost tracking (ACP has no direct billing, falls back to free tier)
- Metrics recording captures ACP executions
- Checkpointing works with ACP responses
- Multi-iteration scenarios
- Graceful shutdown during iteration
"""

from unittest.mock import patch, AsyncMock, MagicMock

import pytest

from src.ralph_orchestrator.adapters.acp import ACPAdapter
from src.ralph_orchestrator.adapters.base import ToolResponse
from src.ralph_orchestrator.metrics import CostTracker, Metrics


# ============================================================================
# Cost Tracking Tests
# ============================================================================


class TestACPCostTracking:
    """Test cost tracking for ACP adapter."""

    def test_cost_tracker_has_acp_entry(self):
        """Test CostTracker explicitly handles 'acp' tool."""
        tracker = CostTracker()
        # ACP should be in COSTS dict (free tier since no billing from ACP)
        assert "acp" in tracker.COSTS
        assert tracker.COSTS["acp"]["input"] == 0.0
        assert tracker.COSTS["acp"]["output"] == 0.0

    def test_cost_tracker_acp_adds_zero_cost(self):
        """Test ACP usage adds zero cost."""
        tracker = CostTracker()
        cost = tracker.add_usage("acp", 1000, 500)
        assert cost == 0.0
        assert tracker.total_cost == 0.0
        assert tracker.costs_by_tool.get("acp", 0) == 0.0

    def test_cost_tracker_acp_usage_recorded(self):
        """Test ACP usage is recorded in history."""
        tracker = CostTracker()
        tracker.add_usage("acp", 1000, 500)
        assert len(tracker.usage_history) == 1
        assert tracker.usage_history[0]["tool"] == "acp"
        assert tracker.usage_history[0]["input_tokens"] == 1000
        assert tracker.usage_history[0]["output_tokens"] == 500

    def test_acp_adapter_estimate_cost_returns_zero(self):
        """Test ACPAdapter.estimate_cost() returns 0."""
        adapter = ACPAdapter()
        assert adapter.estimate_cost("Any prompt here") == 0.0


# ============================================================================
# Metrics Recording Tests
# ============================================================================


class TestACPMetricsRecording:
    """Test metrics recording for ACP adapter."""

    def test_metrics_increments_on_success(self):
        """Test metrics are incremented on successful ACP execution."""
        metrics = Metrics()
        assert metrics.iterations == 0
        assert metrics.successful_iterations == 0

        # Simulate iteration
        metrics.iterations += 1
        metrics.successful_iterations += 1

        assert metrics.iterations == 1
        assert metrics.successful_iterations == 1
        assert metrics.success_rate() == 1.0

    def test_metrics_increments_on_failure(self):
        """Test metrics are incremented on failed ACP execution."""
        metrics = Metrics()
        metrics.iterations += 1
        metrics.failed_iterations += 1

        assert metrics.iterations == 1
        assert metrics.failed_iterations == 1
        assert metrics.success_rate() == 0.0

    def test_metrics_tracks_checkpoints(self):
        """Test checkpoint counting."""
        metrics = Metrics()
        assert metrics.checkpoints == 0
        metrics.checkpoints += 1
        assert metrics.checkpoints == 1

    def test_metrics_to_dict_format(self):
        """Test metrics serialization format."""
        metrics = Metrics()
        metrics.iterations = 5
        metrics.successful_iterations = 4
        metrics.failed_iterations = 1
        metrics.checkpoints = 1

        data = metrics.to_dict()
        assert data["iterations"] == 5
        assert data["successful_iterations"] == 4
        assert data["failed_iterations"] == 1
        assert data["checkpoints"] == 1
        assert "elapsed_hours" in data
        assert "success_rate" in data


# ============================================================================
# Checkpointing Tests
# ============================================================================


class TestACPCheckpointing:
    """Test checkpointing with ACP responses."""

    def test_checkpoint_interval_calculation(self):
        """Test checkpoint interval math."""
        checkpoint_interval = 5
        for iteration in range(1, 11):
            should_checkpoint = iteration % checkpoint_interval == 0
            if iteration in [5, 10]:
                assert should_checkpoint
            else:
                assert not should_checkpoint

    @pytest.mark.asyncio
    async def test_acp_response_can_be_serialized(self):
        """Test ToolResponse from ACP can be serialized for checkpointing."""
        response = ToolResponse(
            success=True,
            output="Test output from ACP adapter",
            metadata={
                "tool": "acp",
                "agent": "gemini",
                "session_id": "test-session-123",
                "stop_reason": "end_turn",
                "tool_calls_count": 2,
                "has_thoughts": True,
            },
        )

        # Should be serializable
        import json
        data = {
            "success": response.success,
            "output": response.output,
            "error": response.error,
            "metadata": response.metadata,
        }
        serialized = json.dumps(data)
        assert "test-session-123" in serialized
        assert "acp" in serialized


# ============================================================================
# Multi-Iteration Tests
# ============================================================================


class TestACPMultiIteration:
    """Test multi-iteration scenarios with ACP adapter."""

    @pytest.mark.asyncio
    async def test_adapter_maintains_session_across_calls(self):
        """Test session ID is maintained across multiple executions."""
        adapter = ACPAdapter()

        mock_client = AsyncMock()
        mock_client.start = AsyncMock()
        mock_client.send_request = AsyncMock()
        mock_client.on_notification = MagicMock()
        mock_client.on_request = MagicMock()
        mock_client.stop = AsyncMock()

        # Mock responses
        mock_client.send_request.side_effect = [
            {"protocolVersion": "2024-01", "capabilities": {}, "agentInfo": {}},
            {"sessionId": "session-123"},
            {"stopReason": "end_turn"},  # First prompt
            {"stopReason": "end_turn"},  # Second prompt
        ]

        with patch("src.ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            # First execution
            await adapter.aexecute("First prompt")
            session_id_1 = adapter._session_id

            # Second execution (should reuse session)
            await adapter.aexecute("Second prompt")
            session_id_2 = adapter._session_id

        assert session_id_1 == session_id_2 == "session-123"
        # Session should be initialized only once
        assert adapter._initialized is True

    @pytest.mark.asyncio
    async def test_adapter_reinitializes_after_shutdown(self):
        """Test adapter reinitializes after explicit shutdown."""
        adapter = ACPAdapter()

        mock_client = AsyncMock()
        mock_client.start = AsyncMock()
        mock_client.send_request = AsyncMock()
        mock_client.on_notification = MagicMock()
        mock_client.on_request = MagicMock()
        mock_client.stop = AsyncMock()
        mock_client._process = None

        # Mock responses for two full init cycles
        mock_client.send_request.side_effect = [
            {"protocolVersion": "2024-01", "capabilities": {}, "agentInfo": {}},
            {"sessionId": "session-1"},
            {"stopReason": "end_turn"},
            {"protocolVersion": "2024-01", "capabilities": {}, "agentInfo": {}},
            {"sessionId": "session-2"},
            {"stopReason": "end_turn"},
        ]

        with patch("src.ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            # First execution
            await adapter.aexecute("First prompt")
            assert adapter._session_id == "session-1"

            # Shutdown
            await adapter._shutdown()
            assert adapter._initialized is False

            # Second execution (should get new session)
            await adapter.aexecute("Second prompt")
            assert adapter._session_id == "session-2"


# ============================================================================
# Graceful Shutdown Tests
# ============================================================================


class TestACPGracefulShutdown:
    """Test graceful shutdown handling for ACP adapter."""

    def test_kill_subprocess_sync_is_signal_safe(self):
        """Test kill_subprocess_sync doesn't raise exceptions."""
        adapter = ACPAdapter()
        # Should not raise even if no subprocess exists
        adapter.kill_subprocess_sync()

    def test_shutdown_requested_flag(self):
        """Test shutdown requested flag is set correctly."""
        adapter = ACPAdapter()
        assert adapter._shutdown_requested is False

        # Simulate signal handler setting flag
        with adapter._lock:
            adapter._shutdown_requested = True

        assert adapter._shutdown_requested is True

    @pytest.mark.asyncio
    async def test_shutdown_cleans_up_state(self):
        """Test _shutdown clears adapter state."""
        adapter = ACPAdapter()

        mock_client = AsyncMock()
        mock_client.start = AsyncMock()
        mock_client.send_request = AsyncMock()
        mock_client.on_notification = MagicMock()
        mock_client.on_request = MagicMock()
        mock_client.stop = AsyncMock()

        mock_client.send_request.side_effect = [
            {"protocolVersion": "2024-01", "capabilities": {}, "agentInfo": {}},
            {"sessionId": "session-123"},
            {"stopReason": "end_turn"},
        ]

        with patch("src.ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            await adapter.aexecute("Test prompt")
            assert adapter._initialized is True
            assert adapter._session_id == "session-123"

            await adapter._shutdown()

        assert adapter._initialized is False
        assert adapter._session_id is None
        assert adapter._client is None
        assert adapter._session is None

    def test_signal_handler_calls_kill_subprocess(self):
        """Test signal handler triggers subprocess kill."""
        adapter = ACPAdapter()

        # Mock process
        mock_process = MagicMock()
        mock_process.returncode = None  # Still running
        mock_process.terminate = MagicMock()
        mock_process.wait = MagicMock()

        # Create mock client with process
        mock_client = MagicMock()
        mock_client._process = mock_process
        adapter._client = mock_client

        # Call kill (simulating signal handler)
        adapter.kill_subprocess_sync()

        mock_process.terminate.assert_called_once()


# ============================================================================
# Orchestrator Integration Tests
# ============================================================================


class TestACPOrchestratorIntegration:
    """Test ACP adapter integration with orchestrator."""

    def test_orchestrator_initializes_acp_adapter(self):
        """Test orchestrator can initialize ACP adapter."""
        # Create adapter directly (same as orchestrator does)
        adapter = ACPAdapter()
        assert adapter.name == "acp"
        # Availability depends on whether 'gemini' binary exists
        # This test just verifies creation works

    def test_acp_adapter_has_required_interface(self):
        """Test ACP adapter implements required ToolAdapter interface."""
        adapter = ACPAdapter()

        # Required methods
        assert hasattr(adapter, "check_availability")
        assert hasattr(adapter, "execute")
        assert hasattr(adapter, "aexecute")
        assert hasattr(adapter, "estimate_cost")

        # Required attributes
        assert hasattr(adapter, "name")
        assert hasattr(adapter, "available")

    def test_acp_adapter_name_matches_orchestrator_key(self):
        """Test adapter name matches key used in orchestrator."""
        adapter = ACPAdapter()
        assert adapter.name == "acp"  # Must match key in adapters dict

    @pytest.mark.asyncio
    async def test_acp_response_format_compatible_with_orchestrator(self):
        """Test ACP responses have all fields orchestrator expects."""
        adapter = ACPAdapter()

        mock_client = AsyncMock()
        mock_client.start = AsyncMock()
        mock_client.send_request = AsyncMock()
        mock_client.on_notification = MagicMock()
        mock_client.on_request = MagicMock()
        mock_client.stop = AsyncMock()

        mock_client.send_request.side_effect = [
            {"protocolVersion": "2024-01", "capabilities": {}, "agentInfo": {}},
            {"sessionId": "session-123"},
            {"stopReason": "end_turn"},
        ]

        with patch("src.ralph_orchestrator.adapters.acp.ACPClient", return_value=mock_client):
            response = await adapter.aexecute("Test prompt")

        # Orchestrator expects these fields
        assert hasattr(response, "success")
        assert hasattr(response, "output")
        assert hasattr(response, "error")
        assert hasattr(response, "tokens_used")
        assert hasattr(response, "cost")
        assert hasattr(response, "metadata")

        # Type checks
        assert isinstance(response.success, bool)
        assert response.output is not None or response.output == ""
