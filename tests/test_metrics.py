# ABOUTME: Tests for metrics tracking including memory-efficient iteration stats
# ABOUTME: Validates iteration tracking, memory limits, success rate, and duration tracking

"""Tests for metrics module."""

import pytest
import time
from datetime import datetime
from ralph_orchestrator.metrics import (
    Metrics,
    CostTracker,
    IterationStats,
    TriggerReason,
)


class TestMetrics:
    """Test basic Metrics class."""

    def test_initial_values(self):
        """Test default metric values."""
        m = Metrics()
        assert m.iterations == 0
        assert m.successful_iterations == 0
        assert m.failed_iterations == 0
        assert m.errors == 0

    def test_success_rate_zero_iterations(self):
        """Test success rate with no iterations."""
        m = Metrics()
        assert m.success_rate() == 0.0

    def test_success_rate_calculation(self):
        """Test success rate calculation."""
        m = Metrics()
        m.successful_iterations = 8
        m.failed_iterations = 2
        assert m.success_rate() == 0.8

    def test_elapsed_hours(self):
        """Test elapsed time calculation."""
        m = Metrics()
        m.start_time = time.time() - 3600  # 1 hour ago
        assert 0.99 < m.elapsed_hours() < 1.01

    def test_to_dict(self):
        """Test dict conversion includes all fields."""
        m = Metrics()
        m.iterations = 5
        m.successful_iterations = 4
        d = m.to_dict()
        assert "iterations" in d
        assert "successful_iterations" in d
        assert "success_rate" in d
        assert "elapsed_hours" in d


class TestCostTracker:
    """Test CostTracker class."""

    def test_initial_state(self):
        """Test initial tracker state."""
        tracker = CostTracker()
        assert tracker.total_cost == 0.0
        assert tracker.costs_by_tool == {}
        assert tracker.usage_history == []

    def test_add_usage(self):
        """Test adding usage."""
        tracker = CostTracker()
        cost = tracker.add_usage("claude", 1000, 1000)
        assert cost > 0
        assert tracker.total_cost == cost

    def test_unknown_tool_defaults_to_free(self):
        """Test unknown tool defaults to qchat (free)."""
        tracker = CostTracker()
        cost = tracker.add_usage("unknown_tool", 1000, 1000)
        assert cost == 0.0


class TestIterationStats:
    """Test memory-efficient IterationStats class."""

    def test_initial_values(self):
        """Test default iteration stats."""
        stats = IterationStats()
        assert stats.total == 0
        assert stats.successes == 0
        assert stats.failures == 0
        assert stats.current_iteration == 0
        assert len(stats.iterations) == 0
        assert stats.max_iterations_stored == 1000

    def test_start_time_auto_set(self):
        """Test start time is automatically set on creation."""
        stats = IterationStats()
        assert stats.start_time is not None
        assert isinstance(stats.start_time, datetime)

    def test_record_start(self):
        """Test recording iteration start."""
        stats = IterationStats()
        stats.record_start(5)
        assert stats.current_iteration == 5
        assert stats.total == 5

    def test_record_success(self):
        """Test recording successful iteration."""
        stats = IterationStats()
        stats.record_success(1)
        assert stats.total == 1
        assert stats.successes == 1
        assert stats.failures == 0

    def test_record_failure(self):
        """Test recording failed iteration."""
        stats = IterationStats()
        stats.record_failure(1)
        assert stats.total == 1
        assert stats.failures == 1
        assert stats.successes == 0

    def test_record_iteration_with_details(self):
        """Test recording iteration with full details."""
        stats = IterationStats()
        stats.record_iteration(
            iteration=1,
            duration=5.5,
            success=True,
            error=""
        )
        assert stats.total == 1
        assert stats.successes == 1
        assert len(stats.iterations) == 1

        # Check iteration data structure
        iter_data = stats.iterations[0]
        assert iter_data["iteration"] == 1
        assert iter_data["duration"] == 5.5
        assert iter_data["success"] is True
        assert iter_data["error"] == ""
        assert "timestamp" in iter_data

    def test_record_iteration_failure_with_error(self):
        """Test recording failed iteration with error message."""
        stats = IterationStats()
        stats.record_iteration(
            iteration=1,
            duration=2.0,
            success=False,
            error="Connection timeout"
        )
        assert stats.failures == 1
        assert stats.iterations[0]["error"] == "Connection timeout"

    def test_memory_limit_enforcement(self):
        """Test that iteration storage is limited to max_iterations_stored."""
        stats = IterationStats()
        stats.max_iterations_stored = 10  # Set low limit for testing

        # Add 15 iterations
        for i in range(15):
            stats.record_iteration(i, 1.0, True, "")

        # Should only keep the last 10
        assert len(stats.iterations) == 10
        # First stored iteration should be #5 (0-4 were evicted)
        assert stats.iterations[0]["iteration"] == 5
        # Last should be #14
        assert stats.iterations[-1]["iteration"] == 14

    def test_memory_limit_default_1000(self):
        """Test default memory limit is 1000."""
        stats = IterationStats()
        assert stats.max_iterations_stored == 1000

    def test_success_rate_zero_attempts(self):
        """Test success rate with no attempts."""
        stats = IterationStats()
        assert stats.get_success_rate() == 0.0

    def test_success_rate_calculation(self):
        """Test success rate calculation returns percentage."""
        stats = IterationStats()
        stats.successes = 8
        stats.failures = 2
        # Should be 80.0 (percent, not decimal)
        assert stats.get_success_rate() == 80.0

    def test_success_rate_all_success(self):
        """Test 100% success rate."""
        stats = IterationStats()
        for i in range(5):
            stats.record_success(i)
        assert stats.get_success_rate() == 100.0

    def test_success_rate_all_failures(self):
        """Test 0% success rate."""
        stats = IterationStats()
        for i in range(5):
            stats.record_failure(i)
        assert stats.get_success_rate() == 0.0

    def test_get_runtime_seconds(self):
        """Test runtime formatting in seconds."""
        stats = IterationStats()
        # Set start_time to 30 seconds ago
        stats.start_time = datetime.now()
        time.sleep(0.01)  # Minimal delay to ensure time passes
        runtime = stats.get_runtime()
        assert runtime.endswith("s")

    def test_get_runtime_minutes(self):
        """Test runtime formatting in minutes."""
        stats = IterationStats()
        # Set start_time to 65 seconds ago
        from datetime import timedelta
        stats.start_time = datetime.now() - timedelta(seconds=65)
        runtime = stats.get_runtime()
        assert "m" in runtime
        assert "s" in runtime

    def test_get_runtime_hours(self):
        """Test runtime formatting in hours."""
        stats = IterationStats()
        from datetime import timedelta
        stats.start_time = datetime.now() - timedelta(hours=2, minutes=30, seconds=15)
        runtime = stats.get_runtime()
        assert "h" in runtime
        assert "m" in runtime

    def test_to_dict(self):
        """Test dictionary conversion."""
        stats = IterationStats()
        stats.record_success(1)
        stats.record_failure(2)

        d = stats.to_dict()
        assert d["total"] == 2
        assert d["current"] == 0  # Not updated by record_success/failure
        assert d["successes"] == 1
        assert d["failures"] == 1
        assert d["success_rate"] == 50.0
        assert "runtime" in d
        assert "start_time" in d

    def test_to_dict_with_iterations(self):
        """Test dictionary conversion includes recent iterations."""
        stats = IterationStats()
        stats.record_iteration(1, 2.5, True, "")
        stats.record_iteration(2, 3.0, False, "Error occurred")

        d = stats.to_dict()
        # The base to_dict doesn't include iterations for backwards compatibility
        # but we should have get_recent_iterations() or similar
        assert d["total"] == 2
        assert d["successes"] == 1
        assert d["failures"] == 1

    def test_get_recent_iterations(self):
        """Test getting recent iterations for detailed stats."""
        stats = IterationStats()
        for i in range(5):
            stats.record_iteration(i, float(i), i % 2 == 0, f"err{i}" if i % 2 != 0 else "")

        recent = stats.get_recent_iterations(3)
        assert len(recent) == 3
        # Should be most recent 3 (iterations 2, 3, 4)
        assert recent[0]["iteration"] == 2
        assert recent[-1]["iteration"] == 4

    def test_get_recent_iterations_all(self):
        """Test getting all iterations when count exceeds stored."""
        stats = IterationStats()
        for i in range(3):
            stats.record_iteration(i, 1.0, True, "")

        recent = stats.get_recent_iterations(10)
        assert len(recent) == 3

    def test_get_average_duration(self):
        """Test average iteration duration calculation."""
        stats = IterationStats()
        stats.record_iteration(1, 2.0, True, "")
        stats.record_iteration(2, 4.0, True, "")
        stats.record_iteration(3, 6.0, True, "")

        avg = stats.get_average_duration()
        assert avg == 4.0

    def test_get_average_duration_no_iterations(self):
        """Test average duration with no iterations."""
        stats = IterationStats()
        assert stats.get_average_duration() == 0.0

    def test_get_error_messages(self):
        """Test extracting error messages from failed iterations."""
        stats = IterationStats()
        stats.record_iteration(1, 1.0, True, "")
        stats.record_iteration(2, 1.0, False, "Error A")
        stats.record_iteration(3, 1.0, False, "Error B")
        stats.record_iteration(4, 1.0, True, "")

        errors = stats.get_error_messages()
        assert len(errors) == 2
        assert "Error A" in errors
        assert "Error B" in errors

    def test_get_error_messages_empty(self):
        """Test error messages when all iterations succeed."""
        stats = IterationStats()
        stats.record_iteration(1, 1.0, True, "")

        errors = stats.get_error_messages()
        assert errors == []

    def test_backwards_compatibility_with_metrics(self):
        """Test that IterationStats can work alongside Metrics class."""
        metrics = Metrics()
        stats = IterationStats()

        # Both should coexist and track independently
        metrics.successful_iterations = 5
        stats.record_success(1)

        assert metrics.successful_iterations == 5
        assert stats.successes == 1

    def test_custom_max_iterations_stored(self):
        """Test setting custom max iterations limit."""
        stats = IterationStats(max_iterations_stored=50)
        assert stats.max_iterations_stored == 50

        # Add 60 iterations
        for i in range(60):
            stats.record_iteration(i, 1.0, True, "")

        assert len(stats.iterations) == 50


class TestTriggerReason:
    """Test TriggerReason enum."""

    def test_enum_values_exist(self):
        """Test all expected enum values exist."""
        assert TriggerReason.INITIAL.value == "initial"
        assert TriggerReason.TASK_INCOMPLETE.value == "task_incomplete"
        assert TriggerReason.PREVIOUS_SUCCESS.value == "previous_success"
        assert TriggerReason.RECOVERY.value == "recovery"
        assert TriggerReason.LOOP_DETECTED.value == "loop_detected"
        assert TriggerReason.SAFETY_LIMIT.value == "safety_limit"
        assert TriggerReason.USER_STOP.value == "user_stop"

    def test_enum_is_string(self):
        """Test TriggerReason inherits from str for JSON serialization."""
        assert isinstance(TriggerReason.INITIAL, str)
        assert TriggerReason.INITIAL == "initial"

    def test_enum_count(self):
        """Test expected number of trigger reasons."""
        assert len(TriggerReason) == 7


class TestIterationStatsTelemetry:
    """Test new telemetry fields in IterationStats."""

    def test_record_iteration_with_trigger_reason(self):
        """Test recording iteration with trigger reason."""
        stats = IterationStats()
        stats.record_iteration(
            iteration=1,
            duration=2.5,
            success=True,
            error="",
            trigger_reason=TriggerReason.INITIAL.value,
        )

        assert len(stats.iterations) == 1
        assert stats.iterations[0]["trigger_reason"] == "initial"

    def test_record_iteration_with_all_telemetry_fields(self):
        """Test recording iteration with all telemetry fields."""
        stats = IterationStats()
        stats.record_iteration(
            iteration=1,
            duration=5.0,
            success=True,
            error="",
            trigger_reason=TriggerReason.TASK_INCOMPLETE.value,
            output_preview="Task completed successfully",
            tokens_used=1500,
            cost=0.025,
            tools_used=["Read", "Edit", "Bash"],
        )

        iter_data = stats.iterations[0]
        assert iter_data["trigger_reason"] == "task_incomplete"
        assert iter_data["output_preview"] == "Task completed successfully"
        assert iter_data["tokens_used"] == 1500
        assert iter_data["cost"] == 0.025
        assert iter_data["tools_used"] == ["Read", "Edit", "Bash"]

    def test_output_preview_truncation(self):
        """Test output preview is truncated at 500 characters."""
        stats = IterationStats()
        long_output = "x" * 600  # 600 chars, exceeds 500 limit

        stats.record_iteration(
            iteration=1,
            duration=1.0,
            success=True,
            error="",
            output_preview=long_output,
        )

        preview = stats.iterations[0]["output_preview"]
        # Should be 500 chars + "..." = 503 chars total
        assert len(preview) == 503
        assert preview.endswith("...")
        assert preview[:500] == "x" * 500

    def test_output_preview_under_limit_not_truncated(self):
        """Test output preview under limit is not truncated."""
        stats = IterationStats()
        short_output = "x" * 400  # Under 500 limit

        stats.record_iteration(
            iteration=1,
            duration=1.0,
            success=True,
            error="",
            output_preview=short_output,
        )

        preview = stats.iterations[0]["output_preview"]
        assert len(preview) == 400
        assert not preview.endswith("...")

    def test_output_preview_exactly_at_limit(self):
        """Test output preview exactly at limit is not truncated."""
        stats = IterationStats()
        exact_output = "x" * 500  # Exactly 500 chars

        stats.record_iteration(
            iteration=1,
            duration=1.0,
            success=True,
            error="",
            output_preview=exact_output,
        )

        preview = stats.iterations[0]["output_preview"]
        assert len(preview) == 500
        assert not preview.endswith("...")

    def test_tools_used_defaults_to_empty_list(self):
        """Test tools_used defaults to empty list when not provided."""
        stats = IterationStats()
        stats.record_iteration(
            iteration=1,
            duration=1.0,
            success=True,
            error="",
        )

        assert stats.iterations[0]["tools_used"] == []

    def test_trigger_reason_defaults_to_empty_string(self):
        """Test trigger_reason defaults to empty string when not provided."""
        stats = IterationStats()
        stats.record_iteration(
            iteration=1,
            duration=1.0,
            success=True,
            error="",
        )

        assert stats.iterations[0]["trigger_reason"] == ""

    def test_multiple_iterations_with_different_triggers(self):
        """Test tracking multiple iterations with different trigger reasons."""
        stats = IterationStats()

        # Simulate orchestration flow
        stats.record_iteration(1, 2.0, True, "", trigger_reason=TriggerReason.INITIAL.value)
        stats.record_iteration(2, 3.0, True, "", trigger_reason=TriggerReason.TASK_INCOMPLETE.value)
        stats.record_iteration(3, 1.5, False, "Error occurred", trigger_reason=TriggerReason.TASK_INCOMPLETE.value)
        stats.record_iteration(4, 2.5, True, "", trigger_reason=TriggerReason.RECOVERY.value)

        assert len(stats.iterations) == 4
        assert stats.iterations[0]["trigger_reason"] == "initial"
        assert stats.iterations[1]["trigger_reason"] == "task_incomplete"
        assert stats.iterations[2]["trigger_reason"] == "task_incomplete"
        assert stats.iterations[3]["trigger_reason"] == "recovery"

    def test_cost_accumulation_tracking(self):
        """Test that cost is properly stored per iteration."""
        stats = IterationStats()

        stats.record_iteration(1, 2.0, True, "", cost=0.01)
        stats.record_iteration(2, 3.0, True, "", cost=0.02)
        stats.record_iteration(3, 1.5, True, "", cost=0.015)

        costs = [it["cost"] for it in stats.iterations]
        assert costs == [0.01, 0.02, 0.015]
        # Note: IterationStats doesn't track total cost - that's CostTracker's job
        # But we can verify each iteration stores its cost

    def test_tokens_used_tracking(self):
        """Test that tokens_used is properly stored per iteration."""
        stats = IterationStats()

        stats.record_iteration(1, 2.0, True, "", tokens_used=1000)
        stats.record_iteration(2, 3.0, True, "", tokens_used=1500)
        stats.record_iteration(3, 1.5, True, "", tokens_used=800)

        tokens = [it["tokens_used"] for it in stats.iterations]
        assert tokens == [1000, 1500, 800]

    def test_backward_compatibility_old_record_iteration_call(self):
        """Test backward compatibility with old record_iteration() signature."""
        stats = IterationStats()

        # Old-style call with positional args (pre-telemetry)
        stats.record_iteration(1, 5.5, True, "")

        # Should still work, with new fields defaulting appropriately
        iter_data = stats.iterations[0]
        assert iter_data["iteration"] == 1
        assert iter_data["duration"] == 5.5
        assert iter_data["success"] is True
        assert iter_data["error"] == ""
        assert iter_data["trigger_reason"] == ""
        assert iter_data["output_preview"] == ""
        assert iter_data["tokens_used"] == 0
        assert iter_data["cost"] == 0.0
        assert iter_data["tools_used"] == []

    def test_custom_max_preview_length(self):
        """Test configurable max_preview_length."""
        stats = IterationStats(max_preview_length=100)
        long_output = "x" * 150  # Exceeds custom limit

        stats.record_iteration(
            iteration=1,
            duration=1.0,
            success=True,
            error="",
            output_preview=long_output,
        )

        preview = stats.iterations[0]["output_preview"]
        # Should be 100 chars + "..." = 103 chars total
        assert len(preview) == 103
        assert preview.endswith("...")
        assert preview[:100] == "x" * 100

    def test_custom_max_preview_length_small(self):
        """Test very small max_preview_length."""
        stats = IterationStats(max_preview_length=10)
        output = "Hello World Test"  # 16 chars

        stats.record_iteration(
            iteration=1,
            duration=1.0,
            success=True,
            error="",
            output_preview=output,
        )

        preview = stats.iterations[0]["output_preview"]
        assert preview == "Hello Worl..."
        assert len(preview) == 13  # 10 + "..."

    def test_default_max_preview_length(self):
        """Test default max_preview_length is 500."""
        stats = IterationStats()
        assert stats.max_preview_length == 500
