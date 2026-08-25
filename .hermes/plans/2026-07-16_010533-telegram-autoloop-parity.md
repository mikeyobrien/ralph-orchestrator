# Telegram Autoloop Parity Implementation Plan

> **For Hermes:** Implement this plan directly in the assigned issue-345 worktree; do not mutate live Telegram.

**Goal:** Restore daemon-started Telegram/Web RObot HITL and proactive guidance when Ralph runs Autoloop as its engine.

**Architecture:** Keep Autoloop as a subprocess. Ralph starts the configured `RobotService`, tails Autoloop's structured events for `ask.pending`, relays questions through the service, and sends answers back through Autoloop's `control respond` verb. A dedicated Ralph human-event file remains the inbound surface for Telegram/Web responses and guidance; control invocations reuse the exact Autoloop binary configured by `AutoloopRunner`.

**Tech Stack:** Rust, Tokio, `ralph-proto::RobotService`, `ralph-telegram`, Autoloop CLI control protocol.

---

### Task 1: Add testable Autoloop control invocations

**Files:**
- Modify: `crates/ralph-adapters/src/autoloop_runner.rs`

Add `control respond`, `control guide`, and `control interrupt` methods that reuse `AutoloopRunner`'s binary selection, working directory, and environment. Add unit tests proving exact argv shape and a fake-executable test proving answer/guidance delivery without network access.

### Task 2: Add the RObot/Autoloop bridge

**Files:**
- Create: `crates/ralph-cli/src/autoloop_robot.rs`
- Modify: `crates/ralph-cli/src/main.rs`

Create/start Telegram or Web services from `RalphConfig`. Tail structured Autoloop events exactly once, relay `ask.pending`, wait on the dedicated human-events file, and invoke `control respond`. Forward proactive `human.guidance` lines with `control guide`. Stop the service cooperatively when the subprocess exits. Unit-test with a fake `RobotService` and fake control executable.

### Task 3: Wire bridge into the headless Autoloop engine

**Files:**
- Modify: `crates/ralph-cli/src/autoloop_engine.rs`
- Modify: `crates/ralph-cli/src/web_robot_service.rs`

When `RObot.enabled` on the primary loop, spawn Autoloop and the bridge concurrently instead of using the plain blocking runner. Preserve the non-RObot/TUI paths. Replace stale cutover comments. Ensure the current-events marker points at the human-event file during the run and is restored afterward.

### Task 4: Mock-backed daemon/HITL proof

**Files:**
- Modify/Create tests near `crates/ralph-cli/src/autoloop_robot.rs` and `crates/ralph-adapters/src/autoloop_runner.rs`

Exercise `ask.pending -> send question -> human.response -> control respond` and proactive guidance with local files/fake executable only. Assert no duplicate question/control delivery and graceful shutdown.

### Task 5: Verification

Run:
- `cargo fmt --check`
- `cargo test -p ralph-adapters autoloop_runner`
- `cargo test -p ralph-cli autoloop_robot`
- `cargo test -p ralph-telegram`
- `cargo test -p ralph-core smoke_runner`
- `cargo test`

Inspect `git diff --check` and `git diff --stat`. Do not commit, push, close the issue, or contact live Telegram.
