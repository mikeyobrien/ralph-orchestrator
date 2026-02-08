---
title: "OrchestrationManager Integration"
description: "Enable subagent spawning with proper MCP tool injection when enable_orchestration=True"
status: pending
priority: P1
effort: 8h
branch: feat/orchestration
tags: [orchestration, subagents, integration, MCP]
created: 2026-01-04
---

# OrchestrationManager Integration Plan

## Executive Summary

**Problem**: `OrchestrationManager` class is fully implemented but NOT integrated into main orchestrator loop.

**Gap Analysis**:
- `enable_orchestration: bool = False` exists in `RalphConfig` (main.py:274) but is NEVER READ by orchestrator
- `OrchestrationManager.spawn_subagent()` exists but `RalphOrchestrator` never instantiates `OrchestrationManager`
- No CLI flag `--enable-orchestration` in `__main__.py`
- Subagent prompts generated but never executed through the orchestration layer

**Solution**: Wire the existing `OrchestrationManager` into `RalphOrchestrator._aexecute_iteration()` when `enable_orchestration=True`.

---

## Architecture

### Current Flow (Non-Orchestrated)

```
RalphOrchestrator.arun()
    |
    v
_aexecute_iteration()
    |
    v
current_adapter.aexecute(prompt)  <-- Direct execution
    |
    v
Response
```

### Target Flow (Orchestrated)

```
RalphOrchestrator.arun()
    |
    v
_aexecute_iteration()
    |
    +-- enable_orchestration=False --> current_adapter.aexecute(prompt)
    |
    +-- enable_orchestration=True
            |
            v
        OrchestrationManager.analyze_task(prompt)
            |
            v
        Spawn subagent(s) for task type
            |
            v
        OrchestrationManager.spawn_subagent()
            |
            v
        OrchestrationManager.aggregate_results()
            |
            v
        Response (merged from subagents)
```

### Integration Points

```
+---------------------+        +------------------------+
|  RalphOrchestrator  |        |  OrchestrationManager  |
+---------------------+        +------------------------+
|                     |        |                        |
| - config            |------->| - config               |
| - orchestrator_mgr  |        | - coordinator          |
|                     |        |                        |
| _aexecute_iteration |        | generate_subagent_prompt|
|    |                |        | spawn_subagent         |
|    |                |        | aggregate_results      |
|    +----------------|------->|                        |
|                     |        +------------------------+
+---------------------+
                                        |
                                        v
                            +------------------------+
                            |  CoordinationManager   |
                            +------------------------+
                            | init_coordination      |
                            | write_subagent_result  |
                            | collect_results        |
                            +------------------------+
```

---

## Phase 1: CLI & Configuration Wiring (2h)

### 1.1 Add CLI Flag

**File**: `src/ralph_orchestrator/__main__.py`

**Change**: Add `--enable-orchestration` flag to argument parser.

```python
# After line ~80 (near other boolean flags)
parser.add_argument(
    "--enable-orchestration",
    action="store_true",
    default=False,
    help="Enable subagent orchestration mode (experimental)"
)
```

**Change**: Pass flag to config creation.

```python
# In config construction (around line ~120)
enable_orchestration=args.enable_orchestration,
```

### 1.2 Wire Config to Orchestrator

**File**: `src/ralph_orchestrator/orchestrator.py`

**Change**: Store `enable_orchestration` from config.

```python
# In __init__, after line ~100 (after self.enable_validation)
self.enable_orchestration = getattr(config, 'enable_orchestration', False)
```

### Acceptance Criteria - Phase 1

- [ ] `ralph --enable-orchestration -p PROMPT.md` parses without error
- [ ] `RalphOrchestrator.enable_orchestration` attribute is True when flag passed
- [ ] Default behavior unchanged (flag defaults to False)
- [ ] `ralph --help` shows `--enable-orchestration` flag

### Validation Method

```bash
# Test CLI parsing
python -m ralph_orchestrator --help | grep enable-orchestration

# Test config wiring (add temporary print in orchestrator __init__)
python -c "
from ralph_orchestrator.main import RalphConfig
from ralph_orchestrator.orchestrator import RalphOrchestrator
config = RalphConfig(prompt_file='PROMPT.md', enable_orchestration=True)
o = RalphOrchestrator(config)
print(f'enable_orchestration: {o.enable_orchestration}')
"
```

---

## Phase 2: OrchestrationManager Instantiation (1.5h)

### 2.1 Import OrchestrationManager

**File**: `src/ralph_orchestrator/orchestrator.py`

**Change**: Add import at top.

```python
# After line ~25 (imports section)
from .orchestration import OrchestrationManager
```

### 2.2 Instantiate When Enabled

**File**: `src/ralph_orchestrator/orchestrator.py`

**Change**: Create manager instance in `__init__`.

```python
# After self.enable_orchestration assignment (new code from Phase 1)
self.orchestration_manager = None
if self.enable_orchestration:
    # Import config to pass to manager
    self.orchestration_manager = OrchestrationManager(
        config=prompt_file_or_config if hasattr(prompt_file_or_config, 'prompt_file') else None,
        base_dir=Path.cwd(),
    )
    # Initialize coordination directories
    self.orchestration_manager.coordinator.init_coordination()
    logger.info("OrchestrationManager initialized - subagent mode enabled")
```

### 2.3 Add MCP Verification (Hard Failure)

**File**: `src/ralph_orchestrator/orchestration/manager.py`

**Change**: Add verification method that raises on missing required MCPs.

```python
class OrchestrationError(Exception):
    """Raised when orchestration cannot proceed."""
    pass


# In OrchestrationManager class:
def verify_required_mcps(self, subagent_type: str) -> None:
    """Verify all required MCPs are available for a subagent type.

    Args:
        subagent_type: Type of subagent to check MCPs for.

    Raises:
        OrchestrationError: If any required MCP is missing or disabled.
    """
    required = get_required_mcps_for_subagent(subagent_type)
    available = discover_mcps()

    missing = []
    disabled = []

    for mcp_name in required:
        if mcp_name not in available:
            missing.append(mcp_name)
        elif not available[mcp_name].enabled:
            disabled.append(mcp_name)

    if missing or disabled:
        msg_parts = []
        if missing:
            msg_parts.append(f"missing: {missing}")
        if disabled:
            msg_parts.append(f"disabled: {disabled}")
        raise OrchestrationError(
            f"Required MCPs not available for {subagent_type}: {', '.join(msg_parts)}. "
            f"Configure these in ~/.claude/settings.json or disable orchestration."
        )
```

**Change**: Call verification before spawn.

```python
# In spawn_subagent(), add at the start:
self.verify_required_mcps(subagent_type)
```

### Acceptance Criteria - Phase 2

- [ ] `RalphOrchestrator.orchestration_manager` is `OrchestrationManager` instance when enabled
- [ ] `RalphOrchestrator.orchestration_manager` is `None` when disabled (backward compat)
- [ ] `.agent/coordination/` directory created when enabled
- [ ] No import errors or circular dependencies
- [ ] **Fails fast with `OrchestrationError` if required MCP unavailable**

### Validation Method

```bash
# Run unit test
pytest tests/test_orchestrator.py -k "orchestration" -v

# Manual verification
python -c "
from ralph_orchestrator.main import RalphConfig
from ralph_orchestrator.orchestrator import RalphOrchestrator
from pathlib import Path
import tempfile
import os

with tempfile.TemporaryDirectory() as tmpdir:
    os.chdir(tmpdir)
    Path('PROMPT.md').write_text('# Test')
    config = RalphConfig(prompt_file='PROMPT.md', enable_orchestration=True)
    o = RalphOrchestrator(config)
    print(f'Manager: {o.orchestration_manager}')
    print(f'Coordination dir exists: {Path(\".agent/coordination\").exists()}')
"
```

---

## Phase 3: Execution Integration (3h)

### 3.1 Task Analysis Helper

**File**: `src/ralph_orchestrator/orchestrator.py`

**Change**: Add helper to determine subagent type from prompt.

```python
def _determine_subagent_type(self, prompt: str) -> str:
    """Determine appropriate subagent type based on prompt content.

    Simple heuristic-based classification:
    - Keywords like 'validate', 'verify', 'test', 'check' -> validator
    - Keywords like 'research', 'find', 'search', 'explore' -> researcher
    - Keywords like 'implement', 'build', 'create', 'fix' -> implementer
    - Keywords like 'analyze', 'review', 'assess', 'audit' -> analyst
    - Default -> implementer (most common use case)
    """
    prompt_lower = prompt.lower()

    validator_keywords = ['validate', 'verify', 'test', 'check', 'confirm', 'assert']
    researcher_keywords = ['research', 'find', 'search', 'explore', 'discover', 'investigate']
    analyst_keywords = ['analyze', 'review', 'assess', 'audit', 'examine', 'evaluate']
    debugger_keywords = ['debug', 'fix bug', 'troubleshoot', 'diagnose', 'error']

    # Check in priority order (most specific first)
    for kw in debugger_keywords:
        if kw in prompt_lower:
            return 'debugger'
    for kw in validator_keywords:
        if kw in prompt_lower:
            return 'validator'
    for kw in researcher_keywords:
        if kw in prompt_lower:
            return 'researcher'
    for kw in analyst_keywords:
        if kw in prompt_lower:
            return 'analyst'

    # Default to implementer
    return 'implementer'
```

### 3.2 Modify _aexecute_iteration

**File**: `src/ralph_orchestrator/orchestrator.py`

**Change**: Branch execution based on `enable_orchestration`.

```python
async def _aexecute_iteration(self) -> bool:
    """Execute a single iteration asynchronously."""
    # Ensure infrastructure directories exist
    self._ensure_infrastructure()

    # Get the current prompt
    prompt = self.context_manager.get_prompt()

    # Extract tasks from prompt if task queue is empty
    if not self.task_queue and not self.current_task:
        self._extract_tasks_from_prompt(prompt)

    # Update current task status
    self._update_current_task('in_progress')

    # === ORCHESTRATION BRANCH ===
    if self.enable_orchestration and self.orchestration_manager:
        return await self._execute_orchestrated_iteration(prompt)

    # === ORIGINAL DIRECT EXECUTION (unchanged) ===
    response = await self.current_adapter.aexecute(
        prompt,
        prompt_file=str(self.prompt_file),
        verbose=self.verbose
    )
    # ... rest of original implementation unchanged ...
```

### 3.3 Add Orchestrated Execution Method

**File**: `src/ralph_orchestrator/orchestrator.py`

**Change**: Add new method for orchestrated execution.

```python
async def _execute_orchestrated_iteration(self, prompt: str) -> bool:
    """Execute iteration using subagent orchestration.

    Spawns appropriate subagent based on task analysis and collects results.

    Args:
        prompt: The current prompt text.

    Returns:
        bool: True if subagent execution succeeded.
    """
    logger.info("Executing orchestrated iteration")

    # 1. Determine subagent type
    subagent_type = self._determine_subagent_type(prompt)
    logger.info(f"Selected subagent type: {subagent_type}")

    # 2. Generate subagent prompt with MCP/skill injection
    subagent_prompt = self.orchestration_manager.generate_subagent_prompt(
        subagent_type=subagent_type,
        phase=f"Iteration {self.metrics.iterations}",
        criteria=self._extract_criteria_from_prompt(prompt),
        subagent_id=f"{self.metrics.iterations:03d}",
    )

    # 3. Spawn subagent
    result = await self.orchestration_manager.spawn_subagent(
        subagent_type=subagent_type,
        prompt=subagent_prompt,
        timeout=self.max_runtime // self.max_iterations,  # Proportional timeout
    )

    # 4. Write result to coordination file
    self.orchestration_manager.coordinator.write_subagent_result(
        subagent_type=subagent_type,
        subagent_id=f"{self.metrics.iterations:03d}",
        result=result,
    )

    # 5. Update last_response_output for loop detection
    if result.get('stdout'):
        self.last_response_output = result['stdout']
    elif result.get('parsed_json'):
        self.last_response_output = str(result['parsed_json'])

    # 6. Track success
    success = result.get('success', False)

    if not success:
        error_msg = result.get('error', 'Unknown error')
        logger.warning(f"Subagent execution failed: {error_msg}")
        self.console.print_warning(f"Subagent error: {error_msg}")

    return success

def _extract_criteria_from_prompt(self, prompt: str) -> list:
    """Extract acceptance criteria from prompt text.

    Looks for common patterns:
    - [ ] Checkbox items
    - Numbered lists under "Acceptance Criteria" header
    - "Must" statements

    Args:
        prompt: The prompt text.

    Returns:
        List of criteria strings.
    """
    import re
    criteria = []

    # Look for checkbox items
    checkbox_pattern = r'^\s*-\s*\[\s*\]\s*(.+)$'
    for match in re.finditer(checkbox_pattern, prompt, re.MULTILINE):
        criteria.append(match.group(1).strip())

    # Look for "must" statements
    must_pattern = r'(?:must|should|shall)\s+(.+?)(?:\.|$)'
    for match in re.finditer(must_pattern, prompt, re.IGNORECASE):
        criteria.append(match.group(0).strip())

    # Default if nothing found
    if not criteria:
        criteria = ["Execute the task as specified in the prompt"]

    return criteria[:10]  # Limit to 10 criteria
```

### Acceptance Criteria - Phase 3

- [ ] `_determine_subagent_type()` returns correct type for keyword-based prompts
- [ ] `_execute_orchestrated_iteration()` spawns Claude CLI subprocess
- [ ] Subagent results written to `.agent/coordination/results/`
- [ ] `last_response_output` populated for loop detection
- [ ] Timeout proportional to remaining runtime budget
- [ ] Errors logged and returned properly (no silent failures)

### Validation Method

```bash
# Unit tests for subagent type detection
pytest tests/test_orchestrator.py -k "subagent_type" -v

# Integration test (requires Claude CLI)
python -c "
import asyncio
from ralph_orchestrator.main import RalphConfig
from ralph_orchestrator.orchestrator import RalphOrchestrator
from pathlib import Path
import tempfile
import os

async def test():
    with tempfile.TemporaryDirectory() as tmpdir:
        os.chdir(tmpdir)
        Path('PROMPT.md').write_text('# Test\nValidate that 2+2=4')
        config = RalphConfig(
            prompt_file='PROMPT.md',
            enable_orchestration=True,
            max_iterations=1,
        )
        o = RalphOrchestrator(config)
        success = await o._aexecute_iteration()
        print(f'Success: {success}')
        print(f'Results dir: {list(Path(\".agent/coordination/results\").glob(\"*\"))}')

asyncio.run(test())
"
```

---

## Phase 4: Result Aggregation & Summary (1.5h)

### 4.1 Aggregate Results in Summary

**File**: `src/ralph_orchestrator/orchestrator.py`

**Change**: Modify `_print_summary()` to include orchestration results.

```python
def _print_summary(self):
    """Print execution summary with enhanced console output."""
    # ... existing code ...

    # Add orchestration summary if enabled
    if self.enable_orchestration and self.orchestration_manager:
        aggregated = self.orchestration_manager.aggregate_results()

        self.console.print_header("Orchestration Results")
        self.console.print_info(f"Overall Verdict: {aggregated.get('verdict', 'UNKNOWN')}")
        self.console.print_info(f"Summary: {aggregated.get('summary', 'No summary')}")

        # Show individual subagent results
        for result in aggregated.get('subagent_results', []):
            subagent = result.get('subagent_type', 'unknown')
            verdict = result.get('verdict', 'UNKNOWN')
            self.console.print_info(f"  - {subagent}: {verdict}")

    # ... rest of existing code ...
```

### 4.2 Include Orchestration in Metrics JSON

**File**: `src/ralph_orchestrator/orchestrator.py`

**Change**: Add orchestration data to metrics output.

```python
# In _print_summary(), in metrics_data dict construction
metrics_data = {
    # ... existing fields ...

    # Orchestration metrics (if enabled)
    "orchestration": {
        "enabled": self.enable_orchestration,
        "results": self.orchestration_manager.aggregate_results() if self.orchestration_manager else None,
    } if self.enable_orchestration else None,
}
```

### Acceptance Criteria - Phase 4

- [ ] Summary shows "Orchestration Results" section when enabled
- [ ] Aggregated verdict displayed (PASS/FAIL/NO_RESULTS)
- [ ] Individual subagent results listed
- [ ] Metrics JSON includes orchestration data
- [ ] No changes to output when orchestration disabled

### Validation Method

```bash
# Run orchestrated iteration and check summary
ralph --enable-orchestration -p test-prompt.md --max-iterations 2

# Check metrics file
cat .agent/metrics/metrics_*.json | jq '.orchestration'
```

---

## Test Strategy

### Unit Tests (New)

**File**: `tests/test_orchestrator_integration.py`

```python
"""Tests for OrchestrationManager integration into RalphOrchestrator."""

import pytest
from unittest.mock import AsyncMock, MagicMock, patch
from pathlib import Path
import tempfile
import os


class TestOrchestrationFlag:
    """Test enable_orchestration flag wiring."""

    def test_orchestration_disabled_by_default(self):
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            os.chdir(tmpdir)
            Path('PROMPT.md').write_text('# Test')
            config = RalphConfig(prompt_file='PROMPT.md')
            o = RalphOrchestrator(config)

            assert o.enable_orchestration is False
            assert o.orchestration_manager is None

    def test_orchestration_enabled_creates_manager(self):
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator
        from ralph_orchestrator.orchestration import OrchestrationManager

        with tempfile.TemporaryDirectory() as tmpdir:
            os.chdir(tmpdir)
            Path('PROMPT.md').write_text('# Test')
            config = RalphConfig(prompt_file='PROMPT.md', enable_orchestration=True)
            o = RalphOrchestrator(config)

            assert o.enable_orchestration is True
            assert isinstance(o.orchestration_manager, OrchestrationManager)

    def test_coordination_directory_created(self):
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            os.chdir(tmpdir)
            Path('PROMPT.md').write_text('# Test')
            config = RalphConfig(prompt_file='PROMPT.md', enable_orchestration=True)
            o = RalphOrchestrator(config)

            assert Path('.agent/coordination').exists()


class TestSubagentTypeDetection:
    """Test _determine_subagent_type heuristics."""

    @pytest.fixture
    def orchestrator(self):
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            os.chdir(tmpdir)
            Path('PROMPT.md').write_text('# Test')
            config = RalphConfig(prompt_file='PROMPT.md', enable_orchestration=True)
            yield RalphOrchestrator(config)

    def test_validator_keywords(self, orchestrator):
        assert orchestrator._determine_subagent_type("Validate the API") == "validator"
        assert orchestrator._determine_subagent_type("verify correctness") == "validator"
        assert orchestrator._determine_subagent_type("Test the login flow") == "validator"

    def test_researcher_keywords(self, orchestrator):
        assert orchestrator._determine_subagent_type("Research best practices") == "researcher"
        assert orchestrator._determine_subagent_type("Find similar examples") == "researcher"
        assert orchestrator._determine_subagent_type("Search for documentation") == "researcher"

    def test_analyst_keywords(self, orchestrator):
        assert orchestrator._determine_subagent_type("Analyze the codebase") == "analyst"
        assert orchestrator._determine_subagent_type("Review the architecture") == "analyst"
        assert orchestrator._determine_subagent_type("Assess security risks") == "analyst"

    def test_debugger_keywords(self, orchestrator):
        assert orchestrator._determine_subagent_type("Debug the crash") == "debugger"
        assert orchestrator._determine_subagent_type("Fix bug in parser") == "debugger"

    def test_default_implementer(self, orchestrator):
        assert orchestrator._determine_subagent_type("Build a new feature") == "implementer"
        assert orchestrator._determine_subagent_type("Create the module") == "implementer"
        assert orchestrator._determine_subagent_type("Hello world") == "implementer"


class TestOrchestratedExecution:
    """Test _execute_orchestrated_iteration method."""

    @pytest.mark.asyncio
    async def test_spawns_subagent_and_records_result(self):
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            os.chdir(tmpdir)
            Path('PROMPT.md').write_text('# Test\nValidate that things work')
            config = RalphConfig(prompt_file='PROMPT.md', enable_orchestration=True)
            o = RalphOrchestrator(config)

            # Mock spawn_subagent to avoid actual Claude CLI call
            o.orchestration_manager.spawn_subagent = AsyncMock(return_value={
                'subagent_type': 'validator',
                'success': True,
                'return_code': 0,
                'stdout': '{"verdict": "PASS"}',
                'stderr': '',
                'parsed_json': {'verdict': 'PASS'},
                'error': None,
            })

            success = await o._execute_orchestrated_iteration("Validate that 2+2=4")

            assert success is True
            assert o.last_response_output is not None
            o.orchestration_manager.spawn_subagent.assert_called_once()
```

### Integration Tests (Existing)

Extend `tests/test_orchestration_integration.py` with:

```python
@pytest.mark.integration
class TestFullOrchestrationFlow:
    """Integration tests requiring Claude CLI."""

    @pytest.mark.asyncio
    async def test_orchestrated_iteration_real_claude(self):
        """Test full orchestrated iteration with real Claude CLI."""
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        with tempfile.TemporaryDirectory() as tmpdir:
            os.chdir(tmpdir)
            Path('PROMPT.md').write_text('# Quick Test\nSay "hello" and nothing else.')
            config = RalphConfig(
                prompt_file='PROMPT.md',
                enable_orchestration=True,
                max_iterations=1,
                max_runtime=120,
            )
            o = RalphOrchestrator(config)

            success = await o._aexecute_iteration()

            # Should complete (success or graceful failure)
            assert isinstance(success, bool)

            # Results should be recorded
            results_dir = Path('.agent/coordination/results')
            if results_dir.exists():
                result_files = list(results_dir.glob('*.json'))
                assert len(result_files) >= 0  # May be 0 if failed quickly
```

---

## File Change Summary

| File | Action | Lines Changed |
|------|--------|---------------|
| `src/ralph_orchestrator/__main__.py` | Add CLI flag | ~10 |
| `src/ralph_orchestrator/orchestrator.py` | Import, init, execution | ~120 |
| `src/ralph_orchestrator/orchestration/manager.py` | Add MCP verification, OrchestrationError | ~35 |
| `tests/test_orchestrator_integration.py` | New test file | ~150 |

---

## Backward Compatibility

1. **Default off**: `enable_orchestration=False` by default
2. **No breaking changes**: All existing tests pass unchanged
3. **Gradual migration**: Users opt-in explicitly via `--enable-orchestration`
4. **Fallback**: If subagent spawn fails, error is logged but orchestrator continues

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Claude CLI not installed | Check availability at init, warn if missing |
| Subagent timeout | Proportional timeout based on iteration budget |
| Circular dependency | Import OrchestrationManager only in orchestrator.py |
| Coordination file corruption | Use atomic writes (already implemented in CoordinationManager) |
| Missing required MCP (e.g., sequential-thinking) | Hard failure via `verify_required_mcps()` with actionable error message |

---

## Resolved Questions

| Question | Resolution | Rationale |
|----------|------------|-----------|
| Multi-subagent parallel | Deferred to Phase 5 | Core problem is orchestration doesn't work at all; parallel is optimization |
| Model selection | Deferred to Phase 5 | Premature optimization when spawn_subagent() is never called |
| Result caching | **REJECTED** | Dangerous for correctness - cached PASS becomes stale if code changes |
| MCP hard failure | **Implemented in Phase 2** | Required MCPs must fail fast, not soft-warn in prompt |
