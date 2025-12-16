# ABOUTME: Tests for loop detection feature using rapidfuzz
# ABOUTME: Validates similarity-based output comparison to prevent infinite loops

"""Tests for loop detection in Ralph Orchestrator."""

import unittest

from ralph_orchestrator.safety import SafetyGuard


class TestLoopDetection(unittest.TestCase):
    """Test loop detection functionality in SafetyGuard."""

    def setUp(self):
        """Set up test fixtures."""
        self.guard = SafetyGuard(
            max_iterations=100,
            max_runtime=3600,
            max_cost=50.0
        )

    def test_loop_detected_identical_outputs(self):
        """Test loop detection with identical outputs."""
        output = "Checking the database for user information..."

        # First output - no loop
        self.assertFalse(self.guard.detect_loop(output))

        # Same output again - loop detected
        self.assertTrue(self.guard.detect_loop(output))

    def test_loop_detected_similar_outputs(self):
        """Test loop detection with highly similar outputs."""
        output1 = "I'm checking the database for user information now."
        output2 = "I'm checking the database for user information here."

        # First output
        self.assertFalse(self.guard.detect_loop(output1))

        # Very similar output - should trigger loop (>90% similar)
        self.assertTrue(self.guard.detect_loop(output2))

    def test_no_loop_different_outputs(self):
        """Test no false positive with distinctly different outputs."""
        outputs = [
            "Step 1: Creating the configuration file",
            "Step 2: Installing dependencies from requirements.txt",
            "Step 3: Running the test suite",
            "Step 4: Building the documentation",
            "Step 5: Deploying to production server",
        ]

        for output in outputs:
            self.assertFalse(self.guard.detect_loop(output))

    def test_loop_detection_window_size(self):
        """Test that loop detection uses sliding window of 5.

        The deque has maxlen=5, so the 6th output pushes out the 1st.
        We use substantively different messages to avoid similarity triggers.
        """
        # Use completely different messages (not just numbered variants)
        different_messages = [
            "Starting database connection and initializing schema",
            "Parsing configuration from YAML file successfully",
            "Downloading dependencies from remote repository",
            "Compiling TypeScript source files to JavaScript",
            "Running integration tests against staging server",
            "Deploying application to production cluster now",
        ]

        # Add 6 different messages - none should trigger
        for msg in different_messages:
            self.assertFalse(self.guard.detect_loop(msg))

        # Window now contains messages 2-6 (message 1 was pushed out)
        # Adding message 1 back should NOT trigger - it's no longer in window
        self.assertFalse(self.guard.detect_loop(different_messages[0]))

    def test_loop_detection_empty_output(self):
        """Test handling of empty output."""
        self.assertFalse(self.guard.detect_loop(""))
        self.assertFalse(self.guard.detect_loop(None))

    def test_loop_detection_short_outputs(self):
        """Test loop detection with very short outputs."""
        # Short identical outputs should still trigger
        self.assertFalse(self.guard.detect_loop("Done"))
        self.assertTrue(self.guard.detect_loop("Done"))

    def test_reset_clears_loop_history(self):
        """Test that reset() clears loop detection history."""
        output = "Repeating output pattern"

        # Add to history
        self.guard.detect_loop(output)
        self.assertEqual(len(self.guard.recent_outputs), 1)

        # Reset should clear
        self.guard.reset()
        self.assertEqual(len(self.guard.recent_outputs), 0)

        # Same output should no longer trigger loop
        self.assertFalse(self.guard.detect_loop(output))

    def test_loop_threshold_boundary(self):
        """Test behavior at similarity threshold boundary."""
        # These outputs are similar but should be below 90% threshold
        output1 = "Processing data from the input source now"
        output2 = "Computing results from the output target here"

        self.assertFalse(self.guard.detect_loop(output1))
        # Different enough to not trigger
        self.assertFalse(self.guard.detect_loop(output2))

    def test_loop_detection_with_numbers(self):
        """Test loop detection with outputs containing numbers.

        Note: Outputs that differ only by a single number are still >90% similar
        and WILL trigger loop detection. This is intentional - if an agent is
        just incrementing a counter but otherwise producing identical output,
        that's still a loop pattern we want to catch.
        """
        # First output - no loop
        self.assertFalse(self.guard.detect_loop("Iteration 1: Processing batch"))

        # Second output differs only by number - still triggers (>90% similar)
        self.assertTrue(self.guard.detect_loop("Iteration 2: Processing batch"))

    def test_no_loop_with_substantively_different_outputs(self):
        """Test that substantively different outputs don't trigger loop."""
        # These outputs describe genuinely different actions
        self.assertFalse(self.guard.detect_loop("Step 1: Reading configuration file"))
        self.assertFalse(self.guard.detect_loop("Step 2: Connecting to database"))
        self.assertFalse(self.guard.detect_loop("Step 3: Executing query and fetching results"))

    def test_loop_detection_multiline(self):
        """Test loop detection with multiline outputs."""
        output1 = """Starting task...
Working on step 1
Working on step 2
Task complete."""

        output2 = """Starting task...
Working on step 1
Working on step 2
Task complete."""

        self.assertFalse(self.guard.detect_loop(output1))
        self.assertTrue(self.guard.detect_loop(output2))

    def test_consecutive_failures_independent(self):
        """Test that loop detection is independent of failure tracking."""
        # Record some failures
        self.guard.record_failure()
        self.guard.record_failure()
        self.assertEqual(self.guard.consecutive_failures, 2)

        # Loop detection should still work
        output = "Test output"
        self.assertFalse(self.guard.detect_loop(output))
        self.assertTrue(self.guard.detect_loop(output))

        # Failures should be unchanged
        self.assertEqual(self.guard.consecutive_failures, 2)


class TestLoopDetectionIntegration(unittest.TestCase):
    """Integration tests for loop detection with SafetyGuard."""

    def test_safety_check_passes_with_loop_detection(self):
        """Test that safety check still works alongside loop detection."""
        guard = SafetyGuard(
            max_iterations=10,
            max_runtime=60,
            max_cost=1.0
        )

        # Safety check should pass initially
        result = guard.check(iterations=0, elapsed_time=0, total_cost=0)
        self.assertTrue(result.passed)

        # Loop detection is separate from safety check
        guard.detect_loop("Output 1")
        guard.detect_loop("Output 2")

        # Safety check should still pass
        result = guard.check(iterations=1, elapsed_time=10, total_cost=0.1)
        self.assertTrue(result.passed)

    def test_loop_detection_after_max_iterations(self):
        """Test loop detection state after hitting max iterations."""
        guard = SafetyGuard(max_iterations=2)

        # Hit max iterations
        result = guard.check(iterations=2, elapsed_time=10, total_cost=0)
        self.assertFalse(result.passed)

        # Loop detection should still function
        self.assertFalse(guard.detect_loop("Final output"))


if __name__ == "__main__":
    unittest.main()
