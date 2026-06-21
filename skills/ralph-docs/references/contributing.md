# Proposing a Ralph Code Change

Use this when the user wants to improve or fix something in the
`ralph-orchestrator` repo itself (not their own hat collection).

## Before Writing Code

1. **Confirm it's not already a feature.** Fetch the relevant doc page from
   llms.txt. Ralph is deliberately minimal; sometimes the answer is "use the
   existing knob".
2. **Confirm it doesn't violate a tenet.** Read
   `concepts/tenets/index.md`. The six tenets are load-bearing:
   fresh-context, backpressure, disposable plans, disk-is-state,
   steer-with-signals, let-Ralph-Ralph. Changes that fight these usually
   belong elsewhere.
3. **Map it to the right crate.** See the crate map below.

## Crate Map

```
ralph-cli      → CLI entry point; subcommands (run, plan, task, loops, web, hats)
ralph-core     → Orchestration logic: event loop, hats, memories, tasks, preset_source
ralph-adapters → Backend integrations (Claude, Kiro, Gemini, Codex, Roo, etc.)
ralph-telegram → Telegram bot for human-in-the-loop
ralph-tui      → Terminal UI (ratatui)
ralph-e2e      → End-to-end test framework
ralph-proto    → Protocol definitions
ralph-bench    → Benchmarking
ralph-api      → HTTP API server
backend/       → Web dashboard server (Fastify + tRPC + SQLite)
frontend/      → Web dashboard UI (React + Vite)
```

More precise file map (from the repo AGENTS.md):

| Subsystem | File |
|---|---|
| Event loop | `crates/ralph-core/src/event_loop/mod.rs` |
| Hat system | `crates/ralph-core/src/hatless_ralph.rs` |
| Memory | `crates/ralph-core/src/memory.rs`, `memory_store.rs` |
| Tasks | `crates/ralph-core/src/task.rs`, `task_store.rs` |
| Preset source (YAML + TOML) | `crates/ralph-core/src/preset_source.rs` |
| Lock coordination | `crates/ralph-core/src/worktree.rs` |
| Loop registry | `crates/ralph-core/src/loop_registry.rs` |
| Merge queue | `crates/ralph-core/src/merge_queue.rs` |
| CLI commands | `crates/ralph-cli/src/loops.rs`, `hats.rs`, `task_cli.rs`, `main.rs` |
| Backend selection | `crates/ralph-adapters/src/cli_backend.rs`, `auto_detect.rs` |
| ACP executor | `crates/ralph-adapters/src/acp_executor.rs` |
| Preflight | `crates/ralph-cli/src/preflight.rs` |
| Doctor | `crates/ralph-cli/src/doctor.rs` |

## Required Build/Test Sequence

Run these in order before opening a PR:

```bash
cargo fmt                                            # no warnings accepted
cargo clippy --all-targets --all-features -- -D warnings
cargo test -p ralph-core                             # loader + core tests
cargo test -p ralph-cli --bin ralph <your_module>::  # module-scoped
cargo test -p ralph-cli --test <integration_file>    # integration tests
./scripts/ci-rust-gate.sh                            # what CI actually runs
```

If the change is user-visible, also update:

- `docs/guide/<relevant>.md` (user-facing behavior change)
- `docs/reference/changelog/index.md` (add a line under Unreleased)
- Any preset YAML/TOML under `presets/` if the change affects them

## PR Conventions

- **Branch naming**: `feat/<topic>`, `fix/<topic>`, `docs/<topic>`.
- **Commit style**: conventional commits — `feat(cli):`, `fix(core):`,
  `docs(presets):`, etc. Keep the subject ≤72 chars.
- **Body**: explain *why*, not just *what*. Link to the issue / doc if there
  is one.
- **Merge mode**: squash. The repo squash-merges by default via
  `gh pr merge <N> --squash --delete-branch`.
- **Do not push directly to main**; open a PR even for trivial changes.

## Testing a Change Against a Real Ralph Loop

For behavioral changes (not cosmetics), run at least one end-to-end
iteration with a minimal preset against a real backend before merging:

```bash
cd /tmp/ralph-smoketest && mkdir -p x && cd x
echo "trivial task" > PROMPT.md
ralph run -H builtin:hatless-baseline -b claude -P PROMPT.md --max-iterations 2 -a -q
cat .ralph/events-*.jsonl
```

Confirm events fire and the termination reason is what you expect.

## When To Defer

- Pure surface polish (typos, stale doc links, formatting) → open a docs PR.
- Feature requests touching multiple crates → write a spec under `specs/` first
  and get alignment before coding.
- Anything changing the hat contract, event loop semantics, or disk layout
  (`.ralph/` structure) → read all six tenet docs, draft a spec, and expect
  discussion.

## After Merge

Sync local main, then reinstall:

```bash
git checkout main && git pull --rebase
cargo build --release --bin ralph
install -m 755 target/release/ralph ~/.cargo/bin/ralph
ralph --version
```

Use `install` instead of `cp` — macOS Gatekeeper caches the original
signature and can silently SIGKILL a cp'd binary whose contents changed.
