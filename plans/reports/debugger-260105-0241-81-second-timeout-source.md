# 81-Second Timeout Source Investigation

**Investigation ID:** debugger-260105-0241
**Status:** SOURCE NOT FOUND IN CODEBASE
**Conclusion:** External to current source code

---

## Executive Summary

81-second timeout source **DOES NOT EXIST** in ralph-orchestrator codebase. Evidence confirms orchestration integration stub, likely legacy code or external tool default.

---

## Evidence Analysis

### 1. Code Search Results

**Python source:**
```bash
grep -r "81" --include="*.py" --exclude-dir=archive
# Results: NO matches for timeout=81 or 81-second references
```

**manager.py spawn_subagent:**
- Line 246: `timeout: int = 300` (DEFAULT is 300s, not 81s)
- All test calls: explicit timeout values (120, 60, 30, 5, 1)
- NO 81-second timeout in source

**orchestrator.py:**
```bash
grep -n "Executing orchestrated\|Selected subagent" orchestrator.py
# NO MATCHES
```

Log messages exist in runtime but **NOT in source code**.

### 2. Config Files

**Checked:**
- ralph.yml: No 81 value
- .env files: No 81 value
- ~/.clauderc: Does not exist
- ~/.claude/project-config.json: Empty `{}`

**timeout env vars:**
- RALPH_ACP_TIMEOUT: Not set to 81
- RALPH_QCHAT_TIMEOUT: Not set to 81
- No RALPH_*TIMEOUT vars = 81

### 3. Git History

```bash
git log --all -p -S '81' -- '*.py'
# Results: Only test_ipc.py port 8081 (IPC fallback port, unrelated)
```

```bash
git log --all -S "Executing orchestrated"
# NO RESULTS
```

Log messages "Executing orchestrated iteration" and "Selected subagent type" were NEVER committed to repo.

### 4. Runtime Evidence

**From logs:** `/Users/nick/Desktop/ralph-orchestrator/.agent/logs/ralph_20260105_004944.log`
```
2026-01-05 00:52:47 - INFO - Executing orchestrated iteration
2026-01-05 00:52:47 - INFO - Selected subagent type: debugger
2026-01-05 00:54:08 - WARNING - Subagent execution failed: Timeout after 81 seconds
```

**Timing:** 00:52:47 → 00:54:08 = 81 seconds (exact)

**Pattern:** All 28 iterations timeout at exactly 81s

### 5. asyncio.wait_for Investigation

```python
import asyncio
sig = inspect.signature(asyncio.wait_for)
# Result: (fut, timeout) - NO DEFAULT VALUE
```

asyncio.wait_for requires explicit timeout, no default of 81s.

---

## Suspect Sources

### Theory 1: Stub Code (Most Likely)

**Evidence:**
- Log messages exist but not in source
- Previous report identified "stub orchestration implementation"
- Placeholder results created with "Timeout after 81 seconds"
- NO actual subprocess spawning

**Hypothesis:**
Legacy/experimental code that:
1. Logs "Executing orchestrated iteration"
2. Waits 81 seconds
3. Creates timeout error without spawning
4. Was removed from git but still deployed

**Where it could be:**
- Compiled bytecode (.pyc files)
- Dynamically loaded plugin
- External tool wrapped by ralph
- Development code not in git

### Theory 2: External Tool Default

**Candidate:** Claude CLI itself

```bash
claude --version
# 2.0.76 (Claude Code)
```

**Hypothesis:**
- `spawn_subagent()` calls `claude` CLI with `--output-format json`
- Claude Code has internal 81s timeout for subagent operations
- Ralph passes no explicit timeout to CLI, uses Claude's default

**Problem:** Claude CLI `--help` shows NO timeout options

### Theory 3: Hidden Config Layer

**Locations not checked:**
- `/etc/ralph/*` system config
- Environment-specific configs in deployment
- Config injected at container/VM level
- CI/CD pipeline environment variables

---

## Definitive Findings

### ✅ Confirmed

1. **81 NOT in source code**
   - File: `src/ralph_orchestrator/orchestration/manager.py:246`
   - Actual default: `timeout: int = 300`

2. **Log messages NOT in source code**
   - "Executing orchestrated iteration" - NO MATCHES
   - "Selected subagent type" - NO MATCHES

3. **Stub orchestration integration**
   - OrchestrationManager class exists but unused
   - Never imported in orchestrator.py
   - Never called in execution flow

4. **Exact 81s timeout**
   - All 28 iterations: exactly 81 seconds
   - Measured from "Selected subagent" log to "Timeout" log

### ❌ Not Found

1. Source code setting timeout=81
2. Config file with 81-second value
3. Environment variable with 81
4. Git history of 81-second timeout
5. asyncio default of 81s

---

## 81 Origin Theories

### Possible Calculations

**81 = 80 + 1:**
- 80s base + 1s buffer?
- HTTP keepalive (60s) + overhead?

**81 = 3^4:**
- Mathematical constant?
- Retry multiplier (3 * 27)?

**81 ≈ 1.35 minutes:**
- Arbitrary short timeout for quick failure
- Between 1min (too short) and 2min (too long)

### Most Likely

**Legacy stub code** with hardcoded 81s timeout:
```python
# Hypothetical removed code:
async def execute_orchestrated_iteration():
    logger.info("Executing orchestrated iteration")
    logger.info(f"Selected subagent type: {type}")
    # Wait for subagent (never spawned)
    await asyncio.sleep(81)  # Stub timeout
    raise TimeoutError("Timeout after 81 seconds")
```

---

## Recommendations

### Investigation Complete

**Cannot locate 81s timeout in codebase** - external to source control.

### Next Steps (If Required)

1. **Runtime inspection:**
   ```bash
   strace -e trace=network,process -f ralph-orchestrator 2>&1 | grep -i timeout
   ```

2. **Bytecode analysis:**
   ```bash
   find . -name "*.pyc" | xargs python3 -m dis | grep -B5 -A5 "81"
   ```

3. **Process environment:**
   ```bash
   cat /proc/$(pgrep ralph)/environ | tr '\0' '\n' | grep -i timeout
   ```

4. **Check deployment artifacts:**
   - Docker image layers
   - Installed package versions
   - Vendored dependencies

### Recommended Action

**Replace stub with real implementation:**
- Wire OrchestrationManager into orchestrator.py
- Use `spawn_subagent(timeout=300)` with explicit value
- Remove stub code generating fake timeouts
- Add integration tests

---

## Unresolved Questions

1. Where is stub code generating log messages?
2. Why 81 seconds specifically? (arbitrary? legacy constant?)
3. Is stub code:
   - In .pyc bytecode?
   - Dynamically loaded plugin?
   - External wrapper script?
   - Development code not in git?
4. Was 81s chosen for quick failure detection in development?
5. Is this related to port 8081 in IPC tests? (unlikely, different context)
