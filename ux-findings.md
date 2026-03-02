# CLI UX Findings (Fresh-Eyes Pass)

Date: 2026-02-15
Scope: `ralph` CLI runtime help/docs parity, command flow, and user-facing surfaces.

Status: **Addressed** — all listed findings were remediated in this pass via code and documentation fixes.


## Contradictions / UX Drift

1. **Preset lookup failure points to `init` instead of `run` (wrong next action message)**
   - Triggering a bad built-in preset via non-init entrypoints yields: `Run \`ralph init --list-presets\``.
   - Repro: `ralph run --dry-run -c builtin:not-a-preset` (and same message appears in `preflight`).
   - Why this matters: the user attempted to run, but recovery guidance sends them to setup flow, not runtime command context.
   - Evidence: `crates/ralph-cli/src/main.rs:1147-1151`, `crates/ralph-cli/src/preflight.rs:217-222`.

2. **`ralph init` no-arg fallback prints an outdated backend list**
   - `ralph init` (no flags) prints: `Backends: claude, kiro, gemini, codex, amp, custom`.
   - Actual supported backends include `copilot`, `opencode`, and `pi`.
   - Why this matters: user sees impossible/incomplete option set and may assume supported backends are missing.
   - Evidence: `crates/ralph-cli/src/main.rs:1771-1775`, `crates/ralph-cli/src/backend_support.rs`.

3. **Command surface documented as “complete” but omits major real commands**
   - Runtime `ralph --help` includes `preflight`, `doctor`, `tutorial`, `loops`, `hats`, `web`, `bot`, and `completions`.
   - `docs/guide/cli-reference.md` documents only `run`, `init`, `plan`, `task`, `events`, `emit`, `clean`, and `tools`.
   - Why this matters: users relying on reference docs miss discoverability for operationally important commands.
   - Evidence: `./target/debug/ralph --help` command list; `rg -n "^### ralph" docs/guide/cli-reference.md`.

4. **`tools` subtree docs are significantly incomplete vs runtime API**
   - Docs list only `memory` + `task`, but runtime has `skill` and `interact` as well.
   - `tools memory` docs omit `init`; `tools task` docs omit `fail` and `show`.
   - Why this matters: users miss valid subcommands and automation hooks (skills + human interaction).
   - Evidence: `./target/debug/ralph tools --help`; `./target/debug/ralph tools memory --help`; `./target/debug/ralph tools task --help`; docs headings around line ~288 onward in `docs/guide/cli-reference.md`.

5. **`emit` docs drift from implementation**
   - `cli-reference` describes `--json <DATA>` and implies JSON payload as option-argument.
   - Real CLI uses a boolean `--json` flag; payload is positional.
   - Why this matters: copy/paste from docs creates avoidable errors.
   - Evidence: `docs/guide/cli-reference.md:248-249` vs `crates/ralph-cli/src/main.rs` help output `-j, --json`.

## Redundancies / Friction

6. **`-c/--config` is displayed for almost every command, but many command handlers ignore it**
   - The warning path explicitly tells users: `The -c/--config flag is not used by ...`.
   - This creates mixed UX: a “supported” flag that often does nothing.
   - Evidence: `crates/ralph-cli/src/main.rs:852-858` and unsupported runtime warning example on commands such as `events`.

7. **Hidden `task` alias continues as legacy surface duplication**
   - There is both `code-task` and hidden legacy `task` alias to it; the visible UX requires memorizing this exception.
   - Why this matters: inconsistent command naming and hidden pathways increase onboarding friction.
   - Evidence: `crates/ralph-cli/src/main.rs:426-431` (hidden alias metadata).

## Bugs / Behavioral Issues

8. **`ralph bot onboard --telegram` is a placeholder-like flag**
   - `--telegram` defaults true, and `--no-telegram` is explicitly rejected as unsupported.
   - Why this matters: exposes a flag without actual use and creates a dead affordance.
   - Evidence: `crates/ralph-cli/src/bot.rs:169-173`.

9. **Environment variable docs for CLI are stale: `RALPH_CONFIG` and `NO_COLOR` are documented but not implemented**
   - `rg` shows no `RALPH_CONFIG` handling in `crates/ralph-cli/src`.
   - `ColorMode::should_use_colors` checks only CLI mode + TTY state; it does not honor `NO_COLOR`.
   - Verified by forcing a tty-logged run with `NO_COLOR=1` and `--color always` still emitting ANSI escapes.
   - Why this matters: users set env vars that silently no-op.
   - Evidence: `docs/guide/cli-reference.md:417-421`, `docs/guide/configuration.md:263-265`; `crates/ralph-cli/src/main.rs:151-159`.

10. **Environment/config docs imply CLI-level default config variable support (`RALPH_CONFIG`) that doesn’t exist**
   - Docs list `RALPH_CONFIG` as CLI env, but passing it in a directory with no `ralph.yml` does not alter config loading behavior.
   - Why this matters: bad docs causes failed automation in scripted setups.
   - Evidence: `docs/guide/cli-reference.md:420`, `docs/guide/configuration.md:263`, and runtime test run with `RALPH_CONFIG=custom.yml` still using defaults.

11. **Exit-code table in docs is out of sync with runtime behavior**
   - CLI reference lists fixed meanings (`2=config error`, `3=backend not found`, `4=interrupted`).
   - Actual run exits follow `TerminationReason` codes (e.g., `130` for interrupt, `2` for max-iteration/runtime/cost) and command-specific failures are not mapped to that table.
   - Why this matters: CI scripts that gate on documented return codes can break.
   - Evidence: `docs/guide/cli-reference.md:405-414`, `crates/ralph-core/src/event_loop/mod.rs:61-80`.