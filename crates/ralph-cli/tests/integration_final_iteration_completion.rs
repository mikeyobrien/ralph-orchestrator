//! Completion on the last allowed iteration must win over the iteration limit.
//!
//! Regression tests for #369: when the final allowed iteration completes the
//! loop, the run must end as completed (exit 0), not as `max_iterations`
//! (exit 2). Covers both ways a backend can complete: emitting the completion
//! event with `ralph emit`, and printing the promise as its final output line.

mod common;

use std::path::Path;
use std::process::Output;
use tempfile::TempDir;

fn write_backend(dir: &Path, body: &str) {
    let script = dir.join("backend.sh");
    std::fs::write(&script, format!("#!/bin/sh\ncat >/dev/null\n{body}\n"))
        .expect("write backend script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod backend script");
    }
}

fn run_one_iteration(backend_body: &str) -> Output {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();
    write_backend(temp_path, backend_body);
    std::fs::write(
        temp_path.join("ralph.yml"),
        r#"
cli:
  backend: custom
  command: "./backend.sh"
  prompt_mode: stdin
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 1
  max_runtime_seconds: 30
"#,
    )
    .expect("write config");

    let output = common::ralph_command()
        .args([
            "run",
            "--no-tui",
            "--skip-preflight",
            "--prompt",
            "finish now",
        ])
        .current_dir(temp_path)
        .output()
        .expect("execute ralph");
    // Keep the temp dir alive until the process has exited.
    drop(temp_dir);
    output
}

fn describe(output: &Output) -> String {
    format!(
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn completion_event_on_final_iteration_completes_the_loop() {
    let output = run_one_iteration(&format!(
        "\"{}\" emit LOOP_COMPLETE \"done on the last iteration\"",
        env!("CARGO_BIN_EXE_ralph")
    ));
    assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
}

#[test]
fn completion_text_on_final_iteration_completes_the_loop() {
    let output = run_one_iteration("printf 'all work finished\\nLOOP_COMPLETE\\n'");
    assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
}

#[test]
fn final_iteration_without_completion_still_stops_at_max_iterations() {
    let output = run_one_iteration("printf 'still working\\n'");
    assert_eq!(output.status.code(), Some(2), "{}", describe(&output));
}

#[test]
fn hat_completion_event_on_final_iteration_completes_the_loop() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();
    write_backend(
        temp_path,
        &format!(
            "\"{}\" emit LOOP_COMPLETE \"finalizer done on the last iteration\"",
            env!("CARGO_BIN_EXE_ralph")
        ),
    );
    std::fs::write(
        temp_path.join("ralph.yml"),
        r#"
cli:
  backend: custom
  command: "./backend.sh"
  prompt_mode: stdin
event_loop:
  completion_promise: "LOOP_COMPLETE"
  starting_event: "work.start"
  max_iterations: 1
  max_runtime_seconds: 30
hats:
  finalizer:
    name: "Finalizer"
    description: "Completes the loop"
    triggers: ["work.start"]
    publishes: ["LOOP_COMPLETE"]
    instructions: "Publish LOOP_COMPLETE."
"#,
    )
    .expect("write config");

    let output = common::ralph_command()
        .args([
            "run",
            "--no-tui",
            "--skip-preflight",
            "--prompt",
            "finish now",
        ])
        .current_dir(temp_path)
        .output()
        .expect("execute ralph");
    assert_eq!(output.status.code(), Some(0), "{}", describe(&output));
}
