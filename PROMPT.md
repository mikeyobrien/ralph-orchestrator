# Feature: Per-Iteration Telemetry for Ralph Orchestrator

## Objective

Implement per-iteration telemetry capture in ralph-orchestrator so that each iteration's details (trigger reason, duration, cost, output preview) are persisted to the metrics JSON output.

**PDR Reference:** Read `../../docs/PDR-ralph-iteration-telemetry.md` for full design.

## Completion Criteria

All must be checked for task completion:
- [x] `TriggerReason` enum added to `metrics.py`
- [x] `IterationStats.record_iteration()` extended with new fields
- [ ] `orchestrator.py` uses `IterationStats` alongside `Metrics`
- [ ] `_determine_trigger_reason()` method added to orchestrator
- [ ] `arun()` loop records per-iteration details
- [ ] `_print_summary()` saves full iteration list to JSON
- [ ] Unit tests added for new functionality
- [ ] All existing tests pass
- [ ] TASK_COMPLETE

## Constraints

- Do NOT break backward compatibility - keep `summary` section in output
- Do NOT change the orchestration loop logic itself
- Do NOT merge - leave PR/MR for human review
- Limit output preview to 500 characters (privacy)
- Memory limit of 1000 iterations already exists - respect it

## Repository

**Target:** ralph-orchestrator (should already be cloned to working directory)
**Branch:** Create `feature/per-iteration-telemetry`

## Implementation Phases

### Phase 1: Metrics Enhancement (metrics.py)

1. **Add TriggerReason enum** after line 12:
```python
from enum import Enum

class TriggerReason(str, Enum):
    """Reasons why an iteration was triggered."""
    INITIAL = "initial"
    TASK_INCOMPLETE = "task_incomplete"
    PREVIOUS_SUCCESS = "previous_success"
    RECOVERY = "recovery"
    LOOP_DETECTED = "loop_detected"
    SAFETY_LIMIT = "safety_limit"
    USER_STOP = "user_stop"
```

2. **Extend `IterationStats.record_iteration()`** to accept new parameters:
   - `trigger_reason: str = ""`
   - `output_preview: str = ""`
   - `tokens_used: int = 0`
   - `cost: float = 0.0`
   - `tools_used: List[str] = None`

3. **Add unit tests** in `tests/test_metrics.py`:
   - Test TriggerReason enum values
   - Test record_iteration with new fields
   - Test output preview truncation

### Phase 2: Orchestrator Integration (orchestrator.py)

1. **Add `self.iteration_stats = IterationStats()`** at line ~96 alongside existing Metrics

2. **Add `_determine_trigger_reason()` method**:
```python
def _determine_trigger_reason(self) -> str:
    """Determine why this iteration is being triggered."""
    from .metrics import TriggerReason

    if self.metrics.iterations == 0:
        return TriggerReason.INITIAL.value

    if self.metrics.failed_iterations > 0 and \
       self.metrics.failed_iterations >= self.metrics.iterations - 1:
        return TriggerReason.RECOVERY.value

    return TriggerReason.TASK_INCOMPLETE.value
```

3. **Modify `arun()` loop** (around line 316) to:
   - Capture `trigger_reason` before iteration
   - Time the iteration with `iteration_start = time.time()`
   - Extract cost/tokens from `self.cost_tracker.usage_history[-1]` if available
   - Call `self.iteration_stats.record_iteration()` with all details

4. **Modify `_print_summary()`** (around line 577) to save enhanced JSON:
```python
metrics_data = {
    "summary": {
        "iterations": self.metrics.iterations,
        "successful": self.metrics.successful_iterations,
        "failed": self.metrics.failed_iterations,
        "errors": self.metrics.errors,
        "checkpoints": self.metrics.checkpoints,
        "rollbacks": self.metrics.rollbacks,
    },
    "iterations": self.iteration_stats.iterations,
    "cost": {
        "total": self.cost_tracker.total_cost if self.cost_tracker else 0,
        "by_tool": self.cost_tracker.costs_by_tool if self.cost_tracker else {},
        "history": self.cost_tracker.usage_history if self.cost_tracker else [],
    },
    "analysis": {
        "avg_iteration_duration": self.iteration_stats.get_average_duration(),
        "success_rate": self.iteration_stats.get_success_rate(),
    }
}
```

### Phase 3: Testing & Validation

1. **Run existing tests**: `pytest tests/`
2. **Add integration test** for telemetry capture
3. **Manual test**: Run ralph with a simple prompt and verify JSON output

## Progress (Scratchpad)

### Iteration Log
| # | Status | Action | Result |
|---|--------|--------|--------|
| 1 | ✅ DONE | Add TriggerReason enum to metrics.py | Committed 8dec3ef |
| 2 | ✅ DONE | Extend record_iteration() with telemetry fields | Committed 20005a0 |

### Current State
- **Phase:** Phase 1 - Metrics Enhancement (Step 2 of 3 complete)
- **Blockers:** None
- **Last Action:** Extended IterationStats.record_iteration() with new fields: trigger_reason, output_preview, tokens_used, cost, tools_used. Added 500-char truncation for output_preview. All 34 existing tests pass.

### What Remains
**Phase 1 (metrics.py):**
- [x] Step 1: Add TriggerReason enum (DONE)
- [x] Step 2: Extend `IterationStats.record_iteration()` with new fields (DONE)
- [ ] Step 3: Add unit tests for TriggerReason and new fields

**Phase 2 (orchestrator.py):**
- [ ] Add `self.iteration_stats = IterationStats()` alongside Metrics
- [ ] Add `_determine_trigger_reason()` method
- [ ] Modify `arun()` loop to record per-iteration details
- [ ] Modify `_print_summary()` to save enhanced JSON

**Phase 3 (Testing):**
- [ ] Run existing tests
- [ ] Add integration test
- [ ] Manual validation

---

## IMPORTANT: Iteration Behavior

You are running in Ralph Orchestrator loop. Each iteration:
1. Read the scratchpad to understand what was done before
2. Do ONE focused task (not everything at once)
3. Update the scratchpad with what you accomplished
4. Update the iteration log
5. Signal progress or completion

DO NOT restart from scratch if scratchpad shows progress.
CONTINUE from where the previous iteration left off.

## Test Commands

```bash
# Run unit tests
pytest tests/test_metrics.py -v

# Run all tests
pytest tests/ -v

# Manual validation - run ralph and check output
python -m ralph -p "echo hello" -i 2
cat .agent/metrics/*.json | jq .
```
