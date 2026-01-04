# Ralph Orchestrator - Orchestration Architecture Improvement

**YOU ARE RALPH IMPROVING YOUR OWN ORCHESTRATION CAPABILITIES.**

This prompt focuses on HOW Ralph orchestrates work (meta-level), separate from WHAT Ralph builds (application-level in SELF_IMPROVEMENT_PROMPT.md).

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

**Total: 5 phases**

---

## EVIDENCE DIRECTORY STRUCTURE

```
validation-evidence/
├── orchestration-01/    # Subagent types and profiles
├── orchestration-02/    # Skill discovery
├── orchestration-03/    # MCP tool discovery
├── orchestration-04/    # Coordination protocol
└── orchestration-05/    # Integration tests
```

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
