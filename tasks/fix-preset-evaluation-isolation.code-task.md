---
status: completed
created: 2026-01-15
completed: 2026-01-15
---
# Task: Fix Preset Evaluation Workspace Isolation

## Description
The preset evaluation scripts (`tools/evaluate-preset.sh` and `tools/evaluate-all-presets.sh`) currently run evaluations in the main project workspace, causing shared state pollution. Each evaluation reads the existing `.agent/scratchpad.md` and `.agent/events.jsonl`, which contain state from previous work. This causes Claude to ignore evaluation prompts and output `LOOP_COMPLETE` prematurely, producing false positive results.

## Problem Evidence
From evaluation logs, Claude ignores the test prompt entirely:
```
Test Task: "Using TDD, implement an is_palindrome(s: &str) -> bool function..."

Claude's response:
"All tasks complete. LOOP_COMPLETE - Awaiting new work."
```

The agent never created a test file or implemented anything because the scratchpad said "All tasks complete."

## Technical Requirements

### Primary Fix: Reset Agent State Before Each Evaluation
1. In `tools/evaluate-preset.sh`, before running Ralph:
   - Backup existing `.agent/` directory (optional, for debugging)
   - Create fresh `.agent/` directory with clean state
   - Initialize empty/minimal `scratchpad.md` and `events.jsonl`

2. After evaluation completes:
   - Optionally restore original `.agent/` state
   - Or leave it clean for subsequent evaluations

### Implementation Details

Add state reset logic in `evaluate-preset.sh` around line 100 (before `cargo run`):

```bash
# Backup and reset agent state for clean evaluation
AGENT_BACKUP_DIR="$LOG_DIR/agent-backup"
if [[ -d ".agent" ]]; then
    cp -r .agent "$AGENT_BACKUP_DIR"
fi

# Create fresh agent state
rm -rf .agent
mkdir -p .agent
cat > .agent/scratchpad.md << 'SCRATCHPAD_EOF'
# Scratchpad — Preset Evaluation

## Current Status
**Mode**: Preset Evaluation
**Task**: See prompt below

## Active Task
Follow the instructions in the prompt. This is a fresh evaluation context.
SCRATCHPAD_EOF

echo '[]' > .agent/events.jsonl
```

Add cleanup/restoration after evaluation (around line 170, after metrics extraction):

```bash
# Restore original agent state if backup exists
if [[ -d "$AGENT_BACKUP_DIR" ]]; then
    rm -rf .agent
    mv "$AGENT_BACKUP_DIR" .agent
fi
```

## Files to Modify
- `tools/evaluate-preset.sh` - Add state reset before evaluation, restore after

## Dependencies
- None - this is a shell script modification only

## Acceptance Criteria

1. **Clean State Before Evaluation**
   - Given the evaluation script runs
   - When it starts evaluating a preset
   - Then `.agent/scratchpad.md` contains only evaluation-context content (not "all tasks complete")
   - And `.agent/events.jsonl` is empty or contains only `[]`

2. **Original State Preserved**
   - Given the evaluation script runs to completion
   - When evaluation finishes (success or failure)
   - Then the original `.agent/` directory is restored
   - And no permanent changes are made to the developer's workspace

3. **Evaluation Actually Performs Task**
   - Given the `tdd-red-green` preset is evaluated with clean state
   - When Claude runs
   - Then it should attempt to create test files in `.eval-sandbox/`
   - And it should NOT immediately output "LOOP_COMPLETE" without doing work

4. **Logging for Debugging**
   - Given evaluation runs
   - When state reset occurs
   - Then the original `.agent/` content is preserved in `$LOG_DIR/agent-backup/`
   - And this can be inspected for debugging false positives

## Testing
After implementation:
```bash
# Run single preset evaluation
./tools/evaluate-preset.sh tdd-red-green claude

# Verify the output log shows actual TDD work (test creation, implementation)
cat .eval/logs/tdd-red-green/latest/output.log | grep -E "(test|is_palindrome|\.rs)"

# Verify original .agent/ was restored
cat .agent/scratchpad.md  # Should show original content, not evaluation content
```

## Notes
- This is the "quick fix" approach. A more robust solution would use git worktrees for full workspace isolation.
- The backup/restore pattern ensures developer workflow isn't disrupted by running evaluations.
- Consider adding a `--no-restore` flag for CI environments where state preservation isn't needed.
