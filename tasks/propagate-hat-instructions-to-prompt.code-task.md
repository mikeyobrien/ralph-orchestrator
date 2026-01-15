---
status: completed
created: 2026-01-15
completed: 2026-01-15
---
# Task: Propagate Hat Instructions to Ralph's Prompt

## Description
Fix the data flow so that hat instructions defined in configuration are included in Ralph's prompt. Currently, instructions are captured in the HatRegistry but not propagated through HatTopology to the prompt output.

## Background
When custom hats are defined with instructions (persona/behavior guidance), those instructions should be visible to Ralph so he can adopt the appropriate persona when processing events for that hat. Currently:

1. ✓ `hat_registry.rs:39` captures `hat.instructions = config.instructions.clone()`
2. ✗ `hatless_ralph.rs:25-29` - `HatInfo` struct missing `instructions` field
3. ✗ `hatless_ralph.rs:35-40` - `from_registry()` doesn't copy instructions
4. ✗ `hatless_ralph.rs:202-225` - `hats_section()` doesn't render instructions

## Reference Documentation
- `crates/ralph-core/src/hatless_ralph.rs` - HatInfo struct and hats_section method
- `crates/ralph-core/src/hat_registry.rs` - Where instructions ARE captured
- `crates/ralph-proto/src/hat.rs` - Hat struct with instructions field

## Technical Requirements
1. Add `instructions: String` field to `HatInfo` struct
2. Update `HatTopology::from_registry()` to copy `hat.instructions`
3. Update `hats_section()` to render instructions for each hat
4. Instructions should appear AFTER the topology table, one section per hat with instructions

## Implementation Approach
1. Modify `HatInfo` struct to include `instructions` field
2. Update `from_registry()` mapping to include `hat.instructions.clone()`
3. In `hats_section()`, after the table, add a section for each hat that has non-empty instructions:
   ```
   ### TDD Writer Instructions
   [instructions content here]
   ```
4. Update existing tests to verify instructions propagation
5. Add new test case for hat with instructions

## Acceptance Criteria

1. **HatInfo Contains Instructions**
   - Given a hat with instructions in config
   - When HatTopology is built from registry
   - Then HatInfo.instructions contains the hat's instructions

2. **Instructions Rendered in Prompt**
   - Given a hat with non-empty instructions
   - When Ralph's prompt is built
   - Then the prompt includes a section with that hat's instructions

3. **Empty Instructions Omitted**
   - Given a hat with empty/no instructions
   - When Ralph's prompt is built
   - Then no instructions section appears for that hat

4. **Existing Tests Pass**
   - Given the implementation changes
   - When running `cargo test -p ralph-core`
   - Then all existing tests continue to pass

5. **New Test Coverage**
   - Given a test configuration with hat instructions
   - When testing prompt generation
   - Then instructions appear in the generated prompt

## Metadata
- **Complexity**: Low
- **Labels**: Bug Fix, Prompt Building, Hat System
- **Required Skills**: Rust, understanding of Ralph's prompt system
