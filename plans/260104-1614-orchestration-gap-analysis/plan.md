---
title: "Ralph Orchestrator Gap Resolution Plan"
description: "Two approaches to fix identified gaps in orchestration, mobile, and validation"
status: pending
priority: P1
effort: "Approach A: 60-80h | Approach B: 30-40h"
branch: feat/orchestration
tags: [orchestration, mobile, validation, gap-analysis]
created: 2026-01-04
---

# Ralph Orchestrator Gap Resolution Plan

## Executive Summary

Gap analysis revealed five critical issues:

| Gap | Severity | Impact |
|-----|----------|--------|
| No subagent spawning code | CRITICAL | Orchestration cannot actually run |
| Empty evidence directories (O1-O5) | HIGH | Phases unverified |
| Mobile TypeScript build fails | HIGH | App won't ship |
| Validation marks errors as SUCCESS | HIGH | False completion |
| Mobile UI screens are stubs | MEDIUM | Half-finished app |

This document presents two resolution approaches with effort estimates, specific file changes, and a final recommendation.

---

## Critical Gaps Detail

### Gap 1: NO SUBAGENT SPAWNING CODE

**Location:** `src/ralph_orchestrator/orchestration/`

The orchestration module contains:
- `manager.py` - Generates prompts, aggregates results
- `coordinator.py` - File-based coordination protocol
- `discovery.py` - MCP/skill discovery
- `config.py` - Subagent profiles

**Missing:** Any function to actually spawn a Claude subprocess. No `spawn_subagent()`, no `subprocess.run()`, no SDK integration. The integration tests manually write mock results.

### Gap 2: EMPTY EVIDENCE DIRECTORIES

```
validation-evidence/
├── orchestration-00/  # HAS 2 files
├── orchestration-01/  # .gitkeep only
├── orchestration-02/  # .gitkeep only
├── orchestration-03/  # .gitkeep only
├── orchestration-04/  # .gitkeep only
└── orchestration-05/  # .gitkeep only
```

Prompt claims phases O1-O5 VALIDATED but no evidence exists.

### Gap 3: MOBILE TYPESCRIPT BUILD FAILS

```
lib/pushNotificationHelpers.ts(296,9): error TS7053
Element implicitly has an 'any' type because expression of type
'NotificationType' can't be used to index type 'NotificationPreferences'.
```

App cannot compile for production.

### Gap 4: VALIDATION FALSE POSITIVES

Evidence file `phase-06/control-api.txt` shows:
```json
{"detail":"Orchestrator not found"}
{"detail":"Orchestrator not found"}
{"detail":"Orchestrator not found"}
Status: SUCCESS
```

API returns errors but marked SUCCESS. Validation checks file existence, not content correctness.

### Gap 5: MOBILE UI STUBS

| Screen | Status |
|--------|--------|
| Dashboard (index.tsx) | Partial - list works, nav TODO |
| History (history.tsx) | Stub - placeholder only |
| Settings (settings.tsx) | Stub - static layout |
| Detail ([id].tsx) | Missing - route exists, file doesn't |
| Login | Missing - no auth UI |

---

# APPROACH A: "Fix What We Have" (Incremental)

**Philosophy:** Preserve existing code, add missing pieces, fix broken parts.

**Total Effort:** 60-80 hours

## Phase A1: Fix Immediate Blockers
**Effort:** 4-6 hours

### Tasks:
1. Fix TypeScript error in `pushNotificationHelpers.ts:296`
2. Verify mobile app builds with `npx expo export`
3. Document fix in validation evidence

### Files to Modify:
- `ralph-mobile/lib/pushNotificationHelpers.ts` - Add proper type guard

### Acceptance Criteria:
- [ ] `tsc --noEmit` passes with zero errors
- [ ] `npx expo export` completes successfully
- [ ] Evidence screenshot of successful build

### Dependencies: None

---

## Phase A2: Implement Subagent Spawning
**Effort:** 16-20 hours

### Tasks:
1. Add `spawn_subagent()` to `manager.py`
2. Choose execution method (CLI vs SDK)
3. Implement timeout/retry logic
4. Add async subagent management
5. Integration test with real Claude subprocess

### Files to Modify:
- `src/ralph_orchestrator/orchestration/manager.py` - Add spawn logic
- `src/ralph_orchestrator/orchestration/__init__.py` - Export new functions
- `tests/test_orchestration_integration.py` - Add real spawn tests

### New Code Required:
```python
# In manager.py
async def spawn_subagent(
    self,
    subagent_type: str,
    prompt: str,
    timeout: int = 300
) -> Dict[str, Any]:
    """Spawn Claude subagent and collect results."""
    # Option A: CLI approach
    proc = await asyncio.create_subprocess_exec(
        "claude", "-p", prompt,
        "--output-format", "json",
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE
    )
    stdout, stderr = await asyncio.wait_for(
        proc.communicate(), timeout=timeout
    )
    # Parse response, write to coordination files
    ...
```

### Acceptance Criteria:
- [ ] `spawn_subagent()` function exists and documented
- [ ] Unit tests with mocked subprocess pass
- [ ] Integration test spawns real Claude process
- [ ] Timeout handling works (test with 1s timeout)
- [ ] Results written to coordination directory

### Dependencies: Phase A1 (need working build first)

### Risks:
- Claude CLI behavior may change
- Rate limiting on API calls
- Subprocess isolation complexity

---

## Phase A3: Fix Validation System
**Effort:** 12-16 hours

### Tasks:
1. Add semantic content validation
2. Parse JSON responses for error detection
3. Implement screenshot comparison
4. Enable validation by default
5. Add phase-specific validators

### Files to Modify:
- `src/ralph_orchestrator/orchestrator.py` (lines 886-965) - Enhanced checks
- `tests/test_validation_evidence_freshness.py` - New test cases

### New Validation Checks:
```python
def _validate_json_evidence(self, filepath: Path) -> bool:
    """Check JSON files for error patterns."""
    content = filepath.read_text()
    data = json.loads(content)

    # Check for API error patterns
    if isinstance(data, dict):
        if "detail" in data and "not found" in str(data["detail"]).lower():
            return False
        if "error" in data:
            return False
    return True
```

### Acceptance Criteria:
- [ ] JSON evidence files parsed for errors
- [ ] `{"detail":"...not found"}` fails validation
- [ ] `{"error":"..."}` fails validation
- [ ] Test with known-bad evidence files
- [ ] `enable_validation` defaults to `True`

### Dependencies: None (can parallel with A2)

---

## Phase A4: Complete Mobile UI
**Effort:** 20-28 hours

### Tasks:
1. Create `app/orchestrator/[id].tsx` detail screen
2. Wire navigation from OrchestratorCard
3. Create login screen with auth flow
4. Implement history screen with real data
5. Wire settings logout button
6. Add missing hooks (useAuth, useWebSocket)
7. Implement control buttons UI

### Files to Create:
- `ralph-mobile/app/orchestrator/[id].tsx`
- `ralph-mobile/app/login.tsx`
- `ralph-mobile/hooks/useAuth.ts` (test exists, hook doesn't)
- `ralph-mobile/hooks/useWebSocket.ts` (test exists, hook doesn't)

### Files to Modify:
- `ralph-mobile/app/(tabs)/history.tsx` - Replace stub
- `ralph-mobile/app/(tabs)/settings.tsx` - Wire logout
- `ralph-mobile/components/OrchestratorCard.tsx` - Add navigation

### Acceptance Criteria:
- [ ] Tap card navigates to detail screen
- [ ] Detail screen shows tasks, logs, controls
- [ ] Login screen with email/password
- [ ] Logout redirects to login
- [ ] History shows past orchestrations
- [ ] Control buttons (stop/pause/resume) work

### Dependencies: Phase A1 (TypeScript must compile)

---

## Phase A5: Generate Real Evidence
**Effort:** 8-12 hours

### Tasks:
1. Run orchestration phases O1-O5 with real execution
2. Capture evidence to proper directories
3. Screenshot functional states (not empty states)
4. Validate evidence against criteria

### Evidence Required:
```
validation-evidence/
├── orchestration-01/  # SubagentProfile usage
├── orchestration-02/  # Skill discovery output
├── orchestration-03/  # MCP discovery output
├── orchestration-04/  # Coordination file creation
└── orchestration-05/  # Real subagent spawn
```

### Acceptance Criteria:
- [ ] Each directory has 2+ evidence files
- [ ] Evidence files created during actual run (not tests)
- [ ] No error patterns in TXT files
- [ ] Screenshots show populated states

### Dependencies: Phases A2, A3, A4 (need working system)

---

## Approach A Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Mobile complexity grows | High | Medium | Time-box to core features |
| Subagent spawning edge cases | Medium | High | Extensive error handling |
| Validation changes break existing | Medium | Medium | Run all tests after changes |
| Evidence generation takes long | Low | Medium | Automate where possible |

## Approach A Pros/Cons

**Pros:**
- Preserves existing work (67 source files, 62 test files)
- Incremental progress visible
- Tests already exist for most components
- Mobile foundation is solid

**Cons:**
- Technical debt accumulates
- Mobile app may never feel complete
- Validation fixes are patches not redesign
- Subagent spawning retrofitted rather than designed

---

# APPROACH B: "Clean Slate" (Strategic)

**Philosophy:** Simplify scope, focus on core value, defer non-essential.

**Total Effort:** 30-40 hours

## Phase B1: Remove Mobile App
**Effort:** 2 hours

### Rationale:
Mobile app is half-built, tests are mocked, TypeScript fails. The core value of Ralph is orchestration, not mobile viewing. A web UI already exists.

### Tasks:
1. Move `ralph-mobile/` to `archive/ralph-mobile-v1/`
2. Update documentation to note mobile deferred
3. Close mobile-related issues/PRs

### Files to Modify:
- `README.md` - Remove mobile references
- `docs/roadmap.md` - Note mobile as future work

### Acceptance Criteria:
- [ ] Mobile directory archived
- [ ] No broken references in docs
- [ ] Clear note that mobile is v2.0 feature

### Dependencies: None

---

## Phase B2: Implement Subagent Spawning (Core)
**Effort:** 12-16 hours

Same as Phase A2 but with more focus:
- No mobile distractions
- Full attention on execution correctness
- Better error handling

### Files to Modify:
- `src/ralph_orchestrator/orchestration/manager.py`
- `tests/test_orchestration_integration.py`

### Additional Focus:
- Structured output parsing
- Better timeout handling
- Resource cleanup on failure
- Logging for debugging

### Acceptance Criteria:
- [ ] `spawn_subagent()` works with real Claude
- [ ] Handles timeout gracefully
- [ ] Cleans up on error
- [ ] Results properly aggregated

### Dependencies: None

---

## Phase B3: Rebuild Validation from Scratch
**Effort:** 10-14 hours

### Rationale:
Current validation is fundamentally flawed (opt-in, no semantic checks). Better to rebuild with correct assumptions.

### New Validation Design:
1. Validation is MANDATORY (always on)
2. Each phase has specific validator
3. JSON content is parsed and checked
4. Screenshots require hash comparison to baseline
5. Human approval gate for final completion

### Files to Create:
- `src/ralph_orchestrator/validation/` (new module)
  - `__init__.py`
  - `base_validator.py` - Abstract validator
  - `phase_validators.py` - Per-phase logic
  - `evidence_checker.py` - Content analysis
  - `approval_gate.py` - Human confirmation

### Files to Modify:
- `src/ralph_orchestrator/orchestrator.py` - Use new validation module

### Acceptance Criteria:
- [ ] Validation runs automatically on completion
- [ ] Phase-specific checks implemented
- [ ] JSON errors detected and reported
- [ ] Clear failure messages
- [ ] Human approval required for TASK_COMPLETE

### Dependencies: Phase B2 (need working spawning)

---

## Phase B4: Integration Test Suite
**Effort:** 6-10 hours

### Rationale:
Current tests are unit tests with mocks. Need real integration tests.

### Tasks:
1. Create integration test harness
2. Test real Claude spawning (with short prompts)
3. Test validation with good/bad evidence
4. End-to-end orchestration flow

### Files to Create:
- `tests/integration/test_real_orchestration.py`
- `tests/integration/test_validation_semantic.py`
- `tests/integration/conftest.py` - Fixtures for real runs

### Acceptance Criteria:
- [ ] Integration tests use real (not mocked) Claude
- [ ] Tests have reasonable timeouts (5 min max)
- [ ] CI can skip expensive tests with marker
- [ ] At least 3 end-to-end scenarios covered

### Dependencies: Phases B2, B3

---

## Approach B Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Stakeholder wants mobile | Medium | High | Document as v2.0 scope |
| Rebuild takes longer | Low | Medium | Use existing code as reference |
| Integration tests flaky | Medium | Medium | Retry logic, clear timeouts |

## Approach B Pros/Cons

**Pros:**
- Focused scope = faster delivery
- Clean validation architecture
- Real integration tests
- No mobile technical debt
- Core functionality fully working

**Cons:**
- Mobile work is "thrown away" (archived)
- Some existing tests become obsolete
- User-facing features reduced
- Need to communicate scope change

---

# Comparison Matrix

| Dimension | Approach A | Approach B |
|-----------|------------|------------|
| **Effort** | 60-80 hours | 30-40 hours |
| **Scope** | Full feature set | Core orchestration |
| **Mobile** | Completed (16-20h) | Deferred to v2.0 |
| **Validation** | Patched | Rebuilt |
| **Tests** | Unit + some integration | Full integration suite |
| **Risk** | Higher (more surface) | Lower (focused) |
| **Time to Usable** | 4-6 weeks | 2-3 weeks |
| **Technical Debt** | Increases | Decreases |

---

# RECOMMENDATION: Approach B with Mobile Archive

**Reasoning:**

1. **Core Value First**: Ralph's value is orchestration. A half-working mobile app provides no value; working orchestration does.

2. **80/20 Rule**: Approach B delivers 80% of value (working orchestration, real validation) in 40% of the time.

3. **YAGNI**: The mobile app was built speculatively. No users have requested it. Defer until real demand.

4. **Quality Over Features**: Better to have one thing that works perfectly than three things half-working.

5. **Validation is Broken**: The false positive in `control-api.txt` proves the validation system cannot be trusted. A patch isn't enough.

6. **Evidence Shows Reality**: The gap analysis proves claims exceeded implementation. Time to reset expectations.

## Recommended Execution Order

```
Week 1:
  B1: Archive mobile (2h)
  B2: Subagent spawning (12-16h)

Week 2:
  B3: Rebuild validation (10-14h)

Week 3:
  B4: Integration tests (6-10h)
  B5: Generate real evidence for O1-O5 (4-6h)

Total: 30-40 hours over 3 weeks
```

## Success Metrics

After Approach B completion:
- [ ] Can run `ralph orchestrate` and spawn real subagents
- [ ] Validation catches actual errors (not just existence)
- [ ] Integration tests pass with real Claude calls
- [ ] O1-O5 have legitimate evidence files
- [ ] No false positives in validation

---

# Appendix: Files Reference

## Critical Files to Modify

| File | Changes |
|------|---------|
| `src/ralph_orchestrator/orchestration/manager.py` | Add `spawn_subagent()` |
| `src/ralph_orchestrator/orchestrator.py` | New validation integration |
| `tests/test_orchestration_integration.py` | Real spawn tests |

## Files to Create (Approach B)

| File | Purpose |
|------|---------|
| `src/ralph_orchestrator/validation/__init__.py` | Module init |
| `src/ralph_orchestrator/validation/base_validator.py` | Abstract base |
| `src/ralph_orchestrator/validation/phase_validators.py` | Phase-specific |
| `src/ralph_orchestrator/validation/evidence_checker.py` | Content analysis |
| `tests/integration/test_real_orchestration.py` | E2E tests |

## Files to Archive (Approach B)

| Directory | Destination |
|-----------|-------------|
| `ralph-mobile/` | `archive/ralph-mobile-v1/` |

---

# Unresolved Questions

1. **Claude CLI vs SDK**: Which method for spawning subagents? CLI is simpler but SDK offers more control.

2. **Human Approval Gate**: How should this work? CLI prompt? Web UI? Email approval?

3. **Integration Test Cost**: Real Claude calls cost money. How to handle in CI? Skip marker? Mock for CI only?

4. **Mobile Stakeholders**: Who wanted the mobile app? Need to communicate scope change.

5. **Evidence Retention**: How long to keep evidence files? Should they be in git or external storage?
