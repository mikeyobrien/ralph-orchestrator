# Iteration Loop Investigation - Ralph Orchestrator

**Date**: 2026-01-04 09:56
**Issue**: Orchestrator ran 100 iterations marking TASK_COMPLETE but functionality doesn't work
**File Analyzed**: `src/ralph_orchestrator/orchestrator.py`

---

## How the Iteration Loop Works

### Main Loop (Lines 361-473)
```python
while not self.stop_requested:
    # 1. Check safety limits (iterations, runtime, cost)
    # 2. Check for completion marker
    # 3. Determine trigger reason
    # 4. Execute iteration
    # 5. Record telemetry
    # 6. Sleep 2 seconds
```

**Key characteristics**:
- Loop continues UNTIL: `stop_requested=True` OR safety limit OR completion marker found
- Each iteration calls `_aexecute_iteration()` (line 394)
- Brief 2-second pause between iterations (line 473)

---

## What Triggers TASK_COMPLETE Detection

### Completion Check (Lines 374-377)
```python
if self._check_completion_marker():
    logger.info("Completion marker found - task marked complete")
    self.console.print_success("Task completion marker detected - stopping orchestration")
    break
```

**Checked BEFORE each iteration executes** - prevents unnecessary work after completion.

### `_check_completion_marker()` Method (Lines 818-900)

**Marker formats detected** (lines 844-877):
1. `- [x] TASK_COMPLETE` (checkbox, recommended)
2. `[x] TASK_COMPLETE` (checkbox without dash)
3. `**TASK_COMPLETE**` (bold markdown)
4. `TASK_COMPLETE` (standalone at line start)
5. `Status: TASK_COMPLETE` (colon format)

**Validation enforcement** (lines 882-894):
```python
if self.enable_validation:
    has_evidence, message = self._check_validation_evidence()
    if not has_evidence:
        logger.warning(f"TASK_COMPLETE marker found but validation evidence missing: {message}")
        self.console.print_warning(...)
        return False  # MARKER IGNORED
```

---

## Is Validation Evidence Actually Checked?

### Evidence Check Logic (Lines 781-816)

**When validation is ENABLED**:
```python
def _check_validation_evidence(self) -> tuple[bool, str]:
    if not self.enable_validation:
        return True, "Validation disabled, skipping evidence check"  # BYPASSED

    # Check validation-evidence/ directory
    evidence_dir = self.prompt_file.parent / "validation-evidence"

    # Count files: *.png, *.txt, *.json
    # Requires minimum 3 files
```

**When validation is DISABLED**:
- `_check_validation_evidence()` returns `(True, "Validation disabled, skipping evidence check")` immediately (line 793)
- **NO evidence checking happens**
- Completion marker alone is sufficient

**Default state** (line 98):
```python
self.enable_validation = getattr(config, 'enable_validation', False)  # DEFAULT: FALSE
```

---

## Why 100 Iterations with "Complete" Claims?

### Hypothesis: False Completion Loop

**Scenario**:
1. Agent writes `TASK_COMPLETE` marker to prompt file prematurely
2. Orchestrator checks marker BEFORE iteration (line 374)
3. **IF `enable_validation=False`**: Marker alone triggers stop
4. **BUT**: Agent may have marked complete WITHOUT actual work
5. If marker is subsequently REMOVED or modified by agent, loop continues

### Evidence from Code:

**Iteration execution** (Lines 488-556):
```python
async def _aexecute_iteration(self) -> bool:
    prompt = self.context_manager.get_prompt()  # Reads current prompt file

    # Execute agent
    response = await self.current_adapter.aexecute(prompt, ...)

    # Agent can MODIFY prompt file during execution
    # Next iteration reads UPDATED prompt
```

**No persistent completion state**:
- Marker is checked by re-reading prompt file each iteration
- If agent removes/modifies marker, orchestrator continues
- No internal flag prevents marker "un-setting"

### Why it reaches 100 iterations:

**Two possibilities**:

1. **Marker thrashing**: Agent repeatedly adds/removes marker
   - Iteration N: Marker present → "task complete" → iteration continues
   - Iteration N+1: Agent removes marker → loop continues
   - Repeats until max_iterations (default: 100)

2. **Marker never actually added**: Agent CLAIMS completion in output but doesn't write marker
   - Output contains "completed", "finished", "done" (line 553)
   - Task status updated to 'completed' (line 554)
   - BUT completion marker NOT written to prompt file
   - Loop continues to max_iterations

---

## Gap Between Iteration Claims and Actual Work

### Task Status Tracking (Lines 549-554)
```python
if response.success and self.current_task:
    output_lower = response.output.lower() if response.output else ""
    if any(word in output_lower for word in ['completed', 'finished', 'done', 'committed']):
        self._update_current_task('completed')  # Internal state only
```

**Critical gap**:
- Internal task status marked 'completed' based on keywords in output
- Does NOT require actual `TASK_COMPLETE` marker in prompt file
- Does NOT break the loop
- Creates false sense of completion

### Agent Communication Flow

**Iteration flow**:
```
1. Read prompt file → get current prompt
2. Execute agent with prompt
3. Agent outputs response (may claim "done")
4. Update task status if output contains completion keywords
5. Loop continues (marker check happens BEFORE next iteration)
```

**Disconnect**:
- Agent output → "I've completed the task"
- Task tracking → marks task 'completed'
- Orchestrator loop → continues because NO MARKER in prompt file
- Result: 100 iterations of "completed" claims with continued execution

---

## Summary

### Root Cause Identified

**Primary Issue**: Completion detection relies solely on prompt file marker check BEFORE each iteration. Agent can:
1. Claim completion in output (updates internal task status)
2. NOT write marker to prompt file
3. Continue receiving new iterations

**Validation Bypass**: When `enable_validation=False` (default), NO evidence checking occurs. Marker alone is sufficient, but marker may never be written.

**Loop Continuation**:
- If marker is never written: runs to max_iterations (100)
- If marker is added/removed repeatedly: thrashing until max_iterations
- Internal task status shows "completed" but orchestrator keeps running

### Evidence Gaps

**Not checked in current analysis**:
1. Are agents actually writing markers to prompt files?
2. Is marker format mismatch causing detection failure?
3. Are prompt file modifications being preserved between iterations?
4. What does the actual prompt file contain at iteration 100?

---

## Unresolved Questions

1. **Prompt file state**: What does PROMPT.md contain at iteration 100? Is marker present or absent?
2. **Agent behavior**: Are agents writing markers in unexpected formats not covered by detection regex?
3. **File persistence**: Are prompt file modifications being lost between iterations (file locking, race conditions)?
4. **Validation usage**: Is validation actually being enabled in production runs? Check CLI invocations.
5. **Task vs completion**: Why separate task completion tracking from loop termination? Design intent unclear.
