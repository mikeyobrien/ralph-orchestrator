# ABOUTME: Type-specific validation gate implementations
# ABOUTME: Web, iOS, CLI, and API validation gates using appropriate MCP tools

"""Type-specific validation gate implementations."""

import asyncio
import logging
import subprocess
import time
from typing import Any, Dict, List, Optional

from .base import StepResult, ValidationGate, ValidationResult
from .config import ValidationGateConfig, ValidationStep

logger = logging.getLogger("ralph-orchestrator.validation.gates")


class WebValidationGate(ValidationGate):
    """Validation gate for web applications using Puppeteer/Playwright MCP."""

    @property
    def required_tools(self) -> List[str]:
        """MCP tools required for web validation."""
        return [
            "browser_navigate",
            "browser_snapshot",
            "browser_click",
            "browser_type",
        ]

    async def validate(self, context: Dict[str, Any]) -> ValidationResult:
        """Execute web validation steps."""
        start_time = time.time()
        step_results = []
        all_passed = True

        for step in self.config.validation_steps:
            result = await self.execute_step(step, context)
            step_results.append(result)
            if not result.passed:
                all_passed = False
                # Continue to gather all results, don't break early

        duration_ms = (time.time() - start_time) * 1000
        return self._create_result(
            passed=all_passed,
            step_results=step_results,
            duration_ms=duration_ms,
            metadata={"browser": context.get("browser", "unknown")},
        )

    async def execute_step(
        self, step: ValidationStep, context: Dict[str, Any]
    ) -> StepResult:
        """Execute a single web validation step."""
        start_time = time.time()

        try:
            if step.action == "navigate":
                result = await self._navigate(step.target, context)
            elif step.action == "snapshot":
                result = await self._snapshot(context)
            elif step.action == "click":
                result = await self._click(step.target, context)
            elif step.action == "type":
                result = await self._type(step.target, context)
            elif step.action == "wait":
                result = await self._wait(step.target, context)
            else:
                result = {"error": f"Unknown action: {step.action}"}

            passed = self._evaluate_expected(result, step.expected)
            duration_ms = (time.time() - start_time) * 1000

            return StepResult(
                step=step,
                passed=passed,
                actual=result,
                duration_ms=duration_ms,
            )

        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            return StepResult(
                step=step,
                passed=False,
                error=str(e),
                duration_ms=duration_ms,
            )

    async def _navigate(self, url: str, context: Dict[str, Any]) -> Any:
        """Navigate to a URL using browser MCP."""
        tool = self.get_mcp_tool("browser_navigate")
        if tool:
            return await tool(url=url)
        logger.warning("browser_navigate tool not available")
        return None

    async def _snapshot(self, context: Dict[str, Any]) -> Any:
        """Take a page snapshot."""
        tool = self.get_mcp_tool("browser_snapshot")
        if tool:
            return await tool()
        logger.warning("browser_snapshot tool not available")
        return None

    async def _click(self, selector: str, context: Dict[str, Any]) -> Any:
        """Click an element."""
        tool = self.get_mcp_tool("browser_click")
        if tool:
            return await tool(element=selector, ref=selector)
        logger.warning("browser_click tool not available")
        return None

    async def _type(self, target: str, context: Dict[str, Any]) -> Any:
        """Type into an element."""
        # target format: "selector|text"
        parts = target.split("|", 1)
        if len(parts) != 2:
            return {"error": "Invalid type target format"}
        selector, text = parts
        tool = self.get_mcp_tool("browser_type")
        if tool:
            return await tool(element=selector, ref=selector, text=text, submit=False)
        logger.warning("browser_type tool not available")
        return None

    async def _wait(self, target: str, context: Dict[str, Any]) -> Any:
        """Wait for a condition."""
        try:
            seconds = float(target)
            await asyncio.sleep(seconds)
            return {"waited": seconds}
        except ValueError:
            return {"error": f"Invalid wait time: {target}"}


class iOSValidationGate(ValidationGate):
    """Validation gate for iOS apps using xc-mcp."""

    @property
    def required_tools(self) -> List[str]:
        """MCP tools required for iOS validation."""
        return [
            "simctl-boot",
            "simctl-install",
            "simctl-launch",
            "screenshot",
        ]

    async def validate(self, context: Dict[str, Any]) -> ValidationResult:
        """Execute iOS validation steps."""
        start_time = time.time()
        step_results = []
        all_passed = True

        for step in self.config.validation_steps:
            result = await self.execute_step(step, context)
            step_results.append(result)
            if not result.passed:
                all_passed = False

        duration_ms = (time.time() - start_time) * 1000
        return self._create_result(
            passed=all_passed,
            step_results=step_results,
            duration_ms=duration_ms,
            metadata={"simulator": context.get("simulator", "iPhone 15 Pro")},
        )

    async def execute_step(
        self, step: ValidationStep, context: Dict[str, Any]
    ) -> StepResult:
        """Execute a single iOS validation step."""
        start_time = time.time()

        try:
            if step.action == "boot_simulator":
                result = await self._boot_simulator(step.target, context)
            elif step.action == "install_app":
                result = await self._install_app(step.target, context)
            elif step.action == "launch_app":
                result = await self._launch_app(step.target, context)
            elif step.action == "screenshot":
                result = await self._screenshot(context)
            elif step.action == "tap":
                result = await self._tap(step.target, context)
            elif step.action == "input":
                result = await self._input(step.target, context)
            else:
                result = {"error": f"Unknown action: {step.action}"}

            passed = self._evaluate_expected(result, step.expected)
            duration_ms = (time.time() - start_time) * 1000

            return StepResult(
                step=step,
                passed=passed,
                actual=result,
                duration_ms=duration_ms,
            )

        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            return StepResult(
                step=step,
                passed=False,
                error=str(e),
                duration_ms=duration_ms,
            )

    async def _boot_simulator(self, device_name: str, context: Dict[str, Any]) -> Any:
        """Boot an iOS simulator."""
        tool = self.get_mcp_tool("simctl-boot")
        if tool:
            return await tool(deviceId=device_name)
        # Fallback to direct simctl command
        proc = await asyncio.create_subprocess_exec(
            "xcrun", "simctl", "boot", device_name,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        _, stderr = await proc.communicate()
        if proc.returncode == 0 or "already booted" in stderr.decode():
            return "booted"
        return {"error": stderr.decode()}

    async def _install_app(self, app_path: str, context: Dict[str, Any]) -> Any:
        """Install an app to the simulator."""
        tool = self.get_mcp_tool("simctl-install")
        if tool:
            udid = context.get("simulator_udid", "booted")
            return await tool(udid=udid, appPath=app_path)
        # Fallback to direct simctl command
        proc = await asyncio.create_subprocess_exec(
            "xcrun", "simctl", "install", "booted", app_path,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        _, stderr = await proc.communicate()
        if proc.returncode == 0:
            return "installed"
        return {"error": stderr.decode()}

    async def _launch_app(self, bundle_id: str, context: Dict[str, Any]) -> Any:
        """Launch an app on the simulator."""
        tool = self.get_mcp_tool("simctl-launch")
        if tool:
            udid = context.get("simulator_udid", "booted")
            return await tool(udid=udid, bundleId=bundle_id)
        # Fallback to direct simctl command
        proc = await asyncio.create_subprocess_exec(
            "xcrun", "simctl", "launch", "booted", bundle_id,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, stderr = await proc.communicate()
        if proc.returncode == 0:
            return "running"
        return {"error": stderr.decode()}

    async def _screenshot(self, context: Dict[str, Any]) -> Any:
        """Take a screenshot of the simulator."""
        tool = self.get_mcp_tool("screenshot")
        if tool:
            udid = context.get("simulator_udid")
            return await tool(udid=udid)
        return {"error": "screenshot tool not available"}

    async def _tap(self, coords: str, context: Dict[str, Any]) -> Any:
        """Tap at coordinates on the simulator."""
        tool = self.get_mcp_tool("idb-ui-tap")
        if tool:
            parts = coords.split(",")
            if len(parts) == 2:
                x, y = int(parts[0]), int(parts[1])
                return await tool(x=x, y=y)
        return {"error": "idb-ui-tap tool not available or invalid coords"}

    async def _input(self, text: str, context: Dict[str, Any]) -> Any:
        """Input text on the simulator."""
        tool = self.get_mcp_tool("idb-ui-input")
        if tool:
            return await tool(operation="text", text=text)
        return {"error": "idb-ui-input tool not available"}


class CLIValidationGate(ValidationGate):
    """Validation gate for CLI tools using shell execution."""

    @property
    def required_tools(self) -> List[str]:
        """No MCP tools required for CLI validation."""
        return []  # Uses subprocess directly

    async def validate(self, context: Dict[str, Any]) -> ValidationResult:
        """Execute CLI validation steps."""
        start_time = time.time()
        step_results = []
        all_passed = True

        for step in self.config.validation_steps:
            result = await self.execute_step(step, context)
            step_results.append(result)
            if not result.passed:
                all_passed = False

        duration_ms = (time.time() - start_time) * 1000
        return self._create_result(
            passed=all_passed,
            step_results=step_results,
            duration_ms=duration_ms,
        )

    async def execute_step(
        self, step: ValidationStep, context: Dict[str, Any]
    ) -> StepResult:
        """Execute a single CLI validation step."""
        start_time = time.time()

        try:
            if step.action == "execute":
                result = await self._execute_command(step.target, context)
            elif step.action == "check_output":
                result = await self._check_output(step.target, context)
            elif step.action == "check_file":
                result = await self._check_file(step.target, context)
            else:
                result = {"error": f"Unknown action: {step.action}"}

            passed = self._evaluate_expected(result, step.expected)
            duration_ms = (time.time() - start_time) * 1000

            return StepResult(
                step=step,
                passed=passed,
                actual=result,
                duration_ms=duration_ms,
            )

        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            return StepResult(
                step=step,
                passed=False,
                error=str(e),
                duration_ms=duration_ms,
            )

    async def _execute_command(self, command: str, context: Dict[str, Any]) -> Dict[str, Any]:
        """Execute a shell command."""
        proc = await asyncio.create_subprocess_shell(
            command,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=context.get("project_path"),
        )
        stdout, stderr = await proc.communicate()

        return {
            "exit_code": proc.returncode,
            "stdout": stdout.decode() if stdout else "",
            "stderr": stderr.decode() if stderr else "",
        }

    async def _check_output(self, pattern: str, context: Dict[str, Any]) -> Dict[str, Any]:
        """Check the last command output for a pattern."""
        last_output = context.get("last_output", "")
        return {"contains": pattern in last_output, "pattern": pattern}

    async def _check_file(self, file_path: str, context: Dict[str, Any]) -> Dict[str, Any]:
        """Check if a file exists and optionally its content."""
        import os
        from pathlib import Path

        project_path = context.get("project_path", ".")
        full_path = Path(project_path) / file_path

        return {
            "exists": full_path.exists(),
            "is_file": full_path.is_file() if full_path.exists() else False,
            "size": full_path.stat().st_size if full_path.exists() else 0,
        }


class APIValidationGate(ValidationGate):
    """Validation gate for API endpoints using HTTP requests."""

    @property
    def required_tools(self) -> List[str]:
        """No MCP tools required for API validation (uses aiohttp)."""
        return []

    async def validate(self, context: Dict[str, Any]) -> ValidationResult:
        """Execute API validation steps."""
        start_time = time.time()
        step_results = []
        all_passed = True

        for step in self.config.validation_steps:
            result = await self.execute_step(step, context)
            step_results.append(result)
            if not result.passed:
                all_passed = False

        duration_ms = (time.time() - start_time) * 1000
        return self._create_result(
            passed=all_passed,
            step_results=step_results,
            duration_ms=duration_ms,
        )

    async def execute_step(
        self, step: ValidationStep, context: Dict[str, Any]
    ) -> StepResult:
        """Execute a single API validation step."""
        start_time = time.time()

        try:
            if step.action in ("GET", "POST", "PUT", "DELETE", "PATCH"):
                result = await self._http_request(step.action, step.target, context)
            elif step.action == "check_status":
                result = await self._check_status(step.target, context)
            elif step.action == "check_json":
                result = await self._check_json(step.target, context)
            else:
                result = {"error": f"Unknown action: {step.action}"}

            passed = self._evaluate_expected(result, step.expected)
            duration_ms = (time.time() - start_time) * 1000

            return StepResult(
                step=step,
                passed=passed,
                actual=result,
                duration_ms=duration_ms,
            )

        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            return StepResult(
                step=step,
                passed=False,
                error=str(e),
                duration_ms=duration_ms,
            )

    async def _http_request(
        self, method: str, url: str, context: Dict[str, Any]
    ) -> Dict[str, Any]:
        """Make an HTTP request."""
        try:
            import aiohttp

            async with aiohttp.ClientSession() as session:
                async with session.request(method, url) as response:
                    body = await response.text()
                    return {
                        "status": response.status,
                        "body": body[:1000],  # Limit body size
                        "headers": dict(response.headers),
                    }
        except ImportError:
            # Fallback to subprocess curl
            proc = await asyncio.create_subprocess_exec(
                "curl", "-s", "-w", "\n%{http_code}", "-X", method, url,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
            stdout, _ = await proc.communicate()
            output = stdout.decode()
            lines = output.rsplit("\n", 1)
            body = lines[0] if len(lines) > 1 else ""
            status = int(lines[-1]) if lines[-1].isdigit() else 0
            return {"status": status, "body": body[:1000]}

    async def _check_status(self, expected: str, context: Dict[str, Any]) -> Dict[str, Any]:
        """Check the last response status."""
        last_response = context.get("last_response", {})
        return {"status": last_response.get("status"), "expected": expected}

    async def _check_json(self, path: str, context: Dict[str, Any]) -> Dict[str, Any]:
        """Check a JSON path in the last response."""
        import json

        last_response = context.get("last_response", {})
        body = last_response.get("body", "{}")
        try:
            data = json.loads(body)
            # Simple path parsing (e.g., "data.items.0.name")
            parts = path.split(".")
            value = data
            for part in parts:
                if part.isdigit():
                    value = value[int(part)]
                else:
                    value = value.get(part)
            return {"value": value, "path": path}
        except (json.JSONDecodeError, KeyError, IndexError, TypeError) as e:
            return {"error": str(e), "path": path}


def create_validation_gate(
    config: "ValidationConfig", gate_id: Optional[str] = None
) -> Optional[ValidationGate]:
    """Factory function to create the appropriate validation gate.

    Args:
        config: Full validation configuration
        gate_id: Specific gate ID to create, or None for functional gate

    Returns:
        Appropriate ValidationGate instance or None
    """
    from .config import ValidationConfig

    if gate_id:
        gate_config = config.get_gate(gate_id)
    else:
        gate_config = config.get_functional_gate()

    if not gate_config:
        logger.warning(f"No gate found for id={gate_id}")
        return None

    gate_type = gate_config.type

    if gate_type == "web":
        return WebValidationGate(gate_config)
    elif gate_type == "ios":
        return iOSValidationGate(gate_config)
    elif gate_type == "cli":
        return CLIValidationGate(gate_config)
    elif gate_type == "api":
        return APIValidationGate(gate_config)
    else:
        logger.warning(f"Unknown gate type: {gate_type}")
        return None
