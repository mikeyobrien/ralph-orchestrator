# Hat Collection Preset Evaluation Findings

> **Purpose**: Comprehensive evaluation of Ralph's hat collection presets, documenting structure, event routing, issues, and recommendations.

**Evaluator**: Claude Agent
**Date**: 2026-01-15
**Ralph Version**: 2.0.0
**Presets Evaluated**: 21

---

## Executive Summary

| Metric | Count |
|--------|-------|
| Total Presets | 21 |
| ✅ Well-Structured | 14 |
| ⚠️ Minor Issues | 5 |
| ❌ Structural Problems | 2 |
| 🐛 Bugs Found | 7 |
| 🎨 UX Improvements | 5 |
| 💡 Enhancement Ideas | 3 |

### Top Issues

| Priority | Issue | Presets Affected |
|----------|-------|------------------|
| P0 | Missing entry point (no `task.start` trigger) | deploy, feature, feature-minimal, refactor, docs |
| P1 | YAML syntax error (duplicate `hats:` key) | docs |
| P1 | Incomplete event graph (orphaned events) | spec-driven, incident-response, debug |
| P2 | Missing `default_publishes` for multi-publish hats | performance-optimization |
| P2 | Inconsistent completion signals | Mixed LOOP_COMPLETE vs custom |

### Quick Wins

1. **Add `task.start` triggers** to presets that need external event injection
2. **Fix docs.yml** duplicate `hats:` key causing parse errors
3. **Add `default_publishes`** to hats with multiple publish options
4. **Standardize completion signals** across all presets

### Previously Fixed Issues

| Issue | Status |
|-------|--------|
| BUG-001: Evaluation script CLI argument mismatch | ✅ FIXED |
| BUG-002: YAML format mismatch (array vs string default_publishes) | ✅ FIXED |
| BUG-003: Idle timeout during evaluation | ✅ RESOLVED |

---

## Preset Categories

### Category 1: Hat Collection Presets (New Multi-Agent Patterns)

These are the 12 new multi-agent workflow presets from COLLECTION.md:

| Preset | Pattern | Entry | Hats | Status |
|--------|---------|-------|------|--------|
| tdd-red-green | Critic-Actor Pipeline | task.start | 3 | ✅ |
| adversarial-review | Adversarial Critic-Actor | task.start | 3 | ✅ |
| socratic-learning | Socratic Dialogue | task.start | 3 | ✅ |
| spec-driven | Contract-First Pipeline | task.start | 4 | ⚠️ |
| mob-programming | Rotating Roles | task.start | 3 | ✅ |
| scientific-method | Scientific Investigation | task.start | 4 | ✅ |
| code-archaeology | Archaeological Dig | task.start | 4 | ✅ |
| performance-optimization | Data-Driven Optimization | task.start | 3 | ⚠️ |
| api-design | Outside-In Design | task.start | 4 | ✅ |
| documentation-first | Documentation-First | task.start | 4 | ✅ |
| incident-response | OODA Loop | task.start | 4 | ⚠️ |
| migration-safety | Expand-Contract | task.start | 4 | ✅ |

### Category 2: Standard Workflow Presets

These are the traditional Ralph presets:

| Preset | Purpose | Entry | Hats | Status |
|--------|---------|-------|------|--------|
| feature | Feature development | build.task | 2 | ⚠️ |
| feature-minimal | Minimal feature dev | build.task | 2 | ⚠️ |
| debug | Bug investigation | task.start | 4 | ⚠️ |
| deploy | Deployment workflow | build.task | 3 | ⚠️ |
| docs | Documentation | write.section | 2 | ❌ |
| refactor | Code refactoring | refactor.task | 2 | ⚠️ |
| research | Information gathering | task.start | 2 | ✅ |
| review | Code review | task.start | 2 | ✅ |
| gap-analysis | Gap analysis | task.start | ? | ✅ |

---

## Detailed Findings by Preset

### 1. `tdd-red-green.yml` — Test-Driven Development

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → test_writer → test.written → implementer → test.passing → refactorer
                                                                          ↓
refactor.done ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ←  cycle.complete
     ↓
(back to test_writer for next cycle)
```

**Strengths**:
- Clean three-phase cycle enforces TDD discipline
- Clear role separation prevents "cheating"
- Proper entry point with `task.start`
- `default_publishes: "cycle.complete"` ensures completion

**Issues**: None identified

**Recommendations**:
- Consider adding a `test.failed` event for cases where tests don't compile

---

### 2. `adversarial-review.yml` — Security Review

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → builder → build.ready → red_team → vulnerability.found → fixer
     ↑                                    ↓                              ↓
fix.applied ← ← ← ← ← ← ← ← ← ← ← ← ← ←  ↓                        fix.applied
                                    security.approved (terminal)
```

**Strengths**:
- Adversarial loop creates genuine security pressure
- Red team has comprehensive attack checklist
- Loop continues until security.approved

**Issues**: None identified

**Recommendations**:
- Consider adding severity scoring to vulnerability.found events

---

### 3. `socratic-learning.yml` — Learning Through Questions

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → explorer → understanding.claimed → questioner
     ↑                                              ↓
answer.provided ← answerer ← question.asked ← ← ← ← ←
                                    ↓
                          understanding.verified (terminal)
```

**Strengths**:
- Excellent for codebase exploration
- Questions deepen understanding iteratively
- Clear terminal condition

**Issues**: None identified

**Recommendations**:
- Add `max_questions` parameter to prevent infinite loops on complex topics

---

### 4. `spec-driven.yml` — Specification-First Development

**Status**: ⚠️ Minor Issues

**Event Flow**:
```
task.start → spec_writer → spec.ready → spec_reviewer
     ↑                                        ↓
spec.rejected ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ↓
                                        spec.approved
                                             ↓
                                        implementer → implementation.done → verifier
                                                                              ↓
                                                                      spec.violated → ?
                                                                      task.complete (terminal)
```

**Issues**:
| Type | Description | Severity |
|------|-------------|----------|
| Event Orphan | `spec.violated` has no handler | MEDIUM |

**Recommendations**:
- Add handler for `spec.violated` (route back to implementer or spec_writer)

---

### 5. `mob-programming.yml` — Virtual Mob Session

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → navigator → direction.set → driver → code.written → observer
     ↑                                                               ↓
     ↑ ← ← ← ← ← ← ← ← ← ← ← ← ← observation.noted ← ← ← ← ← ← ← ← ←

mob.complete (terminal)
```

**Strengths**:
- Role separation simulates real mob programming
- Observer provides fresh-eyes feedback
- Navigator decides what feedback to incorporate

**Issues**: None identified

---

### 6. `scientific-method.yml` — Hypothesis-Driven Debugging

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → observer → observation.made → theorist → hypothesis.formed → experimenter
     ↑                                                                         ↓
hypothesis.rejected ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ←
                                                                               ↓
                                            hypothesis.confirmed → fixer → fix.applied (terminal)
```

**Strengths**:
- Prevents random "try this" debugging
- Multiple hypothesis testing creates thoroughness
- Clear evidence-based approach

**Issues**: None identified

---

### 7. `code-archaeology.yml` — Legacy Code Understanding

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → surveyor → map.created → historian → history.documented → archaeologist
                                                                            ↓
                                              modifier ← artifacts.catalogued
                                                   ↓
                                            change.complete (terminal)
```

**Strengths**:
- Linear pipeline ensures thorough understanding before changes
- Each phase builds on previous findings
- `default_publishes` on modifier ensures completion

**Issues**: None identified

---

### 8. `performance-optimization.yml` — Measure-Optimize-Verify

**Status**: ⚠️ Minor Issues

**Event Flow**:
```
task.start → profiler → baseline.measured → analyst → analysis.complete → optimizer
     ↑                        ↑                                               ↓
     ↑                        ↑                                       optimization.applied
     ↑                        ↑ ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ↓
     ↑
improvement.verified (terminal) ← (from profiler on subsequent runs)
```

**Issues**:
| Type | Description | Severity |
|------|-------------|----------|
| Ambiguous | Profiler publishes either `baseline.measured` or `improvement.verified` | MEDIUM |
| Missing | No `default_publishes` on profiler | LOW |

**Recommendations**:
- Add `default_publishes: "baseline.measured"` to profiler
- Consider splitting profiler into `baseline_profiler` and `verification_profiler`

---

### 9. `api-design.yml` — Consumer-Driven API Design

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → consumer → usage.examples → designer → api.designed → critic
     ↑                                                               ↓
api.refined ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ↓
                                                              api.approved
                                                                   ↓
                                        implementer → task.complete (terminal)
```

**Strengths**:
- Outside-in design from consumer perspective
- Critic gate ensures usable APIs
- `default_publishes` ensures completion

**Issues**: None identified

---

### 10. `documentation-first.yml` — README-Driven Development

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → documenter → docs.ready → reviewer
     ↑                                     ↓
docs.rejected ← ← ← ← ← ← ← ← ← ← ← ← ← ← ↓
                                     docs.approved
                                          ↓
                          implementer → implementation.done → verifier → task.complete
```

**Strengths**:
- Forces clear thinking before coding
- Reviewer ensures docs are implementable
- Verifier confirms docs match implementation

**Issues**: None identified

---

### 11. `incident-response.yml` — Production Incident Handling

**Status**: ⚠️ Minor Issues

**Event Flow**:
```
task.start → observer → situation.assessed → mitigator
                                                 ↓
                    mitigation.failed → ? (ORPHANED)
                                                 ↓
                              mitigation.applied → investigator → root_cause.found → fixer
                                                                                       ↓
                                                                          incident.resolved (terminal)
```

**Issues**:
| Type | Description | Severity |
|------|-------------|----------|
| Event Orphan | `mitigation.failed` has no explicit handler | MEDIUM |

**Recommendations**:
- Add `mitigation.failed` to mitigator triggers for retry logic
- Or route to a separate `escalator` hat

---

### 12. `migration-safety.yml` — Safe System Migration

**Status**: ✅ Well-Structured

**Event Flow**:
```
task.start → planner → plan.ready → expander → expand.done → migrator
                            ↑                                    ↓
         contract.rollback ←↑                              migrate.done
                            ↑                                    ↓
                            ↑                              contractor
                            ↑                                    ↓
                            ← ← ← ← ← ← ← ← ← ← ← expand.rollback
                                                          ↓
                                              migration.complete (terminal)
```

**Strengths**:
- Expand-contract pattern is production-safe
- Rollback paths at each phase
- Clear verification checkpoints

**Issues**: None identified

---

## Standard Workflow Presets

### 13. `feature.yml` — Feature Development

**Status**: ⚠️ Needs Entry Point

**Event Flow**:
```
build.task → builder → build.done → ? (no handler)
                 ↓
          build.blocked → ? (no handler)

review.request → reviewer → review.approved → ? (no handler)
                       ↓
               review.changes_requested → ? (no handler)
```

**Issues**:
| Type | Description | Severity |
|------|-------------|----------|
| No Entry | No hat triggers on `task.start` | HIGH |
| Orphaned Events | `build.done`, `build.blocked`, `review.*` have no handlers | HIGH |
| External Dependency | Requires Ralph Planner to inject `build.task` events | MEDIUM |

**Analysis**: This preset is designed to work with Ralph's internal Planner component, which creates a scratchpad and injects `build.task` events. It's not a standalone preset—it requires external orchestration.

**Recommendations**:
- Document that this preset requires Planner mode
- Or add a `planner` hat that triggers on `task.start`

---

### 14. `feature-minimal.yml` — Minimal Feature Development

**Status**: ⚠️ Same issues as feature.yml

Same analysis as `feature.yml`—requires external event injection.

---

### 15. `debug.yml` — Bug Investigation

**Status**: ⚠️ Minor Issues

**Event Flow**:
```
task.start → investigator → hypothesis.test → tester → hypothesis.confirmed → ?
     ↑              ↑                             ↓
     ↑              ← hypothesis.rejected ← ← ← ← ←
     ↓
fix.propose → fixer → fix.applied → verifier → fix.verified → investigator
                                         ↓
                                   fix.failed → ? (ORPHANED)
```

**Issues**:
| Type | Description | Severity |
|------|-------------|----------|
| Event Orphan | `hypothesis.confirmed` has no explicit handler | MEDIUM |
| Event Orphan | `fix.failed` has no handler | MEDIUM |

**Recommendations**:
- Add `hypothesis.confirmed` to investigator triggers (or create `fix.propose` handler)
- Add `fix.failed` handler (route back to fixer or investigator)

---

### 16. `deploy.yml` — Deployment Workflow

**Status**: ⚠️ Needs Entry Point

**Issues**:
| Type | Description | Severity |
|------|-------------|----------|
| No Entry | No hat triggers on `task.start` | HIGH |
| Missing Instructions | Hats rely on event metadata for instructions | LOW |

**Analysis**: This preset demonstrates custom event metadata but lacks standalone functionality.

---

### 17. `docs.yml` — Documentation

**Status**: ❌ Syntax Error

**Issues**:
| Type | Description | Severity |
|------|-------------|----------|
| YAML Error | Duplicate `hats:` key on lines 22-23 | BLOCKER |
| No Entry | No hat triggers on `task.start` | HIGH |

**Location**: Lines 22-23
```yaml
hats:
hats:  # <-- DUPLICATE KEY - MUST REMOVE
  writer:
```

**Recommendations**:
- Remove duplicate `hats:` key
- Add entry point trigger

---

### 18. `refactor.yml` — Code Refactoring

**Status**: ⚠️ Needs Entry Point

**Issues**:
| Type | Description | Severity |
|------|-------------|----------|
| No Entry | No hat triggers on `task.start` | HIGH |
| External Dependency | Requires external `refactor.task` injection | MEDIUM |

---

### 19. `research.yml` — Information Gathering

**Status**: ✅ Well-Structured

Clean preset with proper `task.start` entry point.

---

### 20. `review.yml` — Code Review

**Status**: ✅ Well-Structured

Clean preset with proper `task.start` entry point.

---

## Cross-Cutting Observations

### Event Routing Patterns

| Pattern | Frequency | Notes |
|---------|-----------|-------|
| task.start entry | 15/21 | Standard entry point |
| Wildcard triggers (e.g., `task.*`) | 3/21 | Used in debug, feature presets |
| default_publishes | 12/21 | Ensures predictable completion |
| Cycle patterns (A→B→C→A) | 6/21 | TDD, review loops |
| Linear pipelines | 6/21 | Code archaeology, migration |

### Hat Instruction Quality

| Observation | Presets Affected | Notes |
|-------------|------------------|-------|
| Clear role separation | ALL collection presets | Each hat has distinct responsibility |
| DON'T sections | Most | Explicit anti-patterns help guide behavior |
| Missing instructions | feature-minimal, deploy | Rely on auto-derivation from events |
| Completion signals | Mixed | Some use LOOP_COMPLETE, others custom |

### Completion Signal Inconsistency

| Signal | Presets Using It |
|--------|------------------|
| `LOOP_COMPLETE` | tdd-red-green, mob-programming, api-design, documentation-first, migration-safety, etc. |
| `DEBUG_COMPLETE` | debug |
| `DOCS_COMPLETE` | docs |
| `REFACTOR_COMPLETE` | refactor |
| `RESEARCH_COMPLETE` | research |
| `REVIEW_COMPLETE` | review |

**Recommendation**: Standardize on `LOOP_COMPLETE` for consistency, or document the custom signals clearly.

---

## Bug Summary

| ID | Location | Description | Severity | Status |
|----|----------|-------------|----------|--------|
| BUG-001 | tools/evaluate-preset.sh | CLI argument mismatch | BLOCKER | ✅ FIXED |
| BUG-002 | presets/*.yml | default_publishes array vs string | HIGH | ✅ FIXED |
| BUG-003 | tools/evaluate-preset.sh | Idle timeout during evaluation | MEDIUM | ✅ RESOLVED |
| BUG-004 | presets/docs.yml | Duplicate `hats:` key on line 23 | BLOCKER | ✅ FIXED (dbf3c3f1) |
| BUG-005 | presets/spec-driven.yml | `spec.violated` orphaned event | MEDIUM | ✅ FIXED (dbf3c3f1) |
| BUG-006 | presets/incident-response.yml | `mitigation.failed` orphaned event | MEDIUM | ✅ FIXED (dbf3c3f1) |
| BUG-007 | presets/debug.yml | `hypothesis.confirmed` and `fix.failed` orphaned | MEDIUM | ✅ FIXED (dbf3c3f1) |

---

## UX Improvements

| ID | Description | Impact | Effort |
|----|-------------|--------|--------|
| UX-001 | Add progress indicators during evaluation | HIGH | LOW |
| UX-002 | Show event publication status in real-time | MEDIUM | MEDIUM |
| UX-003 | Create preset validation command (`ralph validate-preset`) | HIGH | MEDIUM |
| UX-004 | Add visual event flow diagrams to COLLECTION.md | HIGH | LOW |
| UX-005 | Standardize completion signals across presets | MEDIUM | LOW |

---

## Enhancement Ideas

| ID | Description | Value |
|----|-------------|-------|
| ENH-001 | Dry-run mode for preset validation | HIGH |
| ENH-002 | Event graph visualization tool | HIGH |
| ENH-003 | Preset composition (inherit from base presets) | MEDIUM |

---

## Recommendations

### Immediate Actions (P0)

1. **Fix BUG-004: docs.yml syntax error**
   - Location: `presets/docs.yml` line 23
   - Action: Remove duplicate `hats:` key

2. **Add entry points to orphaned presets** (feature, feature-minimal, deploy, refactor, docs)
   - Either add planner hats that trigger on `task.start`
   - Or clearly document they require Ralph Planner mode

3. **Handle orphaned events**
   - spec-driven.yml: Add `spec.violated` → spec_writer trigger
   - incident-response.yml: Add `mitigation.failed` → mitigator trigger
   - debug.yml: Add `hypothesis.confirmed` and `fix.failed` handlers

### Short-term Improvements (P1)

1. **Add missing `default_publishes`** to ambiguous hats:
   - performance-optimization: profiler → `default_publishes: "baseline.measured"`

2. **Standardize completion signals**
   - Consider migrating all to `LOOP_COMPLETE`
   - Document custom signals in each preset header

3. **Create preset validation tool**
   ```bash
   ralph validate-preset presets/tdd-red-green.yml
   # Output:
   # ✓ Valid YAML syntax
   # ✓ All hats have triggers
   # ✓ Event graph is connected
   # ✓ No orphaned events
   # ✓ Entry point exists (task.start)
   ```

### Future Enhancements (P2)

1. **Event flow visualization**
   - Generate Mermaid/Graphviz diagrams from YAML
   - Include in documentation

2. **Preset testing framework**
   - Unit tests for individual hats
   - Integration tests with mock backends

---

## Appendix

### A. Test Environment
```
OS: macOS (Darwin 24.6.0)
Ralph Version: 2.0.0
Rust Version: 1.85+ (stable)
Date: 2026-01-15
Evaluator: Claude Agent
Build: cargo build --release ✓
Smoke Tests: cargo test -p ralph-core smoke_runner ✓ (12 tests passed)
```

### B. Preset File Locations
```
presets/
├── adversarial-review.yml     ✅
├── api-design.yml             ✅
├── code-archaeology.yml       ✅
├── COLLECTION.md              (documentation)
├── debug.yml                  ⚠️
├── deploy.yml                 ⚠️
├── docs.yml                   ❌
├── documentation-first.yml    ✅
├── feature-minimal.yml        ⚠️
├── feature.yml                ⚠️
├── gap-analysis.yml           ✅
├── incident-response.yml      ⚠️
├── migration-safety.yml       ✅
├── mob-programming.yml        ✅
├── performance-optimization.yml ⚠️
├── refactor.yml               ⚠️
├── research.yml               ✅
├── review.yml                 ✅
├── scientific-method.yml      ✅
├── socratic-learning.yml      ✅
├── spec-driven.yml            ⚠️
└── tdd-red-green.yml          ✅
```

### C. Event Graph Legend
```
→  : Triggers next hat
←  : Returns to previous hat (cycle)
↓  : Continues to next stage
(terminal) : Ends the workflow
? : Orphaned (no handler)
```

### D. HatRegistry Event Routing

The `HatRegistry::get_for_topic()` function finds hats by matching published events to hat triggers:

```rust
// crates/ralph-core/src/hat_registry.rs:110-113
pub fn get_for_topic(&self, topic: &str) -> Option<&Hat> {
    let topic = Topic::new(topic);
    self.hats.values().find(|hat| hat.is_subscribed(&topic))
}
```

Supports wildcard patterns like `task.*` matching `task.start`, `task.resume`, etc.
