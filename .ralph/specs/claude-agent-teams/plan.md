# Implementation Plan: Agent Teams in Ralph

## Checklist

- [ ] Step 1: Add `env_vars` to `CliBackend` and teams backend methods
- [ ] Step 2: Apply `env_vars` in executors
- [ ] Step 3: Add `--teams` flag to interactive commands and thread plumbing
- [ ] Step 4: Create team addendum file
- [ ] Step 5: Wire up SOP runner (addendum pattern, backend selection, warning)

---

## Step 1: Add `env_vars` to `CliBackend` and teams backend methods

**Objective:** Extend `CliBackend` with `env_vars` field and add teams-aware backend constructors.

**Implementation guidance:**

- `crates/ralph-adapters/src/cli_backend.rs`:
  - Add `pub env_vars: Vec<(String, String)>` to `CliBackend` struct
  - Add `env_vars: vec![]` to ALL existing constructors (`claude()`, `claude_interactive()`, `kiro()`, `kiro_interactive()`, `gemini()`, etc., `from_config()`, `custom()`, `from_hat_backend()`)
  - Add `claude_interactive_teams()` method — like `claude_interactive()` but `--disallowedTools=TodoWrite` only, env_vars includes Agent Teams env var

**Test requirements:**

- `test_claude_interactive_teams_backend` — verify args, env_vars, no prompt_flag
- `test_env_vars_default_empty` — verify `claude()` and `claude_interactive()` have empty env_vars

**Integration notes:** Compile will enforce `env_vars` on all struct literals. This is the foundation — no behavior change yet since executors don't apply env_vars yet.

**Demo:** `cargo test -p ralph-adapters` passes.

---

## Step 2: Apply `env_vars` in executors

**Objective:** Make both executors apply `backend.env_vars` when spawning processes.

**Implementation guidance:**

- `crates/ralph-adapters/src/cli_executor.rs`:
  - After `command.current_dir()` (line ~72), add:

    ```rust
    for (key, value) in &self.backend.env_vars {
        command.env(key, value);
    }
    ```

- `crates/ralph-adapters/src/pty_executor.rs`:
  - After `cmd_builder.cwd()` (line ~291), before `cmd_builder.env("TERM", ...)`, add:

    ```rust
    for (key, value) in &self.backend.env_vars {
        cmd_builder.env(key, value);
    }
    ```

- `crates/ralph-cli/src/sop_runner.rs`:
  - In `spawn_interactive()`, after `Command::new()`, add same pattern:

    ```rust
    for (key, value) in &backend.env_vars {
        child_cmd.env(key, value);
    }
    ```

**Test requirements:**

- Existing executor tests continue to pass (empty env_vars = no change)
- No new tests needed — env_vars application is trivial and covered by integration

**Integration notes:** With empty env_vars on all current backends, this is a no-op for existing behavior. Combined with Step 1, the plumbing is complete.

**Demo:** `cargo test -p ralph-adapters && cargo test -p ralph-cli` passes.

---

## Step 3: Add `--teams` flag to interactive commands and thread plumbing

**Objective:** Thread the `--teams` CLI flag from argument parsing through to `SopRunConfig`.

**Implementation guidance:**

- `crates/ralph-cli/src/main.rs`:
  - Add `#[arg(long)] teams: bool` to `PlanArgs` (after `backend`)
  - Add `#[arg(long)] teams: bool` to `CodeTaskArgs` (after `backend`)
  - In `plan_command()`: set `agent_teams: args.teams` on `SopRunConfig`
  - In `code_task_command()`: set `agent_teams: args.teams` on `SopRunConfig`
- `crates/ralph-cli/src/sop_runner.rs`:
  - Add `pub agent_teams: bool` to `SopRunConfig`

**Test requirements:**

- Existing tests that construct `SopRunConfig` need `agent_teams: false` added (compile enforces)

**Integration notes:** After this step, `--teams` is accepted on `plan` and `code-task` but has no effect yet.

**Demo:** `ralph plan --help` and `ralph code-task --help` show `--teams` flag.

---

## Step 4: Create team addendum file

**Objective:** Write the team-specific prompt instructions as a separate markdown file.

**Implementation guidance:**

- Create `crates/ralph-cli/sops/pdd-team-addendum.md`
- Content should be suggestive, not prescriptive. Cover:
  - Research phase: use Agent Teams to parallelize research across multiple teammates; synthesize findings before presenting to user
  - Design phase: consider spawning a critic teammate for adversarial review
  - Iteration checkpoint: consolidate all teammate findings before asking user to proceed
  - General: user gates still enforced, lead maintains conversation, teammates work in background
- Add `pub const PDD_TEAM_ADDENDUM: &str = include_str!("../sops/pdd-team-addendum.md");` to `sops` module in `sop_runner.rs`

**Test requirements:**

- `test_sop_content_pdd_team_addendum` — verify content contains expected keywords (e.g., "Agent Teams", "teammates")

**Integration notes:** File exists but isn't wired into the prompt yet. Step 5 wires it up.

**Demo:** `cargo test -p ralph-cli test_sop_content_pdd_team_addendum` passes.

---

## Step 5: Wire up SOP runner (addendum pattern, backend selection, warning)

**Objective:** Complete `ralph plan --teams` and `ralph code-task --teams` — extensible addendums, backend selection, non-Claude warning.

**Implementation guidance:**

In `sop_runner.rs`:

1. **`build_prompt()`** — change signature to accept addendums:

   ```rust
   fn build_prompt(sop: Sop, user_input: Option<&str>, addendums: &[(&str, &str)])
   ```

   - Each addendum appended after `</sop>` as `<{tag}>\n{content}\n</{tag}>`
   - `<user-content>` remains last if present
   - Empty addendums = identical to current behavior

2. **`run_sop()`** — update the logic:
   - Build a `Vec<(&str, &str)>` of addendums
   - After resolving `backend_name`, check if `agent_teams` is true and backend is not `"claude"`
   - If non-Claude: `warn!("--teams is only supported with the Claude backend, ignoring")`, set `effective_teams = false`
   - When `effective_teams`, push `("team-instructions", sops::PDD_TEAM_ADDENDUM)` onto addendums
   - Select `claude_interactive_teams()` vs `claude_interactive()` based on `effective_teams`
   - Handle the claude-teams case in `run_sop()` before calling `for_interactive_prompt()` to avoid changing the adapter crate's public API

**Extensibility:** Adding a future addendum is just:

```rust
addendums.push(("tool-guidance", sops::TOOL_GUIDANCE_ADDENDUM));
```

**Test requirements:**

- `test_build_prompt_with_addendums` — verify addendum wrapped in correct XML tags
- `test_build_prompt_with_multiple_addendums` — verify multiple addendums in order
- `test_build_prompt_with_addendums_and_user_input` — verify ordering: `<sop>`, addendums, `<user-content>`
- `test_build_prompt_no_addendums_unchanged` — regression test

**Integration notes:** This makes `ralph plan --teams` and `ralph code-task --teams` fully functional.

**Demo:**

- `ralph plan --teams "Build a rate limiter"` — Claude starts with teams
- `ralph plan --teams --backend gemini "idea"` — warning, proceeds without teams
