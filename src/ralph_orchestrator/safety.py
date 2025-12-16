# ABOUTME: Safety guardrails and circuit breakers for Ralph Orchestrator
# ABOUTME: Prevents runaway loops and excessive costs

"""Safety mechanisms for Ralph Orchestrator."""

from collections import deque
from dataclasses import dataclass
from typing import Optional
import logging

logger = logging.getLogger('ralph-orchestrator.safety')


@dataclass
class SafetyCheckResult:
    """Result of a safety check."""
    passed: bool
    reason: Optional[str] = None


class SafetyGuard:
    """Safety guardrails for orchestration."""
    
    def __init__(
        self,
        max_iterations: int = 100,
        max_runtime: int = 14400,  # 4 hours
        max_cost: float = 10.0,
        consecutive_failure_limit: int = 5
    ):
        """Initialize safety guard.
        
        Args:
            max_iterations: Maximum allowed iterations
            max_runtime: Maximum runtime in seconds
            max_cost: Maximum allowed cost in dollars
            consecutive_failure_limit: Max consecutive failures before stopping
        """
        self.max_iterations = max_iterations
        self.max_runtime = max_runtime
        self.max_cost = max_cost
        self.consecutive_failure_limit = consecutive_failure_limit
        self.consecutive_failures = 0
        # Loop detection state
        self.recent_outputs: deque = deque(maxlen=5)
        self.loop_threshold: float = 0.9
    
    def check(
        self,
        iterations: int,
        elapsed_time: float,
        total_cost: float
    ) -> SafetyCheckResult:
        """Check all safety conditions.
        
        Args:
            iterations: Current iteration count
            elapsed_time: Elapsed time in seconds
            total_cost: Total cost so far
            
        Returns:
            SafetyCheckResult indicating if it's safe to continue
        """
        # Check iteration limit
        if iterations >= self.max_iterations:
            return SafetyCheckResult(
                passed=False,
                reason=f"Reached maximum iterations ({self.max_iterations})"
            )
        
        # Check runtime limit
        if elapsed_time >= self.max_runtime:
            hours = elapsed_time / 3600
            return SafetyCheckResult(
                passed=False,
                reason=f"Reached maximum runtime ({hours:.1f} hours)"
            )
        
        # Check cost limit
        if total_cost >= self.max_cost:
            return SafetyCheckResult(
                passed=False,
                reason=f"Reached maximum cost (${total_cost:.2f})"
            )
        
        # Check consecutive failures
        if self.consecutive_failures >= self.consecutive_failure_limit:
            return SafetyCheckResult(
                passed=False,
                reason=f"Too many consecutive failures ({self.consecutive_failures})"
            )
        
        # Additional safety checks for high iteration counts
        if iterations > 50:
            # Warn but don't stop
            logger.warning(f"High iteration count: {iterations}")
        
        if iterations > 75:
            # More aggressive checks
            if elapsed_time / iterations > 300:  # More than 5 min per iteration avg
                return SafetyCheckResult(
                    passed=False,
                    reason="Iterations taking too long on average"
                )
        
        return SafetyCheckResult(passed=True)
    
    def record_success(self):
        """Record a successful iteration."""
        self.consecutive_failures = 0
    
    def record_failure(self):
        """Record a failed iteration."""
        self.consecutive_failures += 1
        logger.warning(f"Consecutive failures: {self.consecutive_failures}")
    
    def reset(self):
        """Reset safety counters."""
        self.consecutive_failures = 0
        self.recent_outputs.clear()

    def detect_loop(self, current_output: str) -> bool:
        """Detect if agent is looping based on output similarity.

        Uses rapidfuzz for fast fuzzy string matching. If the current output
        is more than 90% similar to any recent output, a loop is detected.

        Args:
            current_output: The current agent output to check.

        Returns:
            True if loop detected (similar output found), False otherwise.
        """
        if not current_output:
            return False

        try:
            from rapidfuzz import fuzz

            for prev_output in self.recent_outputs:
                ratio = fuzz.ratio(current_output, prev_output) / 100.0
                if ratio >= self.loop_threshold:
                    logger.warning(
                        f"Loop detected: {ratio:.1%} similarity to previous output"
                    )
                    return True

            self.recent_outputs.append(current_output)
            return False
        except ImportError:
            # rapidfuzz not installed, skip loop detection
            logger.debug("rapidfuzz not installed, skipping loop detection")
            return False
        except Exception as e:
            logger.warning(f"Error in loop detection: {e}")
            return False