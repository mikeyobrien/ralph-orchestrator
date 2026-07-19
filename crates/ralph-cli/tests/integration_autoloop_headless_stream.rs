#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const AUTOLOOPS_TOML: &str = r#"
[event_loop]
max_iterations = 2
completion_event = "task.complete"
"#;

const TOPOLOGY_TOML: &str = r#"
name = "headless-stream-test"
completion = "task.complete"

[[role]]
id = "planner"
emits = ["task.complete"]
prompt_file = "roles/planner.md"

[handoff]
"loop.start" = ["planner"]
"#;

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("execute git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn headless_run_streams_iteration_progress_before_autoloop_exits() {
    let workspace = tempfile::tempdir().expect("workspace temp dir");
    let home = tempfile::tempdir().expect("home temp dir");
    let preset = workspace.path().join("preset");
    fs::create_dir_all(preset.join("roles")).expect("create preset roles dir");
    fs::write(preset.join("autoloops.toml"), AUTOLOOPS_TOML).expect("write autoloops.toml");
    fs::write(preset.join("topology.toml"), TOPOLOGY_TOML).expect("write topology.toml");
    fs::write(preset.join("roles/planner.md"), "Complete the test.").expect("write role prompt");
    fs::write(
        workspace.path().join("ralph.yml"),
        "core:\n  engine: autoloop\n  autoloop_preset: preset\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n",
    )
    .expect("write ralph.yml");
    fs::write(workspace.path().join("README.md"), "headless stream test\n").expect("write README");
    run_git(workspace.path(), &["init", "--quiet"]);
    run_git(
        workspace.path(),
        &["add", "README.md", "ralph.yml", "preset"],
    );
    run_git(
        workspace.path(),
        &[
            "-c",
            "user.name=Ralph Test",
            "-c",
            "user.email=ralph@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );

    let bin_dir = workspace.path().join("bin");
    fs::create_dir(&bin_dir).expect("create fake bin dir");
    let fake_autoloop = bin_dir.join("autoloop");
    fs::write(
        &fake_autoloop,
        r#"#!/bin/sh
set -eu
: "${STREAM_READY:?STREAM_READY must be set}"
: "${STREAM_RELEASE:?STREAM_RELEASE must be set}"
events=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--events" ]; then
    shift
    events="$1"
    break
  fi
  shift
done
[ -n "$events" ]
printf '%s\n' '{"type":"iteration.banner","runId":"run-stream","iteration":1,"maxIterations":2,"allowedRoles":["planner"]}' >> "$events"
printf '%s\n' '{"type":"iteration.start","runId":"run-stream","iteration":1,"maxIterations":2}' >> "$events"
sleep 0.2
printf '%s\n' '{"type":"progress","runId":"run-stream","iteration":1,"allowedRoles":["planner"],"emittedTopic":"plan.ready","outcome":"continue:routed_event"}' >> "$events"
touch "$STREAM_READY"
while [ ! -e "$STREAM_RELEASE" ]; do
  sleep 0.01
done
printf '%s\n' '{"type":"loop.finish","runId":"run-stream","iterations":1,"stopReason":"completed","costUsd":0.01}' >> "$events"
cat <<'SUMMARY'
autoloops summary
===================
run_id: run-stream
iterations: 1
stop_reason: completed
cost_usd: 0.01
journal: /tmp/autoloop-stream-journal.jsonl
memory: /tmp/autoloop-stream-memory.jsonl
SUMMARY
"#,
    )
    .expect("write fake autoloop");
    let mut permissions = fs::metadata(&fake_autoloop)
        .expect("fake autoloop metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_autoloop, permissions).expect("make fake autoloop executable");

    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&inherited_path));
    let path = std::env::join_paths(paths).expect("construct PATH");
    let ready = workspace.path().join("stream-ready");
    let release = workspace.path().join("stream-release");

    let mut child = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "--color",
            "never",
            "--config",
            "ralph.yml",
            "run",
            "--no-tui",
            "--skip-preflight",
            "--max-iterations",
            "2",
            "-p",
            "stream progress",
        ])
        .current_dir(workspace.path())
        .env("PATH", path)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("STREAM_READY", &ready)
        .env("STREAM_RELEASE", &release)
        .env_remove("RALPH_CONFIG")
        .env_remove("RALPH_WORKSPACE_ROOT")
        .env_remove("RALPH_MERGE_LOOP_ID")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ralph");

    let stdout = child.stdout.take().expect("piped stdout");
    let (line_tx, line_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut lines = Vec::new();
        for line in BufReader::new(stdout).lines() {
            let line = line.expect("read stdout line");
            line_tx.send(line.clone()).expect("send stdout line");
            lines.push(line);
        }
        lines
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut progress_line = None;
    while Instant::now() < deadline {
        if let Ok(line) = line_rx.recv_timeout(Duration::from_millis(100))
            && line.contains("iteration 1/2")
            && line.contains("planner")
        {
            progress_line = Some(line);
            break;
        }
        if let Some(status) = child.try_wait().expect("poll ralph") {
            panic!("ralph exited before streaming progress: {status}");
        }
    }

    let Some(progress_line) = progress_line else {
        fs::write(&release, "release\n").expect("release fake autoloop after timeout");
        let _ = child.wait();
        panic!("timed out waiting for live iteration progress");
    };
    assert_eq!(
        child.try_wait().expect("poll ralph at progress line"),
        None,
        "progress must be visible before the subprocess exits: {progress_line}"
    );

    fs::write(&release, "release\n").expect("release fake autoloop");
    let status = child.wait().expect("wait for ralph");
    let lines = reader.join().expect("join stdout reader");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("piped stderr")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    assert!(
        status.success(),
        "ralph failed; stdout={lines:?}; stderr={stderr}"
    );
}
