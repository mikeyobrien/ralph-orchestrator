# Orchestration SDK Fix - Implementation Plan

## Objective

Fix Ralph Orchestrator's orchestration mode to use the existing ClaudeAdapter (which properly implements claude_agent_sdk) instead of raw subprocess calls to the "claude" CLI.

**Current broken path:**
```
enable_orchestration=True → _execute_orchestrated_iteration()
  → OrchestrationManager.spawn_subagent()
  → asyncio.create_subprocess_exec("claude", "-p", ...) ❌ BYPASSES SDK
```

**Correct path should be:**
```
enable_orchestration=True → _execute_orchestrated_iteration()
  → OrchestrationManager.spawn_subagent()
  → ClaudeAdapter.aexecute(prompt, **subagent_options) ✅ USES SDK
```

---

## Context Files

Reference these files during implementation:
- @src/ralph_orchestrator/orchestrator.py - Main orchestrator (lines 744-746 show orchestration branch)
- @src/ralph_orchestrator/orchestration/manager.py - OrchestrationManager (lines 279-368 spawn_subagent)
- @src/ralph_orchestrator/adapters/claude.py - ClaudeAdapter with SDK integration (line 272 shows query())
- @examples/mobile.yml - Validation config (enable_orchestration: true)

---

## Root Cause Analysis

**Problem:** `OrchestrationManager.spawn_subagent()` (manager.py:322) uses:
```python
proc = await asyncio.create_subprocess_exec(
    "claude", "-p", prompt, "--model", "claude-opus-4-5-20251101", ...
)
```

**This bypasses:**
1. claude_agent_sdk integration
2. MCP server inheritance from settings.json
3. Plugin loading
4. Message streaming
5. Setting sources (user, project, local)

**The ClaudeAdapter (adapters/claude.py) ALREADY HAS all these features:**
- Line 20: `from claude_agent_sdk import ClaudeAgentOptions, query`
- Line 230-234: `setting_sources=['user', 'project', 'local']` - inherits MCP servers
- Line 272: `async for message in query(prompt=prompt, options=options)`

---

## Phased Implementation Plan

### Phase 1: Pass ClaudeAdapter to OrchestrationManager

**Acceptance Criteria:**
- [ ] OrchestrationManager.__init__() accepts optional `adapter` parameter
- [ ] RalphOrchestrator passes its ClaudeAdapter to OrchestrationManager
- [ ] OrchestrationManager stores adapter reference

**Implementation:**
1. Modify `OrchestrationManager.__init__()` to accept adapter parameter
2. In `orchestrator.py` line 171-176, pass `self.current_adapter` to OrchestrationManager

**Validation Gate:**
```bash
python -c "
from ralph_orchestrator.orchestration.manager import OrchestrationManager
from ralph_orchestrator.adapters.claude import ClaudeAdapter

adapter = ClaudeAdapter(verbose=True)
# Should accept adapter parameter now
class MockConfig:
    pass
om = OrchestrationManager(config=MockConfig(), adapter=adapter)
print(f'✅ OrchestrationManager accepts adapter: {om.adapter is not None}')
" 2>&1
```

---

### Phase 2: Refactor spawn_subagent() to use adapter.aexecute()

**Acceptance Criteria:**
- [ ] spawn_subagent() calls self.adapter.aexecute() instead of subprocess
- [ ] Subagent-specific system_prompt is passed to adapter
- [ ] Returns structured result compatible with existing code

**Implementation:**
Replace subprocess code (manager.py:320-368) with:
```python
async def spawn_subagent(
    self,
    subagent_type: str,
    prompt: str,
    timeout: int = 300,
) -> Dict[str, Any]:
    """Spawn Claude subagent using ClaudeAdapter."""

    # Verify required MCPs before spawning
    self.verify_required_mcps(subagent_type)

    logger.info(f"Spawning {subagent_type} subagent via ClaudeAdapter")

    result = {
        "subagent_type": subagent_type,
        "success": False,
        "output": "",
        "error": None,
    }

    try:
        # Use the adapter (which uses claude_agent_sdk properly)
        response = await self.adapter.aexecute(
            prompt,
            system_prompt=f"You are a specialized {subagent_type} subagent.",
            # Adapter inherits MCP servers via setting_sources
        )

        result["success"] = response.success
        result["output"] = response.output
        result["tokens_used"] = response.tokens_used

        if not response.success:
            result["error"] = response.error

    except Exception as e:
        result["error"] = str(e)
        logger.error(f"Error spawning subagent: {e}")

    return result
```

**Validation Gate:**
```bash
python -c "
from ralph_orchestrator.orchestration.manager import OrchestrationManager
from ralph_orchestrator.adapters.claude import ClaudeAdapter

adapter = ClaudeAdapter(verbose=False)
class MockConfig:
    pass

om = OrchestrationManager(config=MockConfig(), adapter=adapter)

# Check that spawn_subagent no longer uses subprocess
import inspect
source = inspect.getsource(om.spawn_subagent)
uses_subprocess = 'create_subprocess_exec' in source
uses_adapter = 'self.adapter.aexecute' in source or 'adapter.aexecute' in source

print(f'Uses subprocess: {uses_subprocess} (should be False)')
print(f'Uses adapter: {uses_adapter} (should be True)')

if not uses_subprocess and uses_adapter:
    print('✅ Phase 2 PASS: spawn_subagent uses adapter')
else:
    print('❌ Phase 2 FAIL: still using subprocess')
" 2>&1
```

---

### Phase 3: Verify MCP Server Inheritance

**Acceptance Criteria:**
- [ ] Subagent has access to MCP servers from ~/.claude/settings.json
- [ ] sequential-thinking, playwright, tavily available to subagents
- [ ] MCP verification no longer fails for properly configured servers

**Validation Gate:**
```bash
python -c "
from ralph_orchestrator.orchestration.discovery import discover_mcps
from ralph_orchestrator.orchestration.manager import OrchestrationManager
from ralph_orchestrator.adapters.claude import ClaudeAdapter

# Check MCPs are discovered from settings.json
mcps = discover_mcps()
print(f'Discovered {len(mcps)} MCP servers')

required = ['sequential-thinking', 'playwright']
missing = [m for m in required if m not in mcps]
if missing:
    print(f'❌ Missing required MCPs: {missing}')
else:
    print(f'✅ All required MCPs available: {required}')

# Test MCP verification doesn't fail
adapter = ClaudeAdapter(verbose=False)
class MockConfig:
    pass
om = OrchestrationManager(config=MockConfig(), adapter=adapter)

try:
    om.verify_required_mcps('validator')
    print('✅ MCP verification passed for validator')
except Exception as e:
    print(f'❌ MCP verification failed: {e}')
" 2>&1
```

---

### Phase 4: Integration Test with mobile.yml

**Acceptance Criteria:**
- [ ] `ralph run -c examples/mobile.yml` starts without errors
- [ ] Logs show "OrchestrationManager initialized - subagent mode enabled"
- [ ] Subagent spawns and executes
- [ ] Coordination files created in .agent/coordination/

**Validation Gate:**
```bash
# Run Ralph with orchestration enabled (1 iteration for quick test)
timeout 120 ralph run -c examples/mobile.yml --max-iterations 1 2>&1 | tee /tmp/ralph-orchestration-test.log

# Check for success indicators
echo ""
echo "=== VALIDATION CHECKS ==="

if grep -q "OrchestrationManager initialized" /tmp/ralph-orchestration-test.log; then
    echo "✅ OrchestrationManager initialized"
else
    echo "❌ OrchestrationManager NOT initialized"
fi

if grep -q "Spawning.*subagent" /tmp/ralph-orchestration-test.log; then
    echo "✅ Subagent spawning detected"
else
    echo "❌ Subagent spawning NOT detected"
fi

if ls .agent/coordination/subagent-results/*.json 2>/dev/null; then
    echo "✅ Coordination files created"
else
    echo "❌ No coordination files found"
fi
```

---

### Phase 5: Full Orchestration Test (Long Running)

**Acceptance Criteria:**
- [ ] Run mobile.yml for 30+ minutes
- [ ] Multiple subagent iterations complete
- [ ] Real progress on mobile app development
- [ ] No MCP-related errors in logs
- [ ] Coordination files show subagent communication

**Validation Gate:**
```bash
# Full test - run and monitor
ralph run -c examples/mobile.yml --verbose 2>&1 | tee /tmp/ralph-full-test.log &
RALPH_PID=$!

# Monitor for 30 minutes
sleep 1800

# Check if still running (good sign)
if ps -p $RALPH_PID > /dev/null; then
    echo "✅ Ralph still running after 30 minutes"
else
    echo "⚠️ Ralph exited before 30 minutes"
fi

# Count subagent iterations
ITERATIONS=$(grep -c "Spawning.*subagent" /tmp/ralph-full-test.log || echo 0)
echo "Subagent iterations: $ITERATIONS"

# Check for errors
ERRORS=$(grep -c "ERROR\|Error\|error" /tmp/ralph-full-test.log || echo 0)
echo "Error count: $ERRORS"

# Graceful shutdown
kill -INT $RALPH_PID 2>/dev/null
```

---

## Output Requirements

### Primary Output
Save implementation plan to: `.prompts/002-orchestration-sdk-fix-plan/orchestration-sdk-fix-plan.md`

### SUMMARY.md
Create `.prompts/002-orchestration-sdk-fix-plan/SUMMARY.md` with:
- One-liner describing the fix
- Key findings from analysis
- Decisions needed (if any)
- Blockers (if any)
- Next step after plan approval

---

## Success Criteria

**Plan is COMPLETE when:**
1. All 5 phases have clear acceptance criteria
2. Each phase has executable validation gate (bash commands)
3. mobile.yml is the ultimate integration test
4. ClaudeAdapter is used instead of subprocess
5. MCP servers are inherited from settings.json

**DO NOT proceed to implementation until plan is approved.**

---

## Metadata

<confidence>HIGH - Root cause is clear, fix is straightforward</confidence>
<dependencies>
- ClaudeAdapter already has SDK integration (verified)
- MCP servers in ~/.claude/settings.json (verified)
- mobile.yml config exists with enable_orchestration: true
</dependencies>
<open_questions>
- Should subagent prompts include explicit MCP tool instructions?
- How to handle subagent timeouts with adapter?
</open_questions>
<assumptions>
- ClaudeAdapter.aexecute() will properly inherit MCP servers
- No breaking changes to adapter interface needed
</assumptions>
