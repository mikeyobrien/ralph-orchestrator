# Project Summary: SOP Injection Subcommands

## Artifacts Created

```
specs/sop-injection-commands/
├── rough-idea.md              # Original concept
├── idea-honing.md             # 9 requirements questions answered
├── research/
│   ├── cli-architecture.md    # Ralph CLI structure analysis
│   └── backend-compatibility.md # Backend interactive mode research
├── design/
│   └── detailed-design.md     # Full technical specification
├── PROMPT.md                  # Ralph-ready implementation prompt
└── summary.md                 # This file
```

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Event loop | Bypass entirely | Simpler, SOPs handle their own flow |
| Backend resolution | Flag → Config → Auto-detect | Maximum flexibility |
| SOP delivery | `include_str!` bundling | Self-contained binary |
| Execution mode | Always interactive | SOPs require conversation |
| Prompt format | XML tags | Clear structure for LLM parsing |
| Command structure | Top-level | Discoverable, consistent with existing |

## Implementation Scope

### New Code
- `crates/ralph-cli/src/sop_runner.rs` - New module (~150 lines)
- 4 new backend methods in `cli_backend.rs` (~40 lines)
- CLI command handlers in `main.rs` (~30 lines)

### Modified Files
- `crates/ralph-cli/src/main.rs` - Add commands
- `crates/ralph-adapters/src/cli_backend.rs` - Add interactive methods
- `.claude/skills/pdd/SKILL.md` - Add PROMPT.md offer
- `.claude/skills/code-task-generator/SKILL.md` - Add PROMPT.md offer

## Critical Finding from Research

**Gemini CLI requires `-i` flag (not `-p`) for interactive mode with initial prompt.**

Without this, `ralph plan` with Gemini would exit after one response instead of maintaining a conversation. This was discovered through web research and is documented in `research/backend-compatibility.md`.

## Next Steps

1. **Use Ralph to implement**: Run Ralph with `PROMPT.md` as input
2. **Manual testing**: Verify each backend works correctly
3. **Update SOPs**: Add the "Would you like a PROMPT.md?" prompts

## Usage After Implementation

```bash
# Start a planning session
ralph plan
ralph plan "build a REST API for user management"
ralph plan --backend kiro "my idea"

# Start a task generation session
ralph task
ralph task "add authentication to the API"
ralph task specs/my-feature/implementation/plan.md

# View help
ralph plan --help
ralph task --help
```
