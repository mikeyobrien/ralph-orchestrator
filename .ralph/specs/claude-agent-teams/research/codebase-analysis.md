# Codebase Analysis: SOP Runner & Plan Command

## Current Architecture

### `sop_runner.rs` — The SOP execution engine

The SOP runner is a clean, self-contained module (~230 lines + tests) that:

1. **Resolves backend** — CLI flag > config file > auto-detect
2. **Builds prompt** — Wraps SOP content in `<sop>` tags, user input in `<user-content>` tags
3. **Spawns interactive session** — Uses `std::process::Command` with inherited stdin/stdout/stderr

Key types:

- `Sop` enum: `Pdd` | `CodeTaskGenerator` — each maps to `include_str!`'d content
- `SopRunConfig` struct: `sop`, `user_input`, `backend_override`, `config_path`, `custom_args`
- No `agent_teams` field currently exists

### `main.rs` — `PlanArgs` and `plan_command()`

`PlanArgs` (line 671):

```rust
struct PlanArgs {
    idea: Option<String>,        // positional
    backend: Option<String>,     // --backend / -b
    custom_args: Vec<String>,    // after --
}
```

`plan_command()` (line 2067): Simply constructs `SopRunConfig` and calls `run_sop()`. No special handling.

### `cli_backend.rs` — `CliBackend::claude_interactive()`

```rust
pub fn claude_interactive() -> Self {
    Self {
        command: "claude".to_string(),
        args: vec![
            "--dangerously-skip-permissions".to_string(),
            "--disallowedTools=TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet".to_string(),
        ],
        prompt_mode: PromptMode::Arg,
        prompt_flag: None,
        output_format: OutputFormat::Text,
    }
}
```

Key observation: `--disallowedTools` includes `TaskCreate,TaskUpdate,TaskList,TaskGet` — these are **required** by Agent Teams for coordination. They must be removed when teams are enabled.

### `for_interactive_prompt()` dispatch

Maps backend names to their interactive constructors. Only `claude` uses `claude_interactive()`. Other backends (kiro, gemini, codex, amp, copilot, opencode, pi) have their own interactive variants.

## Touch Points for the Feature

1. **`PlanArgs`** — Add `--teams` bool flag
2. **`SopRunConfig`** — Add `agent_teams: bool` field
3. **`plan_command()`** — Thread `args.teams` → `config.agent_teams`
4. **`run_sop()`** — Select SOP variant based on `agent_teams`, adjust backend
5. **`spawn_interactive()`** — Inject `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` env var
6. **`CliBackend`** — Need a way to get a variant of `claude_interactive()` without disallowing Team tools
7. **New SOP** — `pdd-teams.md` with team-aware instructions for Steps 4, 5, 6

## Env Var Injection

`spawn_interactive()` uses `Command::new()` which supports `.env(key, value)`. Trivial to add.

The question is where to thread the flag:

- Option A: Pass through `SopRunConfig` → `spawn_interactive()` (cleanest)
- Option B: Let `CliBackend` carry env vars (more general but over-engineered for now)

## DisallowedTools Adjustment

Currently `claude_interactive()` hardcodes the disallowed tools string. Options:

- Option A: New method `claude_interactive_with_teams()` that omits Task* tools
- Option B: `claude_interactive()` takes a parameter to control tool disallowing
- Option C: Adjust the args in `spawn_interactive()` or `run_sop()` after construction

Option A is simplest and matches the existing pattern (each variant is a separate method).
