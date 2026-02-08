# Tool Profile System Implementation Plan

**Date:** 2026-01-04
**Branch:** feat/orchestration
**Status:** ACTIVE

---

## Objective

Implement AI-driven tool profile generation during prompt onboarding that:
1. Analyzes prompt content to understand task type, technologies, workflow
2. Discovers available skills and MCPs
3. Matches tools to requirements
4. Generates `.agent/tool-profile.json` cached by `prompt_hash`
5. Integrates with orchestration to configure subagents

---

## Context

From design docs:
- `docs/designs/2026-01-04-onboarding-architecture.md`
- `docs/designs/2026-01-04-agent-harness.md`

Key insight: Tool selection happens at **onboarding time** (once per prompt), not runtime.
Profile is cached by `prompt_hash` and reused unless prompt changes.

---

## Implementation Phases

### Phase 1: Prompt Analyzer (`prompt_analyzer.py`)
**Goal:** Analyze prompt content to extract requirements

**Tasks:**
1. Create `PromptAnalysis` dataclass with:
   - `task_type`: str (ios, web, backend, mobile, general)
   - `technologies`: List[str] (react, swift, python, etc.)
   - `phases`: List[str] (extracted from prompt)
   - `required_skills`: List[str] (inferred from content)
   - `required_mcps`: List[str] (inferred from content)

2. Implement `analyze_prompt(prompt_path: Path) -> PromptAnalysis`:
   - Read prompt file
   - Parse for technology keywords
   - Extract phase structure
   - Infer skill/MCP requirements

3. Write tests FIRST (TDD):
   - Test keyword extraction
   - Test phase parsing
   - Test skill inference

**Files:**
- `src/ralph_orchestrator/orchestration/prompt_analyzer.py`
- `tests/orchestration/test_prompt_analyzer.py`

---

### Phase 2: Tool Profile Schema (`tool_profile.py`)
**Goal:** Define and generate tool-profile.json

**Tasks:**
1. Create `ToolProfile` dataclass:
   ```python
   @dataclass
   class ToolProfile:
       prompt_hash: str  # SHA256 of prompt content
       created_at: str   # ISO timestamp
       prompt_file: str  # Original prompt path
       skills: SkillConfig  # required + task_specific
       mcps: MCPConfig     # by category
       analysis: PromptAnalysis  # Original analysis
   ```

2. Implement `generate_tool_profile(analysis: PromptAnalysis) -> ToolProfile`:
   - Match analysis to available skills (from discovery.py)
   - Match analysis to available MCPs (from discovery.py)
   - Compute prompt_hash
   - Return complete profile

3. Write tests FIRST:
   - Test profile generation
   - Test hash computation
   - Test skill/MCP matching

**Files:**
- `src/ralph_orchestrator/orchestration/tool_profile.py`
- `tests/orchestration/test_tool_profile.py`

---

### Phase 3: Profile Caching
**Goal:** Cache and retrieve profiles by prompt_hash

**Tasks:**
1. Implement `save_profile(profile: ToolProfile, agent_dir: Path)`:
   - Write to `.agent/tool-profile.json`
   - Include prompt_hash for validation

2. Implement `load_profile(agent_dir: Path) -> Optional[ToolProfile]`:
   - Read from `.agent/tool-profile.json`
   - Return None if not exists

3. Implement `is_profile_valid(profile: ToolProfile, prompt_path: Path) -> bool`:
   - Recompute prompt_hash
   - Compare with stored hash
   - Return True if match (profile still valid)

4. Write tests FIRST:
   - Test save/load roundtrip
   - Test hash validation
   - Test cache invalidation on prompt change

**Files:**
- Same as Phase 2 (extend `tool_profile.py`)
- `tests/orchestration/test_tool_profile.py` (extend)

---

### Phase 4: Orchestrator Integration
**Goal:** Use tool profiles in subagent configuration

**Tasks:**
1. Add `_check_or_generate_profile()` to OrchestrationManager:
   - Check for cached profile
   - If valid, return it
   - If invalid/missing, run prompt analysis + generation

2. Modify `spawn_subagent()` to use profile:
   - Get skills for subagent type from profile
   - Get MCPs for subagent type from profile
   - Configure subagent accordingly

3. Update coordination flow:
   - Profile generation happens before first iteration
   - Profile is read-only during iterations

**Files:**
- `src/ralph_orchestrator/orchestration/manager.py`
- `tests/orchestration/test_manager.py`

---

## Success Criteria

- [ ] `analyze_prompt()` extracts task type, technologies, phases
- [ ] `generate_tool_profile()` creates valid profile
- [ ] Profile cached to `.agent/tool-profile.json`
- [ ] Profile reused if prompt_hash matches
- [ ] Profile invalidated if prompt changes
- [ ] Subagents configured from profile (not static config)
- [ ] All tests pass

---

## File Structure After Implementation

```
src/ralph_orchestrator/orchestration/
├── __init__.py
├── config.py           # SubagentProfile (existing)
├── discovery.py        # Skill/MCP discovery (existing)
├── manager.py          # OrchestrationManager (modified)
├── prompt_analyzer.py  # NEW: Prompt analysis
└── tool_profile.py     # NEW: Profile generation/caching

.agent/
└── tool-profile.json   # Cached profile (per-prompt)
```

---

## Verification

After each phase:
1. Run tests: `python -m pytest tests/orchestration/ -v`
2. Verify no regressions
3. Check type hints with mypy

Final verification:
1. Run `ralph run -P prompts/MOBILE_PROMPT.md`
2. Confirm `.agent/tool-profile.json` created
3. Re-run same prompt - confirm profile reused
4. Modify prompt - confirm profile regenerated
