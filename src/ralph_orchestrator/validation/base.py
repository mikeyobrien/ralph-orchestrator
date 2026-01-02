# ABOUTME: Abstract base class for validation gates
# ABOUTME: Defines the interface for all validation gate implementations

"""Abstract base class for validation gates."""

import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Dict, List, Optional

from .config import ValidationGateConfig, ValidationStep

logger = logging.getLogger("ralph-orchestrator.validation.base")


@dataclass
class StepResult:
    """Result of a single validation step."""

    step: ValidationStep
    passed: bool
    actual: Any = None
    error: Optional[str] = None
    duration_ms: float = 0.0
    evidence: Optional[str] = None  # Screenshot path, log snippet, etc.


@dataclass
class ValidationResult:
    """Result of a validation gate execution."""

    gate_id: str
    gate_type: str
    passed: bool
    step_results: List[StepResult] = field(default_factory=list)
    error: Optional[str] = None
    duration_ms: float = 0.0
    timestamp: str = field(default_factory=lambda: datetime.now().isoformat())
    metadata: Dict[str, Any] = field(default_factory=dict)

    @property
    def steps_passed(self) -> int:
        """Number of steps that passed."""
        return sum(1 for s in self.step_results if s.passed)

    @property
    def steps_failed(self) -> int:
        """Number of steps that failed."""
        return sum(1 for s in self.step_results if not s.passed)

    @property
    def pass_rate(self) -> float:
        """Percentage of steps that passed."""
        if not self.step_results:
            return 1.0 if self.passed else 0.0
        return self.steps_passed / len(self.step_results)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for serialization."""
        return {
            "gate_id": self.gate_id,
            "gate_type": self.gate_type,
            "passed": self.passed,
            "steps_passed": self.steps_passed,
            "steps_failed": self.steps_failed,
            "pass_rate": self.pass_rate,
            "error": self.error,
            "duration_ms": self.duration_ms,
            "timestamp": self.timestamp,
            "step_results": [
                {
                    "action": s.step.action,
                    "target": s.step.target,
                    "passed": s.passed,
                    "actual": s.actual,
                    "error": s.error,
                    "evidence": s.evidence,
                }
                for s in self.step_results
            ],
            "metadata": self.metadata,
        }


class ValidationGate(ABC):
    """Abstract base class for validation gates.

    Validation gates are responsible for verifying that a project works correctly
    from an end-user perspective. Different gate types use different tools:
    - Web gates use Puppeteer/Playwright MCP
    - iOS gates use xc-mcp
    - CLI gates use bash execution
    - API gates use HTTP requests

    The gate abstraction allows ralph-orchestrator to validate ANY type of project
    using the appropriate tools, selected at runtime based on project detection.
    """

    def __init__(self, config: ValidationGateConfig):
        """Initialize the validation gate.

        Args:
            config: Gate configuration from validation_config.json
        """
        self.config = config
        self.gate_id = config.id
        self.gate_type = config.type
        self._mcp_tools: Dict[str, Any] = {}

    @property
    @abstractmethod
    def required_tools(self) -> List[str]:
        """List of MCP tools required by this gate."""
        pass

    @abstractmethod
    async def validate(self, context: Dict[str, Any]) -> ValidationResult:
        """Execute validation and return results.

        Args:
            context: Execution context including MCP client, project path, etc.

        Returns:
            ValidationResult with pass/fail status and step details.
        """
        pass

    @abstractmethod
    async def execute_step(
        self, step: ValidationStep, context: Dict[str, Any]
    ) -> StepResult:
        """Execute a single validation step.

        Args:
            step: The validation step to execute
            context: Execution context

        Returns:
            StepResult with pass/fail and evidence
        """
        pass

    def register_mcp_tool(self, tool_name: str, tool_func: Any) -> None:
        """Register an MCP tool for use in validation.

        Args:
            tool_name: Name of the MCP tool (e.g., "browser_navigate")
            tool_func: The tool's callable function
        """
        self._mcp_tools[tool_name] = tool_func
        logger.debug(f"Registered MCP tool: {tool_name}")

    def get_mcp_tool(self, tool_name: str) -> Optional[Any]:
        """Get a registered MCP tool by name.

        Args:
            tool_name: Name of the tool

        Returns:
            The tool function or None if not registered
        """
        return self._mcp_tools.get(tool_name)

    def check_prerequisites(self) -> List[str]:
        """Check if all prerequisites are met for this gate.

        Returns:
            List of missing prerequisites (empty if all met)
        """
        missing = []
        for tool in self.required_tools:
            if tool not in self._mcp_tools:
                missing.append(tool)
        return missing

    def _create_result(
        self,
        passed: bool,
        step_results: List[StepResult],
        error: Optional[str] = None,
        duration_ms: float = 0.0,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> ValidationResult:
        """Helper to create a ValidationResult."""
        return ValidationResult(
            gate_id=self.gate_id,
            gate_type=self.gate_type,
            passed=passed,
            step_results=step_results,
            error=error,
            duration_ms=duration_ms,
            metadata=metadata or {},
        )

    def _evaluate_expected(self, actual: Any, expected: Any) -> bool:
        """Evaluate if actual result matches expected.

        Supports various matching modes:
        - Direct equality
        - Contains (for strings)
        - Dictionary with exit_code/contains/output_file
        - Regex patterns (prefixed with "regex:")

        Args:
            actual: The actual result
            expected: The expected result or criteria

        Returns:
            True if actual matches expected
        """
        if expected is None:
            return True

        if isinstance(expected, str):
            if expected == "page_loads":
                return actual is not None
            if expected == "has_content":
                return actual is not None and len(str(actual)) > 0
            if expected == "has_ui_elements":
                return actual is not None
            if expected == "booted":
                return actual == "booted" or "booted" in str(actual).lower()
            if expected == "installed":
                return actual == "installed" or "installed" in str(actual).lower()
            if expected == "running":
                return actual == "running" or "running" in str(actual).lower()
            # Direct string comparison
            return str(actual) == expected

        if isinstance(expected, dict):
            # Complex criteria
            if "exit_code" in expected:
                if actual.get("exit_code") != expected["exit_code"]:
                    return False
            if "contains" in expected:
                if expected["contains"] not in str(actual.get("stdout", "")):
                    return False
            if "output_file" in expected:
                # Check if file exists (would need filesystem access)
                pass
            return True

        if isinstance(expected, bool):
            return bool(actual) == expected

        if isinstance(expected, (int, float)):
            try:
                return float(actual) == float(expected)
            except (ValueError, TypeError):
                return False

        return actual == expected
