---
status: complete
created: 2026-06-21
completed: 2026-06-21
---
# Task: Respect Per-Hat Scratchpad Config in Generated Custom-Hat Instructions

## Description
Generated custom-hat instructions must use the resolved per-hat `ScratchpadConfig`
instead of hardcoded scratchpad wording.

## Acceptance Criteria
- Disabled per-hat scratchpad configs do not generate scratchpad instructions.
- Custom scratchpad paths are named in generated behavior text.
- Hats inheriting the default scratchpad keep the existing default wording.
- Event-loop custom-hat prompt construction passes the resolved scratchpad config.
- Focused and crate-level `ralph-core` tests pass.
