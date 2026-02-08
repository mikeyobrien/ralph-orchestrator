# Rough Idea

**Source:** [GitHub Issue #161](https://github.com/mikeyobrien/ralph-orchestrator/issues/161)

## Summary

Support Claude Agent Teams in `ralph plan` for parallel research and design.

`ralph plan` currently spawns a single interactive Claude session that executes the PDD SOP sequentially. For complex features, the research (Step 4) and design (Step 6) phases become bottlenecks because a single agent must serially explore codebase patterns, investigate libraries, review prior art, and draft designs.

Claude Code's experimental Agent Teams feature allows spawning parallel teammates that work independently and communicate. This is a natural fit for PDD's research and design phases:

- **Parallel research**: One teammate investigates codebase patterns while another researches external approaches
- **Adversarial design review**: A teammate plays devil's advocate on the design while another strengthens it
- **Competing hypotheses**: Multiple teammates explore different architectural approaches and debate trade-offs

## Proposed Solution (from issue)

### Core plumbing (~20 lines across 2 files)

1. **New CLI flag**: `ralph plan --teams "Build a rate limiter"`
2. **Environment variable injection**: Set `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` on the spawned process
3. **Adjust disallowed tools**: Remove `TaskCreate`, `TaskUpdate`, `TaskList`, `TaskGet` from `--disallowedTools` when teams are enabled

### SOP handling: separate team-aware variant

Duplicate the SOP into `pdd.md` (unchanged) and `pdd-teams.md` (team-aware variant). The SOP runner selects which to use based on the `--teams` flag.

### Files affected

| File | Change |
|------|--------|
| `crates/ralph-cli/src/main.rs` | Add `--teams` to `PlanArgs`, thread to `SopRunConfig` |
| `crates/ralph-cli/src/sop_runner.rs` | Accept `agent_teams` flag, select SOP variant, inject env var, adjust `--disallowedTools` |
| `crates/ralph-cli/sops/pdd-teams.md` | New team-aware PDD SOP variant |
