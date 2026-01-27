---
status: completed
created: 2026-01-22
started: 2026-01-22
completed: 2026-01-22
---
# Task: Auto-inject .agent context files, task breakdown guidance, and state management

## Description

Add three features to Ralph's prompt generation:
1. Guidance in `core_prompt()` on how to break work into tasks
2. Guidance in `core_prompt()` about when to use memories vs context files (with naming conventions)
3. List filenames of `.md` files in `.agent/` (agent reads if needed based on name)

This keeps it simple - no new config, just smart defaults that let the agent decide what to write where.

## Background

Ralph currently injects:
- Memories from `.agent/memories.md` (via `prepend_memories()`)
- Task instructions (via `core_prompt()` TASKS section)

But agents often need to write longer research/analysis that doesn't fit the memory format. Currently this requires explicit PROMPT.md instructions like "write findings to .agent/". This should be a default behavior.

## Reference Documentation

**Required:**
- `crates/ralph-core/src/hatless_ralph.rs` - `core_prompt()` method
- `crates/ralph-core/src/event_loop.rs` - `prepend_memories()` method

## Technical Requirements

1. **Task Breakdown Guidance** - Add `### 0c. TASK BREAKDOWN` to `core_prompt()` after TASKS section:
   ```
   ### 0c. TASK BREAKDOWN

   You MUST decompose multi-step work before implementing.

   1. **Research first** — You MUST write findings to `.agent/{topic}.md` before coding
   2. **One outcome per task** — Each task MUST have ONE verifiable result
   3. **Dependency order** — You SHOULD use `--blocked-by` for sequential work
   4. **Max 5 active** — You SHOULD NOT create more than 5 tasks at once

   **Good tasks:**
   - 'Fix login validation bug' (one bug, verifiable)
   - 'Add user model with tests' (one component)
   - 'Document auth flow in .agent/auth-research.md' (research deliverable)

   **Bad tasks:**
   - 'Implement feature X' (too vague)
   - 'Fix bugs' (which bugs?)
   - 'Research and implement Y' (two outcomes)
   ```

2. **State Management Guidance** - Add `### 0d. STATE MANAGEMENT` to `core_prompt()` after TASK BREAKDOWN:
   ```
   ### 0d. STATE MANAGEMENT

   **Memories** (`ralph tools memory add`) — You SHOULD use for:
   - Patterns discovered (code conventions, API usage)
   - Decisions made (why X over Y)
   - Fixes that worked (debugging solutions)
   - MUST be brief: 1-2 sentences with tags

   **Context files** (`.agent/*.md`) — You SHOULD use for:
   - Research in progress (findings with file:line refs)
   - Anything longer than a memory
   - Lists of resources to review

   **Naming**: You MUST use descriptive filenames — you'll see the list, not the content.
   - Good: `auth-flow-research.md`, `stale-docs-list.md`, `api-audit.md`
   - Bad: `notes.md`, `tmp.md`, `research.md`

   Rule: If it fits in a tweet, it's a memory. Otherwise, write a context file.
   ```

3. **List context files (not content)** - New method `list_context_files()` in `event_loop.rs`:
   - List all `.md` files from `.agent/` directory (filenames only, not content)
   - Exclude: `memories.md`, `tasks.jsonl`, `scratchpad.md`
   - Inject as a simple list in the prompt
   - Agent decides whether to read based on filename
   - No budget needed - just filenames

4. **Naming guidance** - Add to STATE MANAGEMENT section:
   - Instruct agent to use descriptive filenames
   - Filename should indicate content without needing to read it
   - Examples: `auth-flow-research.md`, `api-endpoints-audit.md`, `stale-docs-list.md`

## Dependencies

- None - builds on existing memory injection infrastructure

## Implementation Approach

1. Write tests for new `core_prompt()` sections
2. Add task breakdown guidance (`0c. TASK BREAKDOWN`) to `core_prompt()`
3. Add state management guidance (`0d. STATE MANAGEMENT`) with naming guidance to `core_prompt()`
4. Write tests for `list_context_files()` behavior
5. Implement `list_context_files()` in event_loop.rs (just filenames, not content)
6. Wire it into `build_prompt()` after `prepend_memories()`
7. Run smoke tests to verify prompt structure

## Acceptance Criteria

1. **Task breakdown guidance appears in prompt**
   - Given memories are enabled (scratchpad disabled)
   - When `core_prompt()` is called
   - Then prompt contains "### 0c. TASK BREAKDOWN" section with decomposition guidance

2. **State management guidance appears in prompt**
   - Given memories are enabled
   - When `core_prompt()` is called
   - Then prompt contains "### 0d. STATE MANAGEMENT" section with memories vs context guidance

3. **Context filenames are listed**
   - Given `.agent/auth-research.md` and `.agent/api-audit.md` exist
   - When `build_prompt()` is called
   - Then prompt contains list: `- auth-research.md`, `- api-audit.md`
   - And prompt does NOT contain file contents

4. **Reserved files are excluded from list**
   - Given `.agent/memories.md` and `.agent/tasks.jsonl` exist
   - When `list_context_files()` runs
   - Then neither appears in the list

5. **Scratchpad is excluded from list**
   - Given `.agent/scratchpad.md` exists
   - When `list_context_files()` runs
   - Then scratchpad does not appear in the list

6. **Empty .agent directory is handled**
   - Given `.agent/` contains only reserved files
   - When `list_context_files()` runs
   - Then no context files section is added to prompt

7. **Naming guidance in state management**
   - Given state management section exists
   - When prompt is generated
   - Then it includes guidance on descriptive filenames with good/bad examples

## Metadata

- **Complexity**: Medium
- **Labels**: prompt, memories, context, injection
- **Required Skills**: Rust, file I/O, testing
