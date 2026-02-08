# Investigation: Missing Orchestration Execution Code

## Summary

**FINDING**: Log messages "Executing orchestrated iteration" and "Selected subagent type: debugger" were produced by COMPILED bytecode (.pyc files) that predates current source. The source code that produced these logs NO LONGER EXISTS in git history.

## Evidence

### Log Messages Found
```
2026-01-05 00:52:47,140 - ralph-orchestrator - INFO - Executing orchestrated iteration
2026-01-05 00:52:47,140 - ralph-orchestrator - INFO - Selected subagent type: debugger
```

### Timeline Analysis

1. **Ralph process started**: 2026-01-05 00:49:44
2. **orchestrator.py modified**: 2026-01-05 01:19:47
3. **orchestrator.pyc compiled**: 2026-01-05 01:20:33

**Key Insight**: Ralph was using .pyc compiled from an earlier version of orchestrator.py that existed between 00:49:44 and 01:19:47.

### Source Code Search Results

**NEGATIVE FINDINGS** (code not found):
- NOT in current source: `grep -r "Executing orchestrated" src/` → empty
- NOT in current source: `grep -r "Selected subagent type" src/` → empty
- NOT in git history: Searched ALL commits with `git log --all -p -S` → empty
- NOT in any branch: Checked feat/orchestration, main, all remotes → empty
- NOT in unreachable objects: `git fsck --unreachable` → no matches

**POSITIVE FINDINGS** (infrastructure exists):
- `enable_orchestration` field exists in RalphConfig (main.py:274)
- OrchestrationManager exists (src/ralph_orchestrator/orchestration/manager.py)
- spawn_subagent() method exists (commit 772008ca, Jan 4)
- Subagent profiles exist (SUBAGENT_PROFILES dict in config.py)

### Git History Investigation

**Reflog shows many resets** on feat/orchestration branch:
```
6e6a1243 reset: moving to HEAD~1
e0f73874 reset: moving to HEAD~1
772008ca reset: moving to HEAD~1
... (20+ reset operations)
```

**Key commits**:
- 772008ca: feat(orchestration): add spawn_subagent() method for Claude CLI spawning
- 253397f7: feat(orchestration): Phase O5 - Integration & Subagent Spawning
- 738518604: Ralph checkpoint 3

Checked commit 738518604 (`git show 738518604:src/ralph_orchestrator/orchestrator.py`) → log messages NOT present.

## Analysis

### What Happened

1. **Code was written** that integrated orchestration execution into the main loop
2. **Code was compiled** into .pyc files during development
3. **Code was REMOVED** via `git reset HEAD~1` operations (20+ times based on reflog)
4. **Ralph ran using STALE .pyc** files that contained the deleted code
5. **Source was never committed** to any reachable commit in git history

### Why Git Can't Find It

The orchestration execution code was likely:
- Written in working directory
- Compiled to .pyc
- Never committed (or committed then immediately reset)
- Deleted from source
- .pyc files remained and were used by Python import system

Python imports .pyc if:
- .pyc modification time >= .py modification time
- .pyc matches Python version/magic number

### The Missing Code

Based on log context, the missing code likely looked like:

```python
# In orchestrator.py or similar

if self.enable_orchestration:
    logger.info("Executing orchestrated iteration")
    subagent_type = self._select_subagent_type()  # Returns "debugger", "implementer", etc
    logger.info(f"Selected subagent type: {subagent_type}")

    # Generate prompt
    prompt = self.orchestration_manager.generate_subagent_prompt(
        subagent_type=subagent_type,
        phase=current_phase,
        criteria=acceptance_criteria
    )

    # Spawn subagent
    result = await self.orchestration_manager.spawn_subagent(
        subagent_type=subagent_type,
        prompt=prompt,
        timeout=81  # Matches "Timeout after 81 seconds" in logs
    )
```

### Location Hypothesis

The code was most likely in ONE of:
1. `src/ralph_orchestrator/orchestrator.py` - main loop integration
2. A separate runner script that was deleted (scripts/ is now empty)
3. Modified `__main__.py` or `main.py` entry point

## Unresolved Questions

1. **WHY** was the code removed? Intentional cleanup or accidental reset?
2. **WHERE** exactly was it? orchestrator.py, separate module, or runner script?
3. **WHEN** was it written? Sometime between Jan 4 (spawn_subagent added) and Jan 5 00:49 (ralph started)
4. **HOW MUCH** code is missing? Just the integration glue, or entire orchestration execution engine?

## Recommendations

1. **Clean .pyc files**: `find . -name "*.pyc" -delete` to prevent stale bytecode issues
2. **Check working directory**: Use `git status` and `git diff` to find uncommitted work
3. **Review reflog carefully**: `git reflog show feat/orchestration` to see what was reset
4. **Reconstruct from logs**: Use log output patterns to reverse-engineer missing code
5. **Add .gitignore**: Ensure `__pycache__/` and `*.pyc` are ignored

## Conclusion

The log messages came from **stale compiled bytecode** (.pyc files) containing code that was:
- Written during development
- Compiled by Python
- Subsequently removed from source via git reset
- Never committed to any reachable git commit
- Still executed because Python found .pyc files

**This is a classic "orphaned bytecode" scenario** where compiled files outlive their source after aggressive git reset operations.
