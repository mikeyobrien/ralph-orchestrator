---
status: complete
created: 2026-01-15
completed: 2026-01-15
---
# Task: Fix Events Context Ignored in Ralph's Prompt

## Description
The `HatlessRalph::build_prompt()` method receives an events context parameter containing pending events (including the user's task), but completely ignores it. This causes Ralph to never see the user's actual task and instead just read the scratchpad, find "all tasks complete", and terminate.

## Background
The data flow is:
1. User's prompt → `task.start` event payload ✓
2. Event payload → `events_context` string in `event_loop.rs:390-410` ✓
3. `events_context` → `ralph.build_prompt(&events_context)` in `event_loop.rs:414` ✓
4. `build_prompt(_context: &str)` → **IGNORED** ✗

The underscore prefix `_context` in Rust means "deliberately unused" - this suppresses compiler warnings but means the events are never shown to Claude.

## Evidence
From preset evaluation logs (`.eval/logs/adversarial-review/20260115_110419/output.log`):
- User's task: "Review this user input handler for security vulnerabilities..."
- Ralph's response: "The scratchpad shows all 18 code tasks are complete"
- Result: LOOP_COMPLETE without ever addressing the security review task

## Reference Documentation
- `crates/ralph-core/src/hatless_ralph.rs:78` - The ignored `_context` parameter
- `crates/ralph-core/src/event_loop.rs:385-414` - Where events context is built and passed
- `crates/ralph-cli/src/main.rs:1037` - Where user prompt becomes `task.start` event

## Technical Requirements
1. Rename `_context` to `context` in `build_prompt()` signature
2. Add a `## PENDING EVENTS` section to Ralph's prompt that includes the context
3. Place this section BEFORE the workflow section so Ralph sees the task first
4. Handle empty context gracefully (don't add section if no events)
5. Update tests to verify events appear in prompt

## Implementation Approach
1. In `hatless_ralph.rs`, modify `build_prompt()`:
   ```rust
   pub fn build_prompt(&self, context: &str) -> String {
       let mut prompt = self.core_prompt();

       // Include pending events BEFORE workflow so Ralph sees the task
       if !context.trim().is_empty() {
           prompt.push_str("## PENDING EVENTS\n\n");
           prompt.push_str(context);
           prompt.push_str("\n\n");
       }

       prompt.push_str(&self.workflow_section());
       // ... rest unchanged
   }
   ```
2. Update existing tests that call `build_prompt("")` to verify behavior
3. Add new test with actual event content to verify it appears in prompt

## Acceptance Criteria

1. **Events Context Included in Prompt**
   - Given a non-empty events context
   - When `build_prompt(context)` is called
   - Then the prompt contains `## PENDING EVENTS` section with the context

2. **Empty Context Handled**
   - Given an empty events context
   - When `build_prompt("")` is called
   - Then no `## PENDING EVENTS` section appears

3. **Events Section Positioned Correctly**
   - Given events context with a task
   - When prompt is built
   - Then `## PENDING EVENTS` appears BEFORE `## WORKFLOW`

4. **User Task Visible to Claude**
   - Given a user prompt passed via `-p`
   - When Ralph's iteration runs
   - Then Claude's prompt includes the user's task content

5. **Existing Tests Pass**
   - Given the implementation changes
   - When running `cargo test -p ralph-core`
   - Then all existing tests continue to pass

6. **New Test Coverage**
   - Given a test with events context
   - When testing prompt generation
   - Then events appear in the generated prompt

## Metadata
- **Complexity**: Low
- **Labels**: Critical Bug Fix, Prompt Building, Hat Routing
- **Required Skills**: Rust
