---
name: dogfood-ralph
description: "Dogfood ralph on this repo: run a ralph loop to build a ralph spec or feature, with this repo's setup and test gotchas. Use when asked to dogfood or to have ralph implement ralph work."
metadata:
  internal: true
---

# Dogfood ralph on this repo

Launching, monitoring, measuring, and stepping in follow `skills/ralph-loop/references/headless-runs.md`. This skill adds only what is specific to running ralph inside its own repository.

## Setup

1. Cut a worktree for the work: `git worktree add ../ralph-<name> -b <branch> origin/main`. Commit the spec the loop will implement into that branch. This repo's `CLAUDE.md` says to implement only from an approved spec, so the loop needs it on disk.
2. Build the ralph that will run the loop, and copy the binary out of `target/`, so the loop's own `cargo build` can't swap it mid-run: `cargo build --release -p ralph-cli && cp target/release/ralph <outside>/ralph`.
3. Put the core config (`cli.backend`, `event_loop` limits) and `PROMPT.md` outside the worktree, and pass them with `-c` and `-P`.

**Done when** `<outside>/ralph doctor -c <outside>/run.yml -H builtin:<preset>` passes in the worktree and the spec commit is on the branch.

## Write these into the loop's prompt

The loop's hats read the prompt, not this skill:
- The verification commands: `cargo test -p ralph-core`, `cargo test -p ralph-adapters`, `cargo test -p ralph-cli -- --test-threads=1`, `cargo clippy --workspace --all-targets`, `cargo fmt --all --check`, and `./scripts/sync-embedded-files.sh check` after preset or `crates/ralph-core/data/` edits.
- That ralph-cli's wave and backend tests race on `PATH` when run in parallel, which also happens on `main`. Without this, the Critic rejects for failures that aren't regressions.

## While it runs

Tests that call `ralph emit` without clearing `RALPH_EVENTS_FILE` write stray topics (`test.topic`, `t.x`, `t.y`) into the live loop's events file whenever a hat runs the suite. They route nowhere. Filter them out of your monitor.
