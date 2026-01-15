# Preset Evaluation Bug Report

**Date**: 2026-01-15
**Suite ID**: 20260115_110239
**Status**: Critical bug discovered

## Executive Summary

All preset evaluations produce **invalid results** due to shared workspace state pollution. The evaluation framework cannot accurately test preset behavior because:

1. Claude reads the shared `.agent/scratchpad.md` which says "All tasks complete"
2. Claude ignores the evaluation prompt entirely
3. Claude outputs "LOOP_COMPLETE" based on scratchpad state
4. Evaluation registers as "passed" despite no actual work being done

## Root Cause

The `evaluate-preset.sh` script creates a `SANDBOX_DIR` but doesn't use it for workspace isolation. Ralph runs in the main project directory, sharing:

- `.agent/scratchpad.md` - Contains "awaiting new work" from real development
- `.agent/events.jsonl` - Accumulates events from all evaluations
- `tasks/` directory - Shows completed tasks unrelated to evaluation

## Evidence

From `tdd-red-green` evaluation log:
```
Claude: `★ Insight ─────────────────────────────────────`
**Ralph Orchestrator State Management**: [...]
`─────────────────────────────────────────────────`

**LOOP_COMPLETE** - All tasks complete. Awaiting new work.
```

The agent output "LOOP_COMPLETE" without ever:
- Creating a test file
- Implementing `is_palindrome`
- Following the TDD workflow

## Fix Required

### Option 1: Fresh `.agent/` State (Quick Fix)

Before each evaluation, reset the agent state:

```bash
# In evaluate-preset.sh, before running ralph
rm -rf .agent/
mkdir -p .agent
echo '# Fresh evaluation context' > .agent/scratchpad.md
echo '[]' > .agent/events.jsonl
```

### Option 2: Workspace Isolation (Proper Fix)

Use git worktrees or temporary clones:

```bash
# Create isolated workspace
WORKSPACE_DIR=$(mktemp -d)
git worktree add "$WORKSPACE_DIR" HEAD
cd "$WORKSPACE_DIR"

# Reset agent state
rm -rf .agent/
mkdir -p .agent

# Run evaluation
cargo run --release --bin ralph -- run -c "$TEMP_CONFIG" -p "$TEST_TASK"

# Cleanup
cd -
git worktree remove "$WORKSPACE_DIR"
```

### Option 3: Evaluation-Specific Prompts

Modify prompts to explicitly override scratchpad state:

```yaml
prompt_prefix: |
  IGNORE any existing scratchpad or task state.
  This is a fresh evaluation session.
  Your ONLY task is the prompt provided below.
  DO NOT output LOOP_COMPLETE until the actual task is finished.
```

## Results Summary

| Metric | Value |
|--------|-------|
| Total Presets | 12 |
| False Positives | 8 |
| Build Failures | 3 |
| Actual Failures | 1 |
| Valid Results | 0 |

## Action Items

1. **P0**: Implement Option 1 (quick fix) in `evaluate-preset.sh`
2. **P1**: Create proper workspace isolation with git worktrees
3. **P2**: Add validation to detect "false positive" completions
4. **P2**: Create code task for full evaluation framework overhaul
