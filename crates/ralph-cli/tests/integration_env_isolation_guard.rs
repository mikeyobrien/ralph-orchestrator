//! Guards the test suite against leaking into a live ralph loop.
//!
//! `cargo test` often runs inside a ralph loop, whose `RALPH_EVENTS_FILE` and
//! `RALPH_WORKSPACE_ROOT` are in the environment. A test that spawns `ralph`
//! directly lets the child inherit them and write into the live loop. Every spawn
//! must go through `common::ralph_command()`, which strips inherited `RALPH_*`
//! variables. `scripts/check-test-event-isolation.sh` is the end-to-end check;
//! this test catches a new direct spawn before anyone has to run it.

mod common;

use std::fs;
use std::path::Path;

/// Spawning the binary under test without going through the helper. Split with
/// `concat!` so this file does not match its own patterns.
const DIRECT_SPAWNS: &[&str] = &[
    concat!("Command::new(env!(", "\"CARGO_BIN_EXE_ralph\"))"),
    concat!("Command::new(", "ralph_bin())"),
];

#[test]
fn integration_tests_spawn_ralph_only_through_the_shared_helper() {
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut offenders = Vec::new();

    for entry in fs::read_dir(&tests_dir).expect("read tests dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read test file");
        for (line_no, line) in source.lines().enumerate() {
            if DIRECT_SPAWNS.iter().any(|pattern| line.contains(pattern)) {
                offenders.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    line_no + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "spawn `ralph` with common::ralph_command() so the child does not inherit the \
         caller's loop env:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn ralph_command_does_not_pass_loop_env_to_the_child() {
    let temp = tempfile::TempDir::new().expect("temp dir");
    let explicit_events = temp.path().join("explicit.jsonl");

    // An explicitly set target must still win: the helper strips inherited vars only.
    let output = common::ralph_command()
        .args(["emit", "guard.topic", "payload"])
        .current_dir(temp.path())
        .env("RALPH_EVENTS_FILE", &explicit_events)
        .output()
        .expect("run ralph emit");
    assert!(
        output.status.success(),
        "ralph emit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let written = fs::read_to_string(&explicit_events).expect("explicit events file");
    assert!(written.contains("guard.topic"), "got: {written}");

    // Without an explicit target, the event lands in the test's own workspace.
    let output = common::ralph_command()
        .args(["emit", "guard.default", "payload"])
        .current_dir(temp.path())
        .output()
        .expect("run ralph emit");
    assert!(
        output.status.success(),
        "ralph emit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let default_events = temp.path().join(".ralph/events.jsonl");
    let written = fs::read_to_string(&default_events).unwrap_or_else(|err| {
        panic!(
            "expected the event in {}: {err}; inherited loop env leaked into the child",
            default_events.display()
        )
    });
    assert!(written.contains("guard.default"), "got: {written}");
}
