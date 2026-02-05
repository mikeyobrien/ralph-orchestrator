"""LLM-as-judge validation using Claude Agent SDK."""

import asyncio
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional, Any


@dataclass
class CheckResult:
    """Result of a single validation check."""

    passed: bool
    reason: str


@dataclass
class JudgeResult:
    """Result of LLM-as-judge validation."""

    passed: bool
    checks: dict[str, CheckResult] = field(default_factory=dict)
    overall_reason: str = ""
    raw_response: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "passed": self.passed,
            "checks": {
                name: {"passed": check.passed, "reason": check.reason}
                for name, check in self.checks.items()
            },
            "overall_reason": self.overall_reason,
            "raw_response": self.raw_response,
        }


# Default validation criteria for idle timeout TUI state
IDLE_TIMEOUT_CRITERIA = """
Analyze this terminal output and validate:

1. **Session Completed**: The session shows some evidence of completion or termination
   - Look for: return to shell prompt, "done" state, "terminated", "timeout", or process end
   - Pass if there is ANY indication the process finished

2. **No Critical Errors**: The output doesn't show critical/unexpected errors
   - ANSI escape codes (like [0m, [32m) are EXPECTED - these are not errors
   - Pass if the text is generally readable despite ANSI codes
   - Only fail for truly corrupted/garbage output

3. **Content Present**: There is actual content visible
   - Not just empty lines
   - Some meaningful output was captured

Respond with ONLY valid JSON (no markdown, no extra text):
{
  "pass": true/false,
  "checks": {
    "session_completed": {"pass": true/false, "reason": "explanation"},
    "no_critical_errors": {"pass": true/false, "reason": "explanation"},
    "content_present": {"pass": true/false, "reason": "explanation"}
  },
  "overall_reason": "Summary of validation result"
}
"""


# Validation criteria for iteration counter in TUI header
ITERATION_COUNTER_CRITERIA = """
Analyze this TUI capture and validate the iteration display:

1. **Iteration Display**: Header shows iteration in format [iter N]
   - Look for text like "[iter 1]", "[iter 2]", "[iter 3]", etc.
   - Extract the iteration number N
   - Expected iteration: {expected_iteration}
   - Pass if the displayed iteration matches the expected value

2. **Elapsed Time**: Header shows elapsed time
   - Look for time in format MM:SS (e.g., "00:05", "01:23")
   - Time should be visible and non-zero if iteration > 1
   - Pass if any time format is visible

3. **Mode Indicator**: Shows current mode
   - Look for: "auto", "interactive", or mode indicator
   - Should be visible in the header area
   - Pass if ANY mode indication is present

4. **No Visual Artifacts**: TUI renders properly
   - Content should be readable
   - ANSI escape codes are expected - they are NOT errors
   - Pass if text is generally readable

Respond with ONLY valid JSON (no markdown, no extra text):
{{
  "pass": true/false,
  "iteration_found": <number or null>,
  "elapsed_time": "<time string or null>",
  "checks": {{
    "iteration_correct": {{"pass": true/false, "reason": "explanation"}},
    "elapsed_visible": {{"pass": true/false, "reason": "explanation"}},
    "mode_visible": {{"pass": true/false, "reason": "explanation"}},
    "no_artifacts": {{"pass": true/false, "reason": "explanation"}}
  }},
  "overall_reason": "Summary of validation result"
}}
"""


# Validation criteria for max iterations termination
MAX_ITERATIONS_CRITERIA = """
Analyze this TUI capture from a terminated Hats session:

1. **Final Iteration**: Shows the maximum iteration reached
   - Look for [iter N] where N is the expected max
   - Expected max iterations: {max_iterations}
   - Pass if final iteration shows N or N-1 (may capture during transition)

2. **Termination Indication**: Shows loop terminated
   - Look for: "max iterations", "limit reached", "terminated", "Loop terminated"
   - Or shell prompt returned ($ or >)
   - Pass if ANY termination indication is present

3. **No Active Processing**: Session has stopped
   - Activity indicator shows stopped or no active indicator visible
   - No "thinking" or "working" indicators active
   - Pass if session appears inactive

Respond with ONLY valid JSON (no markdown, no extra text):
{{
  "pass": true/false,
  "final_iteration": <number or null>,
  "checks": {{
    "correct_final_iteration": {{"pass": true/false, "reason": "explanation"}},
    "termination_shown": {{"pass": true/false, "reason": "explanation"}},
    "session_inactive": {{"pass": true/false, "reason": "explanation"}}
  }},
  "overall_reason": "Summary of validation result"
}}
"""


# Validation criteria for successful completion
COMPLETION_CRITERIA = """
Analyze this TUI capture from a completed Hats session:

1. **Completion State**: Session completed successfully
   - Look for: "completed", "done", "finished", "success", shell prompt return
   - Should NOT show error states or failure indicators
   - Pass if completion is indicated

2. **No Error States**: No critical errors shown
   - No "error", "failed", "panic", "crash" messages
   - ANSI escape codes are expected - they are NOT errors
   - Pass if no critical error messages visible

3. **Content Present**: Meaningful output was captured
   - Not just empty lines or whitespace
   - Some actual content visible
   - Pass if content exists

Respond with ONLY valid JSON (no markdown, no extra text):
{{
  "pass": true/false,
  "checks": {{
    "completion_shown": {{"pass": true/false, "reason": "explanation"}},
    "no_errors": {{"pass": true/false, "reason": "explanation"}},
    "content_present": {{"pass": true/false, "reason": "explanation"}}
  }},
  "overall_reason": "Summary of validation result"
}}
"""


class LLMJudge:
    """Validates TUI output using Claude as an LLM-as-judge.

    Uses Claude Agent SDK with Haiku model for fast, cheap validation.
    """

    def __init__(self, model: str = "haiku"):
        """Initialize the LLM judge.

        Args:
            model: Claude model to use. SDK uses simplified names: "haiku", "sonnet", "opus".
                   Defaults to Haiku for speed/cost.
        """
        self.model = model

    async def validate(
        self,
        content: str,
        criteria: str = IDLE_TIMEOUT_CRITERIA,
    ) -> JudgeResult:
        """Validate terminal content against criteria.

        Args:
            content: Terminal content to validate (raw text or from capture)
            criteria: Validation criteria prompt

        Returns:
            JudgeResult with validation outcome
        """
        from claude_agent_sdk import query, ClaudeAgentOptions, AssistantMessage, TextBlock

        prompt = f"""{criteria}

TERMINAL OUTPUT TO ANALYZE:
```
{content}
```"""

        options = ClaudeAgentOptions(
            model=self.model,
            max_turns=1,
        )

        response_text = ""
        async for message in query(prompt=prompt, options=options):
            if isinstance(message, AssistantMessage):
                for block in message.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text

        return self._parse_response(response_text)

    async def validate_image(
        self,
        image_path: Path,
        criteria: str = IDLE_TIMEOUT_CRITERIA,
    ) -> JudgeResult:
        """Validate a screenshot image against criteria.

        Args:
            image_path: Path to PNG/SVG image
            criteria: Validation criteria prompt

        Returns:
            JudgeResult with validation outcome
        """
        from claude_agent_sdk import query, ClaudeAgentOptions, AssistantMessage, TextBlock

        prompt = f"""{criteria}

Please read and analyze the image at: {image_path}
"""

        options = ClaudeAgentOptions(
            model=self.model,
            max_turns=2,  # One turn to read image, one to respond
            allowed_tools=["Read"],  # Allow reading the image file
        )

        response_text = ""
        async for message in query(prompt=prompt, options=options):
            if isinstance(message, AssistantMessage):
                for block in message.content:
                    if isinstance(block, TextBlock):
                        response_text += block.text

        return self._parse_response(response_text)

    def _parse_response(self, response: str) -> JudgeResult:
        """Parse LLM response into structured JudgeResult.

        Args:
            response: Raw LLM response text

        Returns:
            Parsed JudgeResult
        """
        # Try to extract JSON from response
        try:
            # Handle potential markdown code blocks
            json_str = response
            if "```json" in response:
                json_str = response.split("```json")[1].split("```")[0]
            elif "```" in response:
                json_str = response.split("```")[1].split("```")[0]

            data = json.loads(json_str.strip())

            checks = {}
            if "checks" in data:
                for name, check_data in data["checks"].items():
                    checks[name] = CheckResult(
                        passed=check_data.get("pass", False),
                        reason=check_data.get("reason", ""),
                    )

            return JudgeResult(
                passed=data.get("pass", False),
                checks=checks,
                overall_reason=data.get("overall_reason", ""),
                raw_response=response,
            )
        except (json.JSONDecodeError, KeyError, IndexError) as e:
            # If parsing fails, try to infer from response
            passed = "pass" in response.lower() and "fail" not in response.lower()
            return JudgeResult(
                passed=passed,
                overall_reason=f"Failed to parse structured response: {e}",
                raw_response=response,
            )

    @staticmethod
    def is_available() -> bool:
        """Check if Claude Agent SDK is available."""
        try:
            import claude_agent_sdk
            return True
        except ImportError:
            return False
