# OrchestrationManager Integration Gap Analysis

**Date:** 2026-01-05 02:41
**Subagent:** debugger (acbc667)
**Status:** Complete

---

## Executive Summary

**Gap Identified:** OrchestrationManager exists with spawn_subagent() but RalphOrchestrator does NOT use it.

**Impact:** enable_orchestration flag exists in config but has zero runtime effect. Orchestration feature is non-functional.

**Root Cause:** Missing integration in _aexecute_iteration() - no code checks enable_orchestration or calls OrchestrationManager.

---

## Analysis

### 1. Current State - orchestrator.py

**File:** `src/ralph_orchestrator/orchestrator.py`

**Adapter Used:** Line 146
```python
self.current_adapter = self.adapters.get(self.primary_tool)
```

Uses ClaudeAdapter, QChatAdapter, GeminiAdapter, or ACPAdapter depending on primary_tool.

**Iteration Method:** Line 590-661 `_aexecute_iteration()`
```python
async def _aexecute_iteration(self) -> bool:
    # ...
    prompt = self.context_manager.get_prompt()

    # MISSING: Check enable_orchestration here

    response = await self.current_adapter.aexecute(
        prompt,
        prompt_file=str(self.prompt_file),
        verbose=self.verbose
    )
    # ...
```

**No OrchestrationManager usage** - orchestrator.py never imports or calls OrchestrationManager.

### 2. Current State - OrchestrationManager

**File:** `src/ralph_orchestrator/orchestration/manager.py`

**spawn_subagent() Signature:** Line 242-326
```python
async def spawn_subagent(
    self,
    subagent_type: str,
    prompt: str,
    timeout: int = 300,
) -> Dict[str, Any]:
    """Spawn Claude subagent and collect results."""
```

**Returns:**
```python
{
    "subagent_type": str,
    "success": bool,
    "return_code": int,
    "stdout": str,
    "stderr": str,
    "parsed_json": Optional[dict],
    "error": Optional[str]
}
```

**Other Methods:**
- `generate_subagent_prompt()` - creates prompt for subagent
- `aggregate_results()` - collects results from coordination files

### 3. Config State - RalphConfig

**File:** `src/ralph_orchestrator/main.py`

**Line 274:**
```python
enable_orchestration: bool = False  # Enable subagent orchestration
```

Field exists, tests pass, but **NOT READ by orchestrator.py**.

---

## The Gap

### Integration Point Location

**File:** `src/ralph_orchestrator/orchestrator.py`
**Method:** `_aexecute_iteration()`
**Line:** Between 596-606

```python
async def _aexecute_iteration(self) -> bool:
    self._ensure_infrastructure()
    prompt = self.context_manager.get_prompt()

    # Extract tasks from prompt if needed
    if not self.task_queue and not self.current_task:
        self._extract_tasks_from_prompt(prompt)

    # Update current task status
    self._update_current_task('in_progress')

    # <-- INTEGRATION POINT HERE (line ~605)

    # Try primary adapter with prompt file path
    response = await self.current_adapter.aexecute(...)
```

### Missing Code

**What needs to be added:**

1. **Import OrchestrationManager** (top of orchestrator.py)
```python
from .orchestration import OrchestrationManager
```

2. **Check enable_orchestration flag** (in _aexecute_iteration)
```python
# Check if orchestration is enabled
if self.enable_orchestration:
    # Use OrchestrationManager instead of direct adapter execution
    return await self._execute_with_orchestration(prompt)
```

3. **New Method: _execute_with_orchestration()** (add to RalphOrchestrator class)
```python
async def _execute_with_orchestration(self, prompt: str) -> bool:
    """Execute iteration using OrchestrationManager for subagent spawning.

    Args:
        prompt: The prompt to execute

    Returns:
        bool: Success status
    """
    # Create OrchestrationManager if not exists
    if not hasattr(self, '_orchestration_manager'):
        self._orchestration_manager = OrchestrationManager(
            config=self  # Pass self if config-compatible
        )

    # Parse acceptance criteria from prompt
    criteria = self._parse_acceptance_criteria(prompt)

    # Generate subagent prompt
    subagent_prompt = self._orchestration_manager.generate_subagent_prompt(
        subagent_type="validator",
        phase=f"Iteration-{self.metrics.iterations}",
        criteria=criteria
    )

    # Spawn subagent
    result = await self._orchestration_manager.spawn_subagent(
        subagent_type="validator",
        prompt=subagent_prompt,
        timeout=300
    )

    # Store output
    if result["success"] and result["stdout"]:
        self.last_response_output = result["stdout"]

    return result["success"]
```

4. **Access enable_orchestration from config** (in __init__)
```python
# Line ~100 in __init__
self.enable_orchestration = getattr(config, 'enable_orchestration', False)
```

---

## Orchestration Flow (When Enabled)

```
┌─────────────────────────────────────────┐
│ RalphOrchestrator._aexecute_iteration() │
└──────────────┬──────────────────────────┘
               │
               ├─ Check enable_orchestration == True?
               │
               ├─ YES → Call _execute_with_orchestration()
               │         │
               │         ├─ Create/Get OrchestrationManager
               │         ├─ Generate subagent prompt
               │         ├─ Call manager.spawn_subagent()
               │         │   │
               │         │   └─ Spawns `claude -p <prompt>`
               │         │      subprocess
               │         │
               │         └─ Collect result/output
               │
               └─ NO  → Use current_adapter.aexecute()
                        (ClaudeAdapter, QChatAdapter, etc.)
```

---

## Required Changes Summary

| Location | Action | Lines |
|----------|--------|-------|
| orchestrator.py:26 | Add import | +1 |
| orchestrator.py:100 | Store enable_orchestration | +1 |
| orchestrator.py:605 | Check flag & route | +5 |
| orchestrator.py:END | Add _execute_with_orchestration() | +40 |
| orchestrator.py:END | Add _parse_acceptance_criteria() | +15 |

**Total:** ~62 new lines in orchestrator.py

---

## Unresolved Questions

1. How to extract acceptance criteria from prompt? (Need parser or YAML structure)
2. Should OrchestrationManager be instantiated once in __init__ or per-iteration?
3. What happens if subagent fails? (Fallback to normal adapter?)
4. How to handle spawn_subagent() errors? (Retry? Log and continue?)
5. Should enable_orchestration be runtime-toggleable or init-only?
