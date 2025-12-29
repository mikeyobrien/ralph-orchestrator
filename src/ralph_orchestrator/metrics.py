# ABOUTME: Metrics tracking and cost calculation for Ralph Orchestrator
# ABOUTME: Monitors performance, usage, and costs across different AI tools

"""Metrics and cost tracking for Ralph Orchestrator."""

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Dict, List, Any
import time
import json


class TriggerReason(str, Enum):
    """Reasons why an iteration was triggered.

    Used for per-iteration telemetry to understand why the orchestrator
    started each iteration, enabling analysis of orchestration patterns.
    """
    INITIAL = "initial"              # First iteration of a session
    TASK_INCOMPLETE = "task_incomplete"  # Previous iteration didn't complete task
    PREVIOUS_SUCCESS = "previous_success"  # Previous iteration succeeded, continuing
    RECOVERY = "recovery"            # Recovering from a previous failure
    LOOP_DETECTED = "loop_detected"  # Loop detection triggered intervention
    SAFETY_LIMIT = "safety_limit"    # Safety limits triggered
    USER_STOP = "user_stop"          # User requested stop


@dataclass
class Metrics:
    """Track orchestration metrics."""
    
    iterations: int = 0
    successful_iterations: int = 0
    failed_iterations: int = 0
    errors: int = 0
    checkpoints: int = 0
    rollbacks: int = 0
    start_time: float = field(default_factory=time.time)
    
    def elapsed_hours(self) -> float:
        """Get elapsed time in hours."""
        return (time.time() - self.start_time) / 3600
    
    def success_rate(self) -> float:
        """Calculate success rate."""
        total = self.successful_iterations + self.failed_iterations
        if total == 0:
            return 0.0
        return self.successful_iterations / total
    
    def to_dict(self) -> Dict:
        """Convert to dictionary."""
        return {
            "iterations": self.iterations,
            "successful_iterations": self.successful_iterations,
            "failed_iterations": self.failed_iterations,
            "errors": self.errors,
            "checkpoints": self.checkpoints,
            "rollbacks": self.rollbacks,
            "elapsed_hours": self.elapsed_hours(),
            "success_rate": self.success_rate()
        }
    
    def to_json(self) -> str:
        """Convert to JSON string."""
        return json.dumps(self.to_dict(), indent=2)


class CostTracker:
    """Track costs across different AI tools."""
    
    # Cost per 1K tokens (approximate)
    COSTS = {
        "claude": {
            "input": 0.003,   # $3 per 1M input tokens
            "output": 0.015   # $15 per 1M output tokens
        },
        "gemini": {
            "input": 0.00025,  # $0.25 per 1M input tokens
            "output": 0.001    # $1 per 1M output tokens
        },
        "qchat": {
            "input": 0.0,      # Free/local
            "output": 0.0
        },
        "acp": {
            "input": 0.0,      # ACP doesn't provide billing info
            "output": 0.0      # Cost depends on underlying agent
        },
        "gpt-4": {
            "input": 0.03,     # $30 per 1M input tokens
            "output": 0.06     # $60 per 1M output tokens
        }
    }
    
    def __init__(self):
        """Initialize cost tracker."""
        self.total_cost = 0.0
        self.costs_by_tool: Dict[str, float] = {}
        self.usage_history: List[Dict] = []
    
    def add_usage(
        self,
        tool: str,
        input_tokens: int,
        output_tokens: int
    ) -> float:
        """Add usage and calculate cost.
        
        Args:
            tool: Name of the AI tool
            input_tokens: Number of input tokens
            output_tokens: Number of output tokens
            
        Returns:
            Cost for this usage
        """
        if tool not in self.COSTS:
            tool = "qchat"  # Default to free tier
        
        costs = self.COSTS[tool]
        input_cost = (input_tokens / 1000) * costs["input"]
        output_cost = (output_tokens / 1000) * costs["output"]
        total = input_cost + output_cost
        
        # Update tracking
        self.total_cost += total
        if tool not in self.costs_by_tool:
            self.costs_by_tool[tool] = 0.0
        self.costs_by_tool[tool] += total
        
        # Add to history
        self.usage_history.append({
            "timestamp": time.time(),
            "tool": tool,
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "cost": total
        })
        
        return total
    
    def get_summary(self) -> Dict:
        """Get cost summary."""
        return {
            "total_cost": self.total_cost,
            "costs_by_tool": self.costs_by_tool,
            "usage_count": len(self.usage_history),
            "average_cost": self.total_cost / len(self.usage_history) if self.usage_history else 0
        }
    
    def to_json(self) -> str:
        """Convert to JSON string."""
        return json.dumps(self.get_summary(), indent=2)


@dataclass
class IterationStats:
    """Memory-efficient iteration statistics tracking.

    Tracks per-iteration details (duration, success/failure, errors) while
    limiting stored iterations to prevent memory leaks in long-running sessions.
    """

    total: int = 0
    successes: int = 0
    failures: int = 0
    start_time: datetime | None = None
    current_iteration: int = 0
    iterations: List[Dict[str, Any]] = field(default_factory=list)
    max_iterations_stored: int = 1000  # Memory limit for stored iterations
    max_preview_length: int = 500  # Max chars for output preview truncation

    def __post_init__(self) -> None:
        """Initialize start time if not set."""
        if self.start_time is None:
            self.start_time = datetime.now()

    def record_start(self, iteration: int) -> None:
        """Record iteration start.

        Args:
            iteration: Iteration number
        """
        self.current_iteration = iteration
        self.total = max(self.total, iteration)

    def record_success(self, iteration: int) -> None:
        """Record successful iteration.

        Args:
            iteration: Iteration number
        """
        self.total = iteration
        self.successes += 1

    def record_failure(self, iteration: int) -> None:
        """Record failed iteration.

        Args:
            iteration: Iteration number
        """
        self.total = iteration
        self.failures += 1

    def record_iteration(
        self,
        iteration: int,
        duration: float,
        success: bool,
        error: str,
        trigger_reason: str = "",
        output_preview: str = "",
        tokens_used: int = 0,
        cost: float = 0.0,
        tools_used: List[str] | None = None,
    ) -> None:
        """Record iteration with full details.

        Args:
            iteration: Iteration number
            duration: Duration in seconds
            success: Whether iteration was successful
            error: Error message if any
            trigger_reason: Why this iteration was triggered (from TriggerReason)
            output_preview: Preview of iteration output (truncated for privacy)
            tokens_used: Total tokens consumed in this iteration
            cost: Cost in dollars for this iteration
            tools_used: List of tools/MCPs invoked during iteration
        """
        # Update basic statistics
        self.total = max(self.total, iteration)
        self.current_iteration = iteration

        if success:
            self.successes += 1
        else:
            self.failures += 1

        # Truncate output preview for privacy (configurable length)
        if output_preview and len(output_preview) > self.max_preview_length:
            output_preview = output_preview[:self.max_preview_length] + "..."

        # Store detailed iteration information
        iteration_data = {
            "iteration": iteration,
            "duration": duration,
            "success": success,
            "error": error,
            "timestamp": datetime.now().isoformat(),
            "trigger_reason": trigger_reason,
            "output_preview": output_preview,
            "tokens_used": tokens_used,
            "cost": cost,
            "tools_used": tools_used or [],
        }
        self.iterations.append(iteration_data)

        # Enforce memory limit by evicting oldest entries
        if len(self.iterations) > self.max_iterations_stored:
            excess = len(self.iterations) - self.max_iterations_stored
            self.iterations = self.iterations[excess:]

    def get_success_rate(self) -> float:
        """Calculate success rate as percentage.

        Returns:
            Success rate (0-100)
        """
        total_attempts = self.successes + self.failures
        if total_attempts == 0:
            return 0.0
        return (self.successes / total_attempts) * 100

    def get_runtime(self) -> str:
        """Get human-readable runtime duration.

        Returns:
            Runtime string (e.g., "2h 30m 15s")
        """
        if self.start_time is None:
            return "Unknown"

        delta = datetime.now() - self.start_time
        hours, remainder = divmod(int(delta.total_seconds()), 3600)
        minutes, seconds = divmod(remainder, 60)

        if hours > 0:
            return f"{hours}h {minutes}m {seconds}s"
        if minutes > 0:
            return f"{minutes}m {seconds}s"
        return f"{seconds}s"

    def get_recent_iterations(self, count: int) -> List[Dict[str, Any]]:
        """Get most recent iterations.

        Args:
            count: Maximum number of iterations to return

        Returns:
            List of recent iteration data dictionaries
        """
        if count >= len(self.iterations):
            return self.iterations.copy()
        return self.iterations[-count:]

    def get_average_duration(self) -> float:
        """Calculate average iteration duration.

        Returns:
            Average duration in seconds, or 0.0 if no iterations
        """
        if not self.iterations:
            return 0.0
        total_duration = sum(it["duration"] for it in self.iterations)
        return total_duration / len(self.iterations)

    def get_error_messages(self) -> List[str]:
        """Extract error messages from failed iterations.

        Returns:
            List of non-empty error messages
        """
        return [
            it["error"]
            for it in self.iterations
            if not it["success"] and it["error"]
        ]

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization.

        Returns:
            Stats as dictionary (excludes iteration list for compatibility)
        """
        return {
            "total": self.total,
            "current": self.current_iteration,
            "successes": self.successes,
            "failures": self.failures,
            "success_rate": self.get_success_rate(),
            "runtime": self.get_runtime(),
            "start_time": self.start_time.isoformat() if self.start_time else None,
        }