# Orchestration Test Coverage Analysis

## Source Modules (src/ralph_orchestrator/orchestration/)

| Module | Lines | Purpose |
|--------|-------|---------|
| `__init__.py` | 32 | Package exports |
| `config.py` | 225 | SubagentProfile dataclass + SUBAGENT_PROFILES constant |
| `coordinator.py` | 182 | CoordinationManager - file-based coordination protocol |
| `discovery.py` | 313 | SkillInfo/MCPInfo + discover_skills/discover_mcps |
| `manager.py` | 236 | OrchestrationManager - prompt generation + result aggregation |

## Test Files Covering Orchestration

| Test File | Target | Tests |
|-----------|--------|-------|
| `test_orchestration_config.py` | config.py | 11 tests |
| `test_orchestration_integration.py` | manager.py | 17 tests |
| `test_discovery.py` | discovery.py | 27 tests |
| `test_coordinator.py` | coordinator.py | 22 tests |

## Functions/Classes TESTED

### config.py
- [x] SubagentProfile dataclass creation
- [x] SubagentProfile field validation (name, description, required_tools, required_mcps, optional_mcps, prompt_template)
- [x] SubagentProfile default values (optional_mcps=[])
- [x] SubagentProfile serialization to dict
- [x] SUBAGENT_PROFILES contains validator, researcher, implementer, analyst
- [x] Each profile has non-empty prompt_template

### coordinator.py (CoordinationManager)
- [x] `__init__()` with base_dir
- [x] `init_coordination()` - creates .agent/coordination/ structure
- [x] `write_attempt_start()` - creates current-attempt.json
- [x] `get_current_attempt()` - reads attempt metadata
- [x] `write_shared_context()` - creates shared-context.md
- [x] `get_shared_context()` - reads shared context
- [x] `write_subagent_result()` - writes subagent result file
- [x] `collect_results()` - collects all result JSON files
- [x] `clear_subagent_results()` - removes result files
- [x] `append_to_journal()` - appends to attempt-journal.md
- [x] Default base_dir is cwd

### discovery.py
- [x] SkillInfo dataclass creation
- [x] `discover_skills()` - finds SKILL.md files, parses frontmatter
- [x] `get_skills_for_subagent()` - filters by subagent_type
- [x] `get_required_skills_for_subagent()` - returns profile's required_tools
- [x] MCPInfo dataclass creation
- [x] `discover_mcps()` - parses ~/.mcp.json
- [x] `get_mcps_for_subagent()` - filters MCPs for subagent type
- [x] `get_required_mcps_for_subagent()` - returns profile's required_mcps
- [x] Handles missing directories/files gracefully
- [x] Handles invalid YAML/JSON gracefully
- [x] Filters disabled MCPs correctly

### manager.py (OrchestrationManager)
- [x] `__init__()` - accepts RalphConfig, creates CoordinationManager
- [x] `generate_subagent_prompt()` - generates prompt from template
- [x] Prompt contains skill instructions
- [x] Prompt contains MCP list
- [x] Prompt contains coordination paths
- [x] Prompt contains task description with criteria
- [x] `aggregate_results()` - collects and determines verdict
- [x] Verdict PASS when all pass
- [x] Verdict FAIL when any fail
- [x] Returns subagent_results list

## Functions/Classes NOT TESTED

### CRITICAL MISSING: Subagent Spawning

**NO SPAWN FUNCTION EXISTS.** The orchestration module does NOT contain:
- `spawn_subagent()`
- `run_subagent()`
- `execute_subagent()`
- `start_subagent()`

The entire orchestration module is **preparation-only**:
1. Generates prompts
2. Manages coordination files
3. Aggregates results

But there is **NO code to actually spawn Claude subagent processes**.

### Not Tested (Minor)
- `_generate_skill_instructions()` - private helper (tested indirectly)
- `_generate_mcp_list()` - private helper (tested indirectly)
- `_generate_task_description()` - private helper (tested indirectly)
- `_parse_skill_frontmatter()` - private helper (tested indirectly)

## Integration Tests vs Unit Tests

| Type | Count | Notes |
|------|-------|-------|
| Unit Tests | ~74 | Mock file system, isolated function testing |
| Integration Tests | ~3 | `TestIntegrationScenario` in test_orchestration_integration.py |

### Integration Test Coverage
- `test_full_orchestration_workflow()` - Tests the workflow BUT **simulates** subagent results by manually calling `write_subagent_result()`. Does NOT spawn real subagents.

## CRITICAL ANALYSIS: Subagent Spawning

### Question: "Can we actually run an orchestration and have it spawn subagents successfully?"

**ANSWER: NO.**

The orchestration code is **incomplete**. It provides:
1. Prompt generation
2. File coordination protocol
3. Result aggregation

It does NOT provide:
1. Subprocess spawning (`claude -p "prompt"` or SDK calls)
2. Async subagent management
3. Timeout/retry logic for subagent execution
4. Error handling for subprocess failures

### Evidence:
1. `grep -r "spawn_subagent\|run_subagent" .` returns no matches
2. `manager.py` has no subprocess import
3. `coordinator.py` only handles file I/O
4. The integration test manually writes results instead of spawning

### What Would Be Needed:
```python
# Missing code that should exist in manager.py:
async def spawn_subagent(
    self,
    subagent_type: str,
    prompt: str,
    timeout: int = 300
) -> Dict[str, Any]:
    """Spawn a Claude subagent and wait for results."""
    # Option 1: claude CLI
    proc = await asyncio.create_subprocess_exec(
        "claude", "-p", prompt,
        stdout=asyncio.subprocess.PIPE
    )

    # Option 2: claude_agent_sdk
    async with ClaudeSession(...) as session:
        result = await session.prompt(prompt)
```

## Summary Table

| Component | Code Complete | Tests Complete | Can Execute |
|-----------|--------------|----------------|-------------|
| SubagentProfile | YES | YES | N/A (config) |
| CoordinationManager | YES | YES | YES (file I/O) |
| Skill Discovery | YES | YES | YES |
| MCP Discovery | YES | YES | YES |
| Prompt Generation | YES | YES | YES |
| Result Aggregation | YES | YES | YES |
| **Subagent Spawning** | **NO** | **NO** | **NO** |

## Unresolved Questions

1. Was subagent spawning intentionally deferred to a later phase?
2. Should spawning use CLI (`claude -p`) or SDK (`claude_agent_sdk`)?
3. Is there a separate module for subagent execution not in orchestration/?
4. Are there design docs specifying the spawning mechanism?
