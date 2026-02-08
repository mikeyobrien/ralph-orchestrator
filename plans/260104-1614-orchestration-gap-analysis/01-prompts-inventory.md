# Prompts Inventory - Gap Analysis

**Date**: 2026-01-04
**Report**: 01-prompts-inventory.md
**Purpose**: Comprehensive analysis of all prompts in ralph-orchestrator

---

## Summary

| Category | Count | Status |
|----------|-------|--------|
| Active Prompts | 2 | COMPLETE (both) |
| Archived/Completed | 4 | COMPLETE |
| Support Prompts | 2 | ACTIVE |
| Total Tests | 105 | PASSING (orchestration) |
| Evidence Files | 53+ | PRESENT |

---

## Active Prompts

### 1. prompts/orchestration/PROMPT.md

**Claims**: Ralph Orchestrator Architecture Improvement (6 phases O0-O5)
**Status**: TASK_COMPLETE

**Phases:**
| Phase | Name | Status | Tests |
|-------|------|--------|-------|
| O0 | Run Isolation & State Management | VALIDATED | 18 |
| O1 | Subagent Types & Profiles | VALIDATED | 12 |
| O2 | Skill Discovery | VALIDATED | 14 |
| O3 | MCP Tool Discovery | VALIDATED | 17 |
| O4 | Coordination Protocol | VALIDATED | 22 |
| O5 | Integration & Subagent Spawning | VALIDATED | 22 |

**Acceptance Criteria File**: `COMPREHENSIVE_ACCEPTANCE_CRITERIA.yaml`

**Validation Evidence Location**: `validation-evidence/orchestration-00/` through `orchestration-05/`

**Evidence Files Found**:
- `orchestration-00/`: run-manager-create.txt, run-manager-tests.txt
- `orchestration-01/` through `orchestration-05/`: Empty (.gitkeep only)

**Gap Identified**:
- Only orchestration-00 has actual evidence files
- orchestration-01 through orchestration-05 directories are EMPTY
- Prompt claims all phases validated but evidence is missing

---

### 2. prompts/self-improvement/PROMPT.md

**Claims**: Ralph Orchestrator v2.0 - Subagent Orchestration (7 phases 00-06)
**Status**: TASK COMPLETE

**Phases:**
| Phase | Name | Status | Evidence |
|-------|------|--------|----------|
| 00 | TUI Verification | VALIDATED | 2 files |
| 01 | Process Isolation | VALIDATED | 2 files |
| 02 | Daemon Mode | VALIDATED | 5 files |
| 03 | REST API | VALIDATED | 3 files |
| 04 | Mobile Foundation | VALIDATED | 5 files |
| 05 | Mobile Dashboard | VALIDATED | 4 files |
| 06 | Mobile Control | VALIDATED | 2 files |

**Acceptance Criteria File**: `ACCEPTANCE_CRITERIA.yaml`

**Validation Evidence Location**: `prompts/self-improvement/validation-evidence/`

**Evidence Files Found**:
- `phase-00/`: tui-screenshot.png (1MB), tui-output.txt
- `phase-01/`: 2 files
- `phase-02/`: 5 files
- `phase-03/`: 3 files
- `phase-04/`: 5 files (expo-build.txt, simulator screenshots)
- `phase-05/`: 4 files
- `phase-06/`: 2 files
- `ios/`: 7 files (SwiftUI source + screenshots)
- `web/`: 9 files (HTML + Playwright screenshots)
- `cli/`: 2 files
- `final/summary.md`: Validation summary

**Gap Identified**: None - evidence appears complete

---

## Archived/Completed Prompts

### 3. prompts/archive/completed/WEB_PROMPT.md

**Claims**: Web UI for Ralph Orchestrator Monitoring (11 iterations)
**Status**: COMPLETE (September 8, 2024)
**Tests**: 73 tests passing

**Key Features Implemented**:
- FastAPI backend with WebSocket
- JWT authentication
- SQLite persistence
- Rate limiting
- Chart.js visualizations

**Acceptance Criteria**: All 12 requirements met
**Evidence**: Tests embedded in prompt document

**Gap Identified**: None - well documented completion

---

### 4. prompts/archive/completed/VALIDATION_FEATURE_PROMPT.md

**Claims**: User-Collaborative Validation Gate System
**Status**: COMPLETE (30+ verification passes)
**Tests**: 26 tests passing

**Key Features**:
- `enable_validation` parameter
- `_propose_validation_strategy()` method
- Three validation targets: iOS, Web, CLI

**Evidence Location**: `validation-evidence/{cli,ios,web}/`

**Evidence Files Found**:
- cli/: cli-output.txt, ralph_validator_cli.py
- ios/: 2 screenshots (~3.4MB each), 4 Swift files
- web/: 5 screenshots, index.html, Playwright test

**Gap Identified**: None - extensively verified (30+ times!)

---

### 5. prompts/archive/completed/ONBOARDING_PROMPT.md

**Claims**: Intelligent Project Onboarding & Pattern Analysis
**Status**: COMPLETE (26 verification iterations)
**Tests**: 171 tests passing

**Key Features**:
- `ralph onboard` CLI command
- SettingsLoader, ProjectScanner, AgentAnalyzer
- HistoryAnalyzer, PatternExtractor, ConfigGenerator

**Documentation**: `docs/guide/onboarding.md`

**Gap Identified**: None - extensively verified

---

### 6. prompts/archive/completed/VALIDATION_PROPOSAL_PROMPT.md

**Claims**: Validation Strategy Proposal (Session 0 phase)
**Status**: ACTIVE (used by validation system)

**Purpose**: AI proposes validation, user confirms before execution

**Gap Identified**: None - this is a supporting prompt

---

## Support/Utility Prompts

### 7. prompts/VALIDATION_PROPOSAL_PROMPT.md

**Purpose**: Top-level validation proposal prompt
**Status**: ACTIVE (in use)
**Content**: User-centric validation proposal flow

---

### 8. prompts/test-tui.md

**Purpose**: TUI testing prompt
**Status**: UTILITY
**Content**: Simple instruction to run `ralph tui`

---

## Critical Gap Analysis

### Gaps Found

| Prompt | Gap | Severity |
|--------|-----|----------|
| orchestration/PROMPT.md | Evidence dirs O1-O5 are EMPTY | HIGH |
| self-improvement/PROMPT.md | None | OK |
| archive/completed/* | None | OK |

### Detailed Gap: Orchestration Evidence Missing

**Location**: `validation-evidence/orchestration-*/`

| Directory | Expected Files | Actual |
|-----------|---------------|--------|
| orchestration-00 | run-manager-*.txt | PRESENT |
| orchestration-01 | profiles.txt, tests.txt | MISSING |
| orchestration-02 | discovery.txt, tests.txt | MISSING |
| orchestration-03 | mcps.txt, tests.txt | MISSING |
| orchestration-04 | coordination.txt, tests.txt | MISSING |
| orchestration-05 | integration.txt, tests.txt | MISSING |

**Impact**: The prompt claims all 6 phases are VALIDATED but 5 out of 6 evidence directories are empty.

**Possible Explanations**:
1. Evidence was stored in a different location (.agent/runs/)
2. Evidence was generated but not committed
3. Tests pass but real-execution evidence was skipped

---

## Test Coverage Verification

### Orchestration Tests (105 total)
```
tests/test_run_manager.py       - 18 tests
tests/test_orchestration_config.py - 12 tests
tests/test_discovery.py         - 31 tests
tests/test_coordinator.py       - 22 tests
tests/test_orchestration_integration.py - 22 tests
```

### Self-Improvement/Validation Tests
- 26 validation feature tests
- 171 onboarding tests

### Web Tests
- 73 web module tests

---

## Unresolved Questions

1. **Why are orchestration-01 through orchestration-05 evidence directories empty?**
   - The prompt shows phases as VALIDATED
   - Tests exist and pass (105 tests)
   - But real-execution evidence is missing

2. **Where is the .agent/ directory mentioned in orchestration/PROMPT.md?**
   - Prompt references `.agent/runs/{id}/validation-evidence/`
   - Need to check if this directory exists with evidence

3. **Are the mobile screenshots in self-improvement valid?**
   - Files exist (~100KB each)
   - Need visual verification that they show success states

4. **Was the orchestration prompt actually run to completion or just documented as complete?**
   - 105 tests pass
   - Evidence directories mostly empty
   - Possible gap between claimed and actual validation

---

## Recommendations

1. **HIGH**: Investigate missing orchestration evidence (O1-O5)
2. **MEDIUM**: Verify .agent/ directory structure and contents
3. **LOW**: Review self-improvement screenshots for content accuracy
4. **INFO**: Consider consolidating evidence locations to single standard

---

## File Paths Referenced

```
Active:
- prompts/orchestration/PROMPT.md
- prompts/orchestration/COMPREHENSIVE_ACCEPTANCE_CRITERIA.yaml
- prompts/self-improvement/PROMPT.md
- prompts/self-improvement/ACCEPTANCE_CRITERIA.yaml

Completed:
- prompts/archive/completed/WEB_PROMPT.md
- prompts/archive/completed/VALIDATION_FEATURE_PROMPT.md
- prompts/archive/completed/ONBOARDING_PROMPT.md
- prompts/archive/completed/VALIDATION_PROPOSAL_PROMPT.md

Evidence:
- validation-evidence/orchestration-00/ through orchestration-05/
- prompts/self-improvement/validation-evidence/phase-00/ through phase-06/
- validation-evidence/cli/, ios/, web/
```

---

**Report Complete**: 2026-01-04 16:15
