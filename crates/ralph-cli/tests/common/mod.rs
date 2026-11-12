//! Shared helpers for ralph-cli integration tests.

use std::process::Command;

/// A `Command` for the `ralph` binary under test, with every inherited `RALPH_*`
/// variable removed.
///
/// When `cargo test` runs inside a ralph loop, the loop's `RALPH_EVENTS_FILE`,
/// `RALPH_WORKSPACE_ROOT`, wave and diagnostics variables are in the test process's
/// environment. A child `ralph` that inherits them writes into the live loop (stray
/// `test.topic` events, scratchpad entries) instead of the test's temp dir, and the
/// test's own assertions stop meaning anything. Tests that need one of these
/// variables set it explicitly with `.env(...)` after calling this.
///
/// Spawn `ralph` through this helper only: `integration_env_isolation_guard.rs`
/// rejects direct `Command::new(env!("CARGO_BIN_EXE_ralph"))` calls.
pub fn ralph_command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ralph"));
    for (key, _) in std::env::vars_os() {
        if key.to_str().is_some_and(|k| k.starts_with("RALPH_")) {
            cmd.env_remove(key);
        }
    }
    cmd
}
