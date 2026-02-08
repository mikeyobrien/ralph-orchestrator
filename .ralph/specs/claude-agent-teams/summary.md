# Summary: Agent Teams in Ralph

## Artifacts

| File | Purpose |
|------|---------|
| `rough-idea.md` | Original issue content (GitHub #161) |
| `requirements.md` | Q&A record — 6 decisions captured |
| `research/codebase-analysis.md` | SOP runner, CLI backend, and touch point analysis |
| `research/agent-teams-feature.md` | Agent Teams documentation, enabling, limitations |
| `design.md` | Full design with architecture, interfaces, 8 acceptance criteria |
| `plan.md` | 6-step incremental implementation plan |

## Overview

Adds `--teams` flag to `ralph plan` and `ralph code-task` that enables Claude Code's experimental Agent Teams feature for interactive PDD sessions — parallel research and adversarial design review. Not available on `ralph run` (Agent Teams requires an interactive session). ~60-80 lines across 5 files, no architectural changes.

## Key Decisions

- **`env_vars` on `CliBackend`** — env vars carried by the backend struct, applied uniformly by all executors (CLI, PTY, SOP runner)
- **Conditional assembly** over SOP duplication (no drift risk)
- **Extensible addendum pattern** — `build_prompt()` takes `&[(&str, &str)]` addendums, future features just push tuples
- **Warn-and-continue** for non-Claude backends (matches codebase pattern)
- **Separate teams method** — `claude_interactive_teams()` (matches existing variant pattern)
- **Suggestive team instructions** (let Claude self-organize, don't prescribe structure)

## Files Changed

| File | Change |
|------|--------|
| `crates/ralph-adapters/src/cli_backend.rs` | Add `env_vars` field, `claude_interactive_teams()` |
| `crates/ralph-adapters/src/cli_executor.rs` | Apply `backend.env_vars` when spawning |
| `crates/ralph-adapters/src/pty_executor.rs` | Apply `backend.env_vars` when spawning |
| `crates/ralph-cli/src/main.rs` | `--teams` on `PlanArgs`, `CodeTaskArgs`; thread to `SopRunConfig` |
| `crates/ralph-cli/src/sop_runner.rs` | `agent_teams` field, extensible `build_prompt()`, backend selection, warning |
| `crates/ralph-cli/sops/pdd-team-addendum.md` | New — suggestive team instructions |

## Next Steps

1. **Implement** — Create code tasks from the plan
2. **Manual test** — `ralph plan --teams "Build a rate limiter"` to verify end-to-end
3. **Future addendums** — The extensible pattern is ready for additional prompt addendums (e.g., AskUserQuestion tool guidance)
