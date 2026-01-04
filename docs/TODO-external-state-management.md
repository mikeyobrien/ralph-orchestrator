# TODO: External State Management

## Problem

Currently, Ralph's self-improvement prompt modifies itself as it runs:
- Checkboxes get updated (e.g., `- [ ]` → `- [x]`)
- Status tables change
- Progress tracking sections update

This causes issues:
1. **False positive completion markers**: The literal text `TASK_COMPLETE` in examples triggers completion detection
2. **Confusion on restart**: Ralph may not know where it left off
3. **Git pollution**: Every checkpoint modifies the prompt file

## Proposed Solution

Move state tracking OUT of the prompt file into an external store:

### Option 1: JSON State File

```json
// ~/.ralph/state/{prompt-hash}.json
{
  "prompt_file": "prompts/SELF_IMPROVEMENT_PROMPT.md",
  "prompt_hash": "abc123",
  "started_at": "2026-01-04T04:00:00Z",
  "current_phase": "01",
  "current_plan": "01-02",
  "completed_plans": ["00-01", "00-02", "00-03", "00-04", "01-01"],
  "test_counts": {
    "00": 60,
    "01-01": 17
  },
  "commits": [
    {"plan": "01-01", "hash": "f28c381"}
  ]
}
```

### Option 2: SQLite Database

Use the existing `~/.ralph/history.db`:
- Add `prompt_progress` table
- Track phase/plan completion
- Query to determine where to resume

### Option 3: Git Tags

Use git tags to mark progress:
- `ralph-phase-00-complete`
- `ralph-plan-01-01-complete`
- Ralph checks tags to determine state

## Benefits

1. **Prompt stays static**: No self-modification needed
2. **Clean restarts**: State is separate, prompt is blueprint
3. **No false positives**: No completion markers in prompt
4. **Audit trail**: Clear history of what was completed when

## Implementation Notes

1. Orchestrator reads prompt to understand what to do
2. Orchestrator reads state file to understand what's done
3. Orchestrator continues from where it left off
4. Orchestrator updates state file (not prompt) on completion

## Priority

**Low** - Current workaround (placeholder for completion marker) works. Implement when we have time for proper architecture.

## Related

- Issue: TASK_COMPLETE false positive (fixed with placeholder)
- File: `src/ralph_orchestrator/orchestrator.py` - `_check_completion_marker()`
