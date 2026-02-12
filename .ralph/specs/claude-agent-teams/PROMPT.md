# Implement Agent Teams support in Ralph

## Objective

Add `--teams` flag to `ralph plan`, `ralph code-task`, and `ralph run` that enables Claude Code's experimental Agent Teams feature.

## Key Requirements

1. Add `pub env_vars: Vec<(String, String)>` to `CliBackend` struct in `cli_backend.rs`. All existing constructors set `env_vars: vec![]`.
2. Add `claude_teams()` method — like `claude()` but `--disallowedTools=TodoWrite` only, `env_vars` includes `("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "1")`
3. Add `claude_interactive_teams()` method — like `claude_interactive()` but `--disallowedTools=TodoWrite` only, same env var
4. Apply `backend.env_vars` in `cli_executor.rs`, `pty_executor.rs`, and `sop_runner.rs::spawn_interactive()` when spawning processes
5. Add `--teams` bool flag to `PlanArgs`, `CodeTaskArgs`, and `RunArgs` in `main.rs`
6. Add `agent_teams: bool` to `SopRunConfig` in `sop_runner.rs`
7. Make `build_prompt()` accept `addendums: &[(&str, &str)]` — a slice of `(xml_tag_name, content)` pairs, each wrapped as `<tag>\ncontent\n</tag>` after `</sop>` and before `<user-content>`. Empty slice = current behavior.
8. Create `crates/ralph-cli/sops/pdd-team-addendum.md` with suggestive (not prescriptive) team instructions. Load via `include_str!` as `sops::PDD_TEAM_ADDENDUM`.
9. In `run_sop()`: build addendums vec, push team addendum when `agent_teams && backend == "claude"`. For non-Claude, `warn!()` and proceed without teams. Select `claude_interactive_teams()` vs `claude_interactive()`.
10. In `loop_runner`: accept `agent_teams` param. When true and backend is Claude, swap to `claude_teams()`. For non-Claude, `warn!()` and proceed.

## Acceptance Criteria

- **Given** `ralph plan --teams "idea"` with Claude backend, **when** spawned, **then** env var is set via `backend.env_vars`, `--disallowedTools=TodoWrite` only, and prompt contains `<team-instructions>` block
- **Given** `ralph run --teams -p "prompt"` with Claude backend, **when** loop runs, **then** `claude_teams()` backend is used with env var and reduced disallowed tools
- **Given** `--teams` with non-Claude backend on any command, **when** run, **then** warning is printed and session proceeds without teams
- **Given** any command without `--teams`, **when** spawned, **then** behavior is identical to current
- **Given** `build_prompt()` called with multiple addendums, **when** prompt is built, **then** all addendums appear in order after `</sop>` and before `<user-content>`

## Reference

Full design and plan: `specs/agent-teams-in-plan/`
