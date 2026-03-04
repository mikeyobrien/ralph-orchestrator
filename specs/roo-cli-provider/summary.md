# Project Summary: Add roo-cli as a Provider

## Directory Structure

```
.agents/planning/2026-03-03-roo-cli-provider/
├── rough-idea.md                          # Initial concept
├── idea-honing.md                         # Q&A requirements clarification (10 questions)
├── research/
│   ├── roo-cli-interface.md               # Roo CLI flags, stream-json format, examples
│   ├── ralph-adapter-system.md            # How Ralph's adapter system works
│   └── text-vs-stream-json.md             # Analysis of text vs stream-json impact
├── design/
│   └── detailed-design.md                 # Complete design document
├── implementation/
│   └── plan.md                            # 7-step implementation plan with checklist
└── summary.md                             # This document
```

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Output format | Text mode | Simple, proven (6/9 providers), no parser needed |
| Headless command | `roo --print --ephemeral` | Non-interactive + clean disk state |
| Prompt passing | Always `--prompt-file` | One code path, native roo support, no size threshold |
| `--ephemeral` | Included | Fixed in roo v0.1.15+ to work with Bedrock |
| Tool auto-approval | Default (no flag) | Roo auto-approves by default |
| Auto-detect priority | Last position | Fallback only |
| PTY mode | `pty_mode: false` | Clean pipe output for text parsing |

## Implementation Overview

The implementation consists of **7 incremental steps**, each producing working, testable functionality:

1. **Backend definitions** — `roo()` and `roo_interactive()` methods
2. **Registration** — Wire into `from_config`, `from_name`, `for_interactive_prompt`
3. **Prompt file support** — Always use `--prompt-file` for all prompts
4. **Auto-detection** — Add to `DEFAULT_PRIORITY`, update error messages
5. **Preset file** — `presets/minimal/roo.yml`
6. **Documentation** — Update crate docs
7. **Validation** — Full test suite + manual E2E

## Files Modified

| File | Changes |
|------|---------|
| `crates/ralph-adapters/src/cli_backend.rs` | Add `roo()`, `roo_interactive()`, registration points, tests |
| `crates/ralph-adapters/src/auto_detect.rs` | Add to priority, error message, tests |
| `crates/ralph-adapters/src/lib.rs` | Update doc comment |
| `presets/minimal/roo.yml` | New file |

## Estimated Effort

~2-3 hours for a developer familiar with the codebase. The changes are mechanical — following established patterns from kiro, pi, and other backends.

## Next Steps

1. Review the [detailed design document](design/detailed-design.md)
2. Check the [implementation plan](implementation/plan.md) and its checklist
3. Begin implementation following the plan steps sequentially
4. Run `cargo test` after each step to verify no regressions
