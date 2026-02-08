---
title: "Wire OrchestrationManager into RalphOrchestrator"
description: "Integrate OrchestrationManager.spawn_subagent() with RalphOrchestrator to make enable_orchestration flag functional"
status: pending
priority: P1
effort: 2h
branch: feat/orchestration
tags: [orchestration, integration, bugfix]
created: 2026-01-05
---

# Implementation Plan: Wire OrchestrationManager into RalphOrchestrator

## Problem Statement

**Root Cause:** `OrchestrationManager` exists in `orchestration/manager.py` with `spawn_subagent()` but `RalphOrchestrator` in `orchestrator.py` NEVER imports or uses it. The `enable_orchestration` config flag has zero runtime effect.

**Evidence:** Log file `.agent/logs/ralph_20260105_004944.log` shows code WAS running:
```
2026-01-05 00:52:47,140 - Executing orchestrated iteration
2026-01-05 00:52:47,140 - Selected subagent type: debugger
2026-01-05 00:54:08,218 - Subagent execution failed: Timeout after 81 seconds
```

But this code is MISSING from current source - never committed or got lost.

## Solution Overview

Add orchestration routing in `_aexecute_iteration()`:
- When `enable_orchestration=True`: Use `OrchestrationManager.spawn_subagent()`
- When `enable_orchestration=False`: Continue using `adapter.aexecute()` (current behavior)

---

## Step 1: Add Import and Store Config Flag

**File:** `src/ralph_orchestrator/orchestrator.py`

**Location:** Line ~16-26 (imports section)

**Add import:**
```python
from .orchestration import OrchestrationManager
```

**Location:** Line ~100 (in `__init__`, after line 99 `self.validation_interactive = ...`)

**Store flag from config:**
```python
self.enable_orchestration = getattr(config, 'enable_orchestration', False)
```

---

## Step 2: Initialize OrchestrationManager

**File:** `src/ralph_orchestrator/orchestrator.py`

**Location:** Line ~144 (after `self.adapters = self._initialize_adapters()`)

**Add manager initialization:**
```python
# Initialize OrchestrationManager if orchestration enabled
self._orchestration_manager = None
if self.enable_orchestration:
    # Need to pass config object to OrchestrationManager
    if hasattr(prompt_file_or_config, 'prompt_file'):
        self._orchestration_manager = OrchestrationManager(prompt_file_or_config)
        logger.info("OrchestrationManager initialized for subagent spawning")
```

---

## Step 3: Add Orchestration Routing in _aexecute_iteration()

**File:** `src/ralph_orchestrator/orchestrator.py`

**Location:** Line ~590-610 (in `_aexecute_iteration()`, BEFORE the existing adapter.aexecute call)

**Replace the execution logic:**
```python
async def _aexecute_iteration(self) -> bool:
    """Execute a single iteration asynchronously."""
    # Ensure infrastructure directories exist (defensive check)
    self._ensure_infrastructure()

    # Get the current prompt
    prompt = self.context_manager.get_prompt()

    # Extract tasks from prompt if task queue is empty
    if not self.task_queue and not self.current_task:
        self._extract_tasks_from_prompt(prompt)

    # Update current task status
    self._update_current_task('in_progress')

    # Route to orchestration or direct adapter based on config
    if self.enable_orchestration and self._orchestration_manager:
        response = await self._execute_with_orchestration(prompt)
    else:
        # Original path: direct adapter execution
        response = await self.current_adapter.aexecute(
            prompt,
            prompt_file=str(self.prompt_file),
            verbose=self.verbose
        )

    # ... rest of existing code unchanged ...
```

---

## Step 4: Add _execute_with_orchestration() Method

**File:** `src/ralph_orchestrator/orchestrator.py`

**Location:** After `_aexecute_iteration()` method (line ~661)

**Add new method:**
```python
async def _execute_with_orchestration(self, prompt: str):
    """Execute iteration using OrchestrationManager subagent spawning.

    Routes to appropriate subagent type based on prompt/task analysis.
    Uses 300-second timeout (not 81) for adequate agent completion.

    Args:
        prompt: The current prompt text

    Returns:
        Response object compatible with adapter response format
    """
    from dataclasses import dataclass

    @dataclass
    class OrchestrationResponse:
        success: bool
        output: str
        tokens_used: int = 0

    logger.info("Executing orchestrated iteration")

    # Determine subagent type based on prompt content
    subagent_type = self._select_subagent_type(prompt)
    logger.info(f"Selected subagent type: {subagent_type}")

    # Generate subagent prompt with skill/MCP instructions
    subagent_prompt = self._orchestration_manager.generate_subagent_prompt(
        subagent_type=subagent_type,
        phase=f"Iteration {self.metrics.iterations}",
        criteria=[prompt[:500]],  # Use prompt excerpt as criteria
    )

    # Spawn subagent with proper timeout (300s, not 81s)
    result = await self._orchestration_manager.spawn_subagent(
        subagent_type=subagent_type,
        prompt=subagent_prompt,
        timeout=300,  # 5 minutes - adequate for Claude execution
    )

    # Convert result to response format
    if result["success"]:
        output = result.get("stdout", "")
        if result.get("parsed_json"):
            output = json.dumps(result["parsed_json"], indent=2)
        self.last_response_output = output
        return OrchestrationResponse(success=True, output=output)
    else:
        error_msg = result.get("error", "Unknown subagent error")
        logger.warning(f"Subagent execution failed: {error_msg}")
        return OrchestrationResponse(success=False, output=error_msg)

def _select_subagent_type(self, prompt: str) -> str:
    """Select appropriate subagent type based on prompt content.

    Simple heuristic based on keywords in prompt:
    - "test", "validate", "verify" -> validator
    - "research", "find", "search" -> researcher
    - "implement", "fix", "build", "add" -> implementer
    - "debug", "analyze", "investigate" -> analyst

    Args:
        prompt: Current prompt text

    Returns:
        Subagent type string (validator, researcher, implementer, analyst)
    """
    prompt_lower = prompt.lower()

    # Priority order matters - check more specific patterns first
    if any(kw in prompt_lower for kw in ['debug', 'analyze', 'investigate', 'root cause']):
        return 'analyst'
    if any(kw in prompt_lower for kw in ['test', 'validate', 'verify', 'check']):
        return 'validator'
    if any(kw in prompt_lower for kw in ['research', 'find', 'search', 'look up']):
        return 'researcher'

    # Default to implementer for most tasks
    return 'implementer'
```

---

## Step 5: Add Missing Import for json

**File:** `src/ralph_orchestrator/orchestrator.py`

**Verification:** Line 13 already has `import json` - no change needed.

---

## Test Strategy

### Unit Tests (Add to tests/test_orchestration_integration.py)

```python
class TestOrchestrationManagerWiring:
    """Test RalphOrchestrator uses OrchestrationManager when enabled."""

    def test_orchestrator_stores_enable_orchestration(self):
        """RalphOrchestrator should store enable_orchestration from config."""
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        config = RalphConfig(enable_orchestration=True)
        orch = RalphOrchestrator(config)

        assert hasattr(orch, 'enable_orchestration')
        assert orch.enable_orchestration is True

    def test_orchestrator_initializes_manager_when_enabled(self):
        """RalphOrchestrator should init OrchestrationManager when enabled."""
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator
        from ralph_orchestrator.orchestration import OrchestrationManager

        config = RalphConfig(enable_orchestration=True)
        orch = RalphOrchestrator(config)

        assert hasattr(orch, '_orchestration_manager')
        assert isinstance(orch._orchestration_manager, OrchestrationManager)

    def test_orchestrator_no_manager_when_disabled(self):
        """RalphOrchestrator should NOT init OrchestrationManager when disabled."""
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        config = RalphConfig(enable_orchestration=False)
        orch = RalphOrchestrator(config)

        assert orch._orchestration_manager is None

    def test_select_subagent_type_implementer(self):
        """_select_subagent_type should return implementer for implementation tasks."""
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        config = RalphConfig(enable_orchestration=True)
        orch = RalphOrchestrator(config)

        assert orch._select_subagent_type("implement the login feature") == "implementer"
        assert orch._select_subagent_type("fix the bug in auth") == "implementer"

    def test_select_subagent_type_debugger(self):
        """_select_subagent_type should return analyst for debug tasks."""
        from ralph_orchestrator.main import RalphConfig
        from ralph_orchestrator.orchestrator import RalphOrchestrator

        config = RalphConfig(enable_orchestration=True)
        orch = RalphOrchestrator(config)

        assert orch._select_subagent_type("debug why tests fail") == "analyst"
        assert orch._select_subagent_type("investigate the root cause") == "analyst"
```

### Integration Test (Manual)

1. Create test prompt file:
```bash
echo "Test prompt for orchestration integration" > /tmp/test-orch.md
```

2. Run with orchestration enabled:
```bash
ralph --prompt-file /tmp/test-orch.md --enable-orchestration --max-iterations 1 --verbose
```

3. Verify logs show:
   - "OrchestrationManager initialized for subagent spawning"
   - "Executing orchestrated iteration"
   - "Selected subagent type: implementer"
   - No 81-second timeout (should be 300s or completion)

---

## Validation Criteria

1. **Config flag wired:** `RalphOrchestrator.__init__` stores `enable_orchestration`
2. **Manager initialized:** `_orchestration_manager` is `OrchestrationManager` when enabled, `None` when disabled
3. **Routing works:** `_aexecute_iteration()` calls `_execute_with_orchestration()` when flag is True
4. **Timeout fixed:** `spawn_subagent()` called with `timeout=300` (not 81)
5. **Logging present:** Log messages match evidence from working version
6. **Tests pass:** All existing tests + new wiring tests pass
7. **Fallback works:** When `enable_orchestration=False`, original adapter path unchanged

---

## Files Modified Summary

| File | Changes |
|------|---------|
| `src/ralph_orchestrator/orchestrator.py` | Add import, store flag, init manager, add routing, add methods |
| `tests/test_orchestration_integration.py` | Add wiring tests |

---

## Unresolved Questions

1. **CLI flag:** Should `--enable-orchestration` be added to CLI args in `main.py`? Currently only available via config file/code.
2. **Subagent selection:** The keyword-based heuristic is simple. Should it be more sophisticated (e.g., ML-based or configurable)?
3. **Error recovery:** When subagent fails, should we fall back to direct adapter execution or retry?
