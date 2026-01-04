# Ralph Orchestrator - Orchestration Architecture Improvement

**YOU ARE RALPH IMPROVING YOUR OWN ORCHESTRATION CAPABILITIES.**

This prompt focuses on HOW Ralph orchestrates work (meta-level), separate from WHAT Ralph builds (application-level in SELF_IMPROVEMENT_PROMPT.md).

---

## COMPREHENSIVE VALIDATION PROPOSAL

### Scope Analysis

I have analyzed the complete prompt and identified:
- **Total Phases**: 6 (O0-O5)
- **Total Plans**: 12 (2 per phase)
- **Evidence Files Required**: 12+ (2 per phase minimum)
- **Dependencies**: Linear (O0 → O1 → O2 → O3 → O4 → O5)

### Phase Flow Diagram

```
O0: Run Isolation     → O1: Subagent Profiles → O2: Skill Discovery
        ↓                      ↓                      ↓
O3: MCP Discovery    ← ─ ─ ─ ─ ┘                      ↓
        ↓                                             ↓
O4: Coordination     ← ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘
        ↓
O5: Integration      → COMPLETE
```

### VALIDATION APPROACH: REAL EXECUTION ONLY

All validation uses:
- **Real Python imports** - Import actual modules, create real objects
- **CLI output capture** - Redirect output to validation-evidence/
- **Functional tests** - Tests that create real files and state
- **NO mock-only tests** - Unit tests supplement but don't replace real execution

### Phase-by-Phase Acceptance Criteria

| Phase | Name | Plans | Validation Method | Evidence Required |
|-------|------|-------|-------------------|-------------------|
| O0 | Run Isolation & State Management | 2 | Python import + file creation | run-manager-create.txt, tests.txt |
| O1 | Subagent Types & Profiles | 2 | Python import + profile listing | profiles.txt, tests.txt |
| O2 | Skill Discovery | 2 | Python import + skill scan | discovery.txt, tests.txt |
| O3 | MCP Tool Discovery | 2 | Python import + MCP listing | mcps.txt, tests.txt |
| O4 | Coordination Protocol | 2 | Python import + file creation | coordination.txt, tests.txt |
| O5 | Integration & Subagent Spawning | 3 | Full integration test | config.txt, orchestration-manager.txt, tests.txt |

### Global Success Criteria

- [ ] All 6 phases show `| ✅ VALIDATED` status
- [ ] All unit tests pass (`uv run pytest tests/test_orchestration*.py tests/test_run_manager.py tests/test_discovery.py tests/test_coordinator.py -v`)
- [ ] Orchestration package exists with all modules
- [ ] Integration test demonstrates subagent prompt generation
- [ ] Evidence files exist for all phases (12+ files total)

### Validation Strategy

**FORBIDDEN** (Unit tests with mocks only):
- `uv run pytest` alone without functional verification
- Mock-only tests that don't verify real behavior

**REQUIRED** (Real execution):
- Python imports that verify module structure
- Real file creation in validation-evidence/
- CLI output capture showing actual behavior
- Integration tests with real configuration

### Evidence Directory Structure

```
validation-evidence/
├── orchestration-00/           # Run Isolation
│   ├── run-manager-create.txt  # Run creation output
│   └── run-manager-tests.txt   # Test results
├── orchestration-01/           # Subagent Profiles
│   ├── profiles.txt            # Profile listing
│   └── tests.txt               # Test results
├── orchestration-02/           # Skill Discovery
│   ├── discovery.txt           # Discovered skills
│   └── tests.txt               # Test results
├── orchestration-03/           # MCP Discovery
│   ├── mcps.txt                # Discovered MCPs
│   └── tests.txt               # Test results
├── orchestration-04/           # Coordination
│   ├── coordination.txt        # Coordination files created
│   └── tests.txt               # Test results
└── orchestration-05/           # Integration
    ├── config.txt              # Config field verification
    ├── orchestration-manager.txt # Manager output
    └── tests.txt               # Test results
```

### Approval Required

**Do you approve this REAL EXECUTION validation plan?**
- [A]pprove - Proceed with functional validation
- [M]odify - I want to change something
- [S]kip - Skip validation, proceed without criteria

**Full acceptance criteria saved to:** `prompts/orchestration/COMPREHENSIVE_ACCEPTANCE_CRITERIA.yaml`

---

## ✅ VALIDATION PROPOSAL APPROVED

User approved the validation proposal. Proceeding with Phase O0.

---

## VALIDATION-FIRST WORKFLOW

For each phase, use the same validation-first approach:

```
1. ATTEMPT VALIDATION FIRST
   - Run the validation commands/tests
   - Capture evidence to validation-evidence/orchestration-XX/

2. CHECK RESULTS
   - If validation PASSES → Mark phase complete, move to next
   - If validation FAILS → Implement the feature

3. AFTER IMPLEMENTATION
   - Re-run validation
   - Ensure fresh evidence captured
   - Only proceed when validation passes
```

---

## PROJECT OVERVIEW

Improve Ralph's orchestration architecture to enable:
1. **Parallel subagent execution** - Spawn specialized agents that work together
2. **Skill discovery** - Dynamically find and inject relevant skills into subagents
3. **MCP tool profiling** - Give each subagent only the tools they need
4. **Coordination protocol** - Shared files for subagent communication
5. **Context optimization** - Maximize effectiveness, not minimize tokens

**Total: 6 phases (O0-O5)**

---

## EVIDENCE DIRECTORY STRUCTURE

Evidence is stored in `.agent/` for run isolation:

```
.agent/
├── runs/
│   └── {run-id}/                    # Unique per run
│       ├── manifest.json            # Prompt, criteria, status
│       ├── validation-evidence/
│       │   └── orchestration-XX/
│       ├── coordination/
│       └── metrics/
└── prompts/
    └── ORCHESTRATION_IMPROVEMENT/   # Per-prompt state
        └── latest-run-id            # Pointer to latest run
```

**Total: 6 phases (O0-O5)**

---

## Phase O0: Run Isolation & State Management | ⏳ NEEDS_VALIDATION

### What To Build

Create run isolation infrastructure so each prompt execution is:
- **ID-based** - Unique run identifier
- **Traceable** - Know what prompt/criteria was used
- **Resumable** - Can start over or continue from any point
- **Self-contained** - All state in `.agent/runs/{id}/`

### Acceptance Criteria

- [ ] RunManager class in src/ralph_orchestrator/run_manager.py
- [ ] create_run(prompt_path) → returns run_id, creates directory structure
- [ ] get_run(run_id) → returns RunInfo with manifest
- [ ] get_latest_run(prompt_name) → returns most recent run for prompt
- [ ] Manifest includes: prompt_path, started_at, criteria, status
- [ ] Validation evidence stored in .agent/runs/{id}/validation-evidence/
- [ ] Unit tests pass

### Validation Gate

```bash
# Test run management
uv run python -c "
from ralph_orchestrator.run_manager import RunManager
rm = RunManager()
run_id = rm.create_run('prompts/ORCHESTRATION_IMPROVEMENT_PROMPT.md')
print(f'Created run: {run_id}')
run_info = rm.get_run(run_id)
print(f'Manifest: {run_info.manifest}')
print(f'Evidence dir: {run_info.evidence_dir}')
" > .agent/runs/test/validation-evidence/orchestration-00/run-manager.txt 2>&1

uv run pytest tests/test_run_manager.py -v > .agent/runs/test/validation-evidence/orchestration-00/tests.txt 2>&1
```

### Evidence Required
- `.agent/runs/{id}/validation-evidence/orchestration-00/run-manager.txt` - Run creation output
- `.agent/runs/{id}/validation-evidence/orchestration-00/tests.txt` - Test results (all pass)

---

## Phase O1: Subagent Types & Profiles | ⏳ NEEDS_VALIDATION

### What To Build

Define specialized subagent types with specific capabilities:

| Subagent | Purpose | Skills | MCP Tools |
|----------|---------|--------|-----------|
| VALIDATOR | Check acceptance criteria | playwright-skill, systematic-debugging | sequential-thinking, playwright |
| RESEARCHER | Find solutions, past patterns | mem-search, deep-research | claude-mem, tavily, Context7 |
| IMPLEMENTER | Write/modify code | test-driven-development, backend-development | (standard tools) |
| ANALYST | Root cause analysis | systematic-debugging, error-recovery | sequential-thinking |

### Acceptance Criteria

- [ ] SubagentProfile dataclass exists in src/ralph_orchestrator/orchestration/config.py
- [ ] Default profiles for all 4 subagent types defined
- [ ] Each profile specifies: name, description, required_tools, required_mcps, optional_mcps, prompt_template
- [ ] Unit tests pass for profile creation and validation

### Validation Gate

```bash
# Test profile configuration
uv run python -c "
from ralph_orchestrator.orchestration.config import SubagentProfile, SUBAGENT_PROFILES
print('Subagent profiles defined:', list(SUBAGENT_PROFILES.keys()))
for name, profile in SUBAGENT_PROFILES.items():
    print(f'  {name}: {len(profile.required_tools)} tools, {len(profile.required_mcps)} MCPs')
" > validation-evidence/orchestration-01/profiles.txt 2>&1

# Run unit tests
uv run pytest tests/test_orchestration_config.py -v > validation-evidence/orchestration-01/tests.txt 2>&1
```

### Evidence Required
- `validation-evidence/orchestration-01/profiles.txt` - Profile listing
- `validation-evidence/orchestration-01/tests.txt` - Test results (all pass)

---

## Phase O2: Skill Discovery | ⏳ NEEDS_VALIDATION

### What To Build

Create a skill discovery mechanism that:
1. Scans ~/.claude/skills/ for available skills
2. Reads SKILL.md files to extract descriptions
3. Builds skill index: {name: path, description, when_to_use}
4. Maps skills to subagent types

### Acceptance Criteria

- [ ] discover_skills() function exists in src/ralph_orchestrator/orchestration/discovery.py
- [ ] Returns dict of skill_name → SkillInfo objects
- [ ] SkillInfo contains: path, description, subagent_types (which subagents should use it)
- [ ] get_skills_for_subagent(subagent_type) returns relevant skills
- [ ] Unit tests pass

### Validation Gate

```bash
# Test skill discovery
uv run python -c "
from ralph_orchestrator.orchestration.discovery import discover_skills, get_skills_for_subagent
skills = discover_skills()
print(f'Discovered {len(skills)} skills')
for subagent_type in ['validator', 'researcher', 'implementer', 'analyst']:
    relevant = get_skills_for_subagent(subagent_type)
    print(f'{subagent_type}: {len(relevant)} relevant skills')
" > validation-evidence/orchestration-02/discovery.txt 2>&1

uv run pytest tests/test_discovery.py -v > validation-evidence/orchestration-02/tests.txt 2>&1
```

### Evidence Required
- `validation-evidence/orchestration-02/discovery.txt` - Discovery output
- `validation-evidence/orchestration-02/tests.txt` - Test results (all pass)

---

## Phase O3: MCP Tool Discovery | ⏳ NEEDS_VALIDATION

### What To Build

Create MCP discovery mechanism that:
1. Reads ~/.claude.json for available MCP servers
2. Checks project-level disabledMcpServers
3. Maps MCPs to subagent types
4. Returns available MCPs for each subagent type

### Acceptance Criteria

- [ ] discover_mcps() function exists in src/ralph_orchestrator/orchestration/discovery.py
- [ ] Returns dict of mcp_name → MCPInfo objects
- [ ] MCPInfo contains: name, command, enabled, tools
- [ ] get_mcps_for_subagent(subagent_type) returns relevant MCPs
- [ ] Handles missing/disabled MCPs gracefully
- [ ] Unit tests pass

### Validation Gate

```bash
# Test MCP discovery
uv run python -c "
from ralph_orchestrator.orchestration.discovery import discover_mcps, get_mcps_for_subagent
mcps = discover_mcps()
print(f'Discovered {len(mcps)} MCP servers')
for name, info in list(mcps.items())[:5]:
    print(f'  {name}: enabled={info.enabled}')
" > validation-evidence/orchestration-03/mcps.txt 2>&1

uv run pytest tests/test_discovery.py::test_mcp_discovery -v > validation-evidence/orchestration-03/tests.txt 2>&1
```

### Evidence Required
- `validation-evidence/orchestration-03/mcps.txt` - MCP listing
- `validation-evidence/orchestration-03/tests.txt` - Test results (all pass)

---

## Phase O4: Coordination Protocol | ⏳ NEEDS_VALIDATION

### What To Build

Implement coordination via shared files:

```
.agent/coordination/
├── current-attempt.json       # Attempt metadata
├── shared-context.md          # Common understanding
├── attempt-journal.md         # Full history
└── subagent-results/
    ├── validator-001.json
    ├── researcher-001.json
    └── implementer-001.json
```

### Acceptance Criteria

- [ ] CoordinationManager class in src/ralph_orchestrator/orchestration/coordinator.py
- [ ] init_coordination() creates directory structure
- [ ] write_attempt_start() creates current-attempt.json
- [ ] write_shared_context() updates shared-context.md
- [ ] collect_results() reads all subagent-results/*.json
- [ ] append_to_journal() updates attempt-journal.md
- [ ] Unit tests pass

### Validation Gate

```bash
# Test coordination
uv run python -c "
from ralph_orchestrator.orchestration.coordinator import CoordinationManager
cm = CoordinationManager()
cm.init_coordination()
cm.write_attempt_start(1, 'phase-test', ['Test acceptance'])
cm.write_shared_context({'phase': 'test', 'criteria': ['Test']})
print('Coordination files created:')
import os
for f in os.listdir('.agent/coordination'):
    print(f'  {f}')
" > validation-evidence/orchestration-04/coordination.txt 2>&1

uv run pytest tests/test_coordinator.py -v > validation-evidence/orchestration-04/tests.txt 2>&1
```

### Evidence Required
- `validation-evidence/orchestration-04/coordination.txt` - Coordination output
- `validation-evidence/orchestration-04/tests.txt` - Test results (all pass)

---

## Phase O5: Integration & Subagent Spawning | ⏳ NEEDS_VALIDATION

### What To Build

Integration with Ralph's main loop:

1. Add `enable_orchestration: bool` to RalphConfig
2. When enabled, spawn subagents via Task tool
3. Generate subagent prompts with:
   - Skill loading instructions
   - MCP tool restrictions
   - Coordination file paths
   - Output format requirements
4. Aggregate subagent results
5. Decide PASS/FAIL based on verdicts

### Acceptance Criteria

- [ ] enable_orchestration field in RalphConfig
- [ ] OrchestrationManager class orchestrates subagent workflow
- [ ] generate_subagent_prompt() creates prompt with skills/MCPs
- [ ] spawn_subagents() dispatches via Task tool (or equivalent)
- [ ] aggregate_results() combines subagent outputs
- [ ] Integration test passes with mock subagents

### Validation Gate

```bash
# Test orchestration integration
uv run python -c "
from ralph_orchestrator.main import RalphConfig
config = RalphConfig(enable_orchestration=True)
print(f'enable_orchestration: {config.enable_orchestration}')

from ralph_orchestrator.orchestration import OrchestrationManager
om = OrchestrationManager(config)
prompt = om.generate_subagent_prompt('validator', 'Test phase', ['Criterion 1'])
print(f'Generated prompt length: {len(prompt)} chars')
print(f'Contains skill instructions: {\"Skill(\" in prompt}')
" > validation-evidence/orchestration-05/integration.txt 2>&1

uv run pytest tests/test_orchestration_integration.py -v > validation-evidence/orchestration-05/tests.txt 2>&1
```

### Evidence Required
- `validation-evidence/orchestration-05/integration.txt` - Integration output
- `validation-evidence/orchestration-05/tests.txt` - Test results (all pass)

---

## COMPLETION CRITERIA

**DO NOT write TASK_COMPLETE until:**

1. All phases show `| ✅ VALIDATED` status
2. All unit tests pass
3. Integration test demonstrates subagent prompt generation
4. Evidence files are fresh and show success

---

## ARCHITECTURE NOTES

### Subagent Prompts Should Include

```markdown
## SUBAGENT: {type}

You are a specialized {type} subagent.

### Skills to Load
{skill_instructions}

### MCP Tools Available
{mcp_list}

### Coordination Files
- Read: .agent/coordination/shared-context.md
- Write: .agent/coordination/subagent-results/{type}-{id}.json

### Task
{task_description}

### Output Format
Write results to coordination file as JSON:
{output_schema}
```

### Key Principles

1. **Opus for all subagents** - Quality over cost
2. **sequential-thinking REQUIRED** - Every subagent uses structured reasoning
3. **Full context** - Maximize tokens for effectiveness
4. **Parallel by default** - Spawn multiple subagents simultaneously
5. **Skills before action** - Load relevant skills FIRST

---

**BEGIN ORCHESTRATION IMPROVEMENT**

Start by validating Phase O1 (Subagent Types & Profiles).
