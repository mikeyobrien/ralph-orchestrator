# E2E Test Harness - Project Summary

## Overview

This document summarizes the Prompt-Driven Development process for designing an E2E test harness for Ralph orchestrator.

**Goal:** Create a manual validation tool that tests Ralph against real backends (Claude, Kiro, OpenCode) and generates agent-readable reports for continuous improvement.

---

## Artifacts Created

```
specs/e2e-testing/
├── rough-idea.md                    # Original request
├── idea-honing.md                   # Requirements Q&A (8 questions)
├── research/
│   ├── skill-verification-principles.md   # Writing-skills TDD methodology
│   ├── ralph-backends.md            # Backend capabilities matrix
│   ├── orchestration-loop.md        # Loop mechanics to validate
│   ├── existing-tests.md            # Current test coverage analysis
│   └── features-to-test.md          # Feature checklist
├── design/
│   └── detailed-design.md           # Comprehensive design (~800 lines)
├── implementation/
│   └── plan.md                      # 14-step implementation plan
└── summary.md                       # This document
```

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **New crate: `ralph-e2e`** | Separation of concerns, optional dependency |
| **Hardcoded Rust tests** | Simplicity over configuration |
| **Meta-Ralph analysis** | Dogfoods Ralph, self-improving feedback loop |
| **Writing-skills TDD** | RED/GREEN/REFACTOR phases for thorough validation |
| **Agent-readable reports** | Enables automated improvement cycles |
| **7 test tiers** | Progressive validation from connectivity to errors |

---

## Test Coverage

| Tier | Phase | Scenarios | Purpose |
|------|-------|-----------|---------|
| 1 | GREEN | 3 | Connectivity (Claude, Kiro, OpenCode) |
| 2 | GREEN | 5 | Orchestration loop |
| 3 | GREEN | 2 | Event system |
| 4 | GREEN | 2 | Capabilities (tools, streaming) |
| 5 | GREEN+Pressure | 7 | Hat collections |
| 6 | GREEN+Pressure | 6 | Memory system |
| 7 | RED | 4 | Error handling (baseline) |
| **Total** | | **29** | |

---

## Writing-Skills Principles Applied

1. **RED Phase (Baseline)** - Tier 7 documents failure behavior
2. **GREEN Phase (Compliance)** - Tiers 1-6 verify correct behavior
3. **REFACTOR Phase (Loopholes)** - Meta-Ralph identifies new failure modes
4. **Rationalization Tables** - Detect agent excuses vs reality
5. **One Excellent Test** - Deep scenarios with full context
6. **Pressure Testing** - Combined pressures (ambiguity, time, complexity)

---

## Report Features

### For Failed Tests:
- Full context (config, prompt, output, events)
- Failure type classification
- Root cause hypothesis with evidence
- Suggested investigations
- Potential fixes with confidence scores

### For Passed Tests:
- Quality score (Optimal/Good/Acceptable/Suboptimal)
- Metrics (duration, iterations, tool calls)
- Warnings (slow, excessive iterations, struggled)
- Optimization opportunities

### Global:
- Pass/fail verdict with exit code
- Recommendations prioritized by severity
- Loophole tracking for discovered edge cases
- Quick fix commands

---

## Implementation Highlights

**14 steps, incremental delivery:**

1. Crate scaffold + CLI
2. WorkspaceManager
3. AuthChecker + Backend detection
4. RalphExecutor
5. TestScenario trait + first scenario
6. TestRunner + basic reporting
7. Tier 1 scenarios (connectivity)
8. Tier 2 scenarios (orchestration)
9. **MetaRalphAnalyzer** (key feature)
10. Tier 5 scenarios (hats)
11. Tier 6 scenarios (memories)
12. Full Reporter (MD + JSON)
13. Remaining tiers
14. Polish + documentation

**Critical path:** 1 → 2 → 4 → 5 → 6 → 9 → 12

---

## Next Steps

1. **Review this design** - Ensure it meets requirements
2. **Create code task** - Use `/code-task-generator` to create implementation task
3. **Implement Step 1** - Create crate scaffold
4. **Iterate** - Each step produces working, demoable increment

---

## Success Criteria

The implementation is complete when:

- [x] Design document covers all requirements
- [x] Implementation plan has 14 incremental steps
- [ ] `ralph-e2e claude` runs all Claude scenarios
- [ ] `ralph-e2e all` runs scenarios for all backends
- [ ] Reports are agent-readable with full context
- [ ] Meta-Ralph analysis provides diagnosis and optimizations
- [ ] Writing-skills TDD principles applied
- [ ] All tests pass
- [ ] Documentation complete
