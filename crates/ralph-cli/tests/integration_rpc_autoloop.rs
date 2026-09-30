//! Subprocess-level contract tests for `ralph run --rpc` on the autoloop engine
//! (#343).
//!
//! Unlike the mapper-level integration test in ralph-adapters (which drives
//! `AutoloopEventTailer` + `AutoloopRpcMapper` in-process), these tests spawn
//! the **real `ralph` binary** in RPC mode against a **fake `autoloop`
//! subprocess** and assert the JSON-lines `RpcEvent` protocol on the ralph
//! process's stdout from the consumer's side. This covers:
//!
//! - the documented `RpcEvent` contract (event names, payload fields,
//!   ordering) as observed by an external subprocess consumer;
//! - the full `LoopStarted -> per-iteration -> LoopTerminated` lifecycle
//!   through the actual engine path (`run_autoloop_engine` →
//!   `run_autoloop_with_rpc`), including the final-drain + `finalize()`
//!   flush the coarse `--events` stream depends on;
//! - the deliberate Autoloop→RPC mapping (stop-reason collapse, HITL via
//!   `OrchestrationEvent`);
//! - cancel/control: a group signal to ralph mid-run tears down the whole
//!   subprocess tree without a hang.
//!
//! The fake `autoloop` is a POSIX shell script placed first on the test's
//! `PATH`; it honors the `--events <path>` argument exactly like the real
//! binary (appending the scripted NDJSON stream to that file), prints the
//! `autoloops summary` block the runner parses, and exits 0. It needs no
//! node, no real autoloop checkout, and no backend — so the tests run in any
//! worktree/CI.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const PROMPT: &str = "add a footer to index.html";

/// The fake autoloop binary. Reads `--events <path>` from argv, writes the
/// scripted NDJSON stream (from the `FAKE_AUTOLOOP_SCRIPT` env var) to that
/// file, optionally sleeps (`FAKE_AUTOLOOP_SLEEP_SECS`) to simulate a long
/// run, prints the `autoloops summary` block, and exits 0.
fn fake_autoloop_script() -> String {
    r#"#!/usr/bin/env sh
events_path=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--events" ]; then
    events_path="$arg"
  fi
  prev="$arg"
done
if [ -n "$events_path" ] && [ -n "$FAKE_AUTOLOOP_SCRIPT" ]; then
  : > "$events_path"
  printf '%s\n' "$FAKE_AUTOLOOP_SCRIPT" >> "$events_path"
fi
if [ -n "$FAKE_AUTOLOOP_SLEEP_SECS" ]; then
  sleep "$FAKE_AUTOLOOP_SLEEP_SECS"
fi
cat <<EOF
autoloops summary
=================
run_id: r1
iterations: 2
stop_reason: ${STOP_REASON:-completed}
cost_usd: 0.05
journal: ${AUTOLOOP_JOURNAL_FILE:-${work:-/tmp}/.autoloop/journal.jsonl}
memory: ${AUTOLOOP_MEMORY_FILE:-${work:-/tmp}/.autoloop/memory.jsonl}
EOF
exit 0
"#
    .to_string()
}

/// A scripted events stream for a clean two-iteration run that completes.
const HAPPY_EVENTS: &str = r#"{"type":"iteration.start","iteration":1,"maxIterations":2,"runId":"r1"}
{"type":"progress","runId":"r1","iteration":1,"emittedTopic":"tasks.ready","outcome":"continue:routed_event","allowedRoles":["coder"]}
{"type":"ask.pending","runId":"r1","iteration":1,"questionId":"q1","question":"Proceed?"}
{"type":"ask.answered","runId":"r1","questionId":"q1","answer":"yes"}
{"type":"backend.output","runId":"r1","iteration":1,"output":"iteration one"}
{"type":"iteration.start","iteration":2,"maxIterations":2,"runId":"r1"}
{"type":"backend.output","runId":"r1","iteration":2,"output":"iteration two"}
{"type":"summary","runId":"r1","iterations":2,"stopReason":"completed","costUsd":0.05}
{"type":"loop.finish","runId":"r1","iterations":2,"stopReason":"completed","costUsd":0.05}"#;

/// One early iteration, then the fake autoloop sleeps (still alive).
const EARLY_EVENT: &str =
    r#"{"type":"iteration.start","iteration":1,"maxIterations":5,"runId":"r1"}"#;

struct Harness {
    _dir: tempfile::TempDir,
    /// The temp workspace ralph runs in (cwd).
    work: PathBuf,
    /// The fake `autoloop` executable, first in the test's PATH.
    fake: PathBuf,
}

/// Build the isolated workspace: a git repo (ralph wants one), the PROMPT.md
/// the engine reads, a ralph.yml pointed at a local autoloop preset, and a
/// fake `autoloop` binary on PATH.
fn setup() -> Harness {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let work = dir.path().to_path_buf();

    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "t@example.com"],
        vec!["config", "user.name", "t"],
    ] {
        let status = Command::new("git")
            .args(&args)
            .current_dir(&work)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?} failed");
    }

    // The engine reads the prompt from `config.event_loop.prompt_file`; drive
    // it the way users do for file-based runs: a PROMPT.md the config points
    // at (the default prompt_file, so no -p needed on the CLI).
    fs::write(work.join("PROMPT.md"), PROMPT).unwrap();

    // autoloop preset dir (only needs autoloops.toml to exist).
    fs::create_dir_all(work.join("preset/roles")).unwrap();
    fs::write(work.join("preset/autoloops.toml"), "name = \"test\"\n").unwrap();

    // ralph.yml: explicit preset + the default prompt file.
    let config = "
core:
  autoloop_preset: preset
event_loop:
  max_iterations: 2
  completion_promise: LOOP_COMPLETE
cli:
  backend: claude
";
    let config_path = work.join("ralph.yml");
    fs::write(&config_path, config).unwrap();

    // Fake autoloop, placed first on the child's PATH.
    let fake = work.join("bin/autoloop");
    fs::create_dir_all(work.join("bin")).unwrap();
    fs::write(&fake, fake_autoloop_script()).unwrap();
    let mut perms = fs::metadata(&fake).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake, perms).unwrap();

    Harness {
        _dir: dir,
        work,
        fake,
    }
}

/// Spawn `ralph run --rpc` in the harness workspace. The child gets its own
/// process group (like a user's terminal foreground group) so group signals
/// in the cancel test reach ralph + its subprocess tree and nothing else.
fn spawn_ralph(h: &Harness, extra_env: &[(&str, &str)]) -> std::process::Child {
    let config_path = h.work.join("ralph.yml");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ralph"));
    cmd.args([
        "run",
        "--rpc",
        "--skip-preflight",
        "-c",
        config_path.to_str().unwrap(),
    ])
    .current_dir(&h.work)
    .env(
        "PATH",
        format!(
            "{}:{}",
            h.work.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    )
    .env("RALPH_CONFIG", &config_path)
    .env("RUST_LOG", "off")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    for (key, value) in extra_env {
        cmd.env(key, value);
    }
    cmd.spawn().expect("spawn ralph")
}

/// Wait for the child up to `timeout`, hard-killing its process group on
/// expiry so a hung test never blocks the suite.
fn wait_with_timeout(mut child: std::process::Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().expect("collect output"),
            Ok(None) => {
                if Instant::now() >= deadline {
                    #[cfg(unix)]
                    {
                        use nix::sys::signal::{Signal, killpg};
                        use nix::unistd::Pid;
                        let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
                    }
                    let _ = child.kill();
                    return child.wait_with_output().expect("collect output after kill");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("try_wait: {e}"),
        }
    }
}

/// Split ralph's stdout into parsed `RpcEvent`s, asserting the protocol
/// invariants an external consumer depends on: every line is valid JSON and a
/// `type`-tagged object, and the stream is non-empty.
fn parse_rpc_stream(stdout: &str) -> Vec<serde_json::Value> {
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "RPC stream was empty:\n{stdout}");
    let mut events = Vec::new();
    for line in lines {
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("stdout line is not valid JSON ({e}): {line:?}"));
        assert!(
            v.get("type").is_some_and(|t| t.is_string()),
            "every RPC line must carry a `type` tag: {line:?}"
        );
        events.push(v);
    }
    events
}

fn types(events: &[serde_json::Value]) -> Vec<String> {
    events
        .iter()
        .map(|e| e["type"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn rpc_run_emits_the_contracted_event_lifecycle() {
    let h = setup();
    let child = spawn_ralph(
        &h,
        &[
            ("FAKE_AUTOLOOP_SCRIPT", HAPPY_EVENTS),
            ("STOP_REASON", "completed"),
        ],
    );
    let out = wait_with_timeout(child, Duration::from_secs(60));

    assert!(
        out.status.success(),
        "ralph run --rpc should exit 0; status={:?}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let events = parse_rpc_stream(&stdout);
    let ts = types(&events);

    // 1) The stream opens with loop_started and closes with exactly one
    //    loop_terminated (the contract's lifecycle envelope).
    assert_eq!(ts.first().unwrap(), "loop_started", "stream: {ts:?}");
    let terminated: Vec<&serde_json::Value> = events
        .iter()
        .filter(|e| e["type"] == "loop_terminated")
        .collect();
    assert_eq!(terminated.len(), 1, "exactly one terminal event: {ts:?}");
    assert_eq!(ts.last().unwrap(), "loop_terminated");

    // 2) loop_started carries the engine-side framing the --events stream
    //    doesn't have: the prompt from PROMPT.md, the configured max
    //    iterations, and the backend label.
    let started = &events[0];
    assert_eq!(started["prompt"], PROMPT);
    assert_eq!(started["max_iterations"], 2);
    assert!(started["started_at"].is_u64());
    // backend: ralph's configured name is echoed (see rpc_backend_label).
    assert!(
        started["backend"] == "claude" || started["backend"] == "autoloop",
        "backend label: {}",
        started["backend"]
    );

    // 3) Per-iteration events: two iteration_start events (1, 2), the second
    //    attributed to the role the last progress named; one text_delta per
    //    backend.output, attributed to the active iteration.
    let starts: Vec<u32> = events
        .iter()
        .filter(|e| e["type"] == "iteration_start")
        .map(|e| e["iteration"].as_u64().unwrap() as u32)
        .collect();
    assert_eq!(starts, vec![1, 2], "stream: {ts:?}");
    let second_start = events
        .iter()
        .find(|e| e["type"] == "iteration_start" && e["iteration"] == 2)
        .unwrap();
    assert_eq!(
        second_start["hat"], "coder",
        "role from the last progress event"
    );
    assert_eq!(second_start["max_iterations"], 2);

    let deltas: Vec<(u64, &str)> = events
        .iter()
        .filter(|e| e["type"] == "text_delta")
        .map(|e| {
            (
                e["iteration"].as_u64().unwrap(),
                e["delta"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(deltas, vec![(1, "iteration one"), (2, "iteration two")]);

    // 4) HITL rides the orchestration_event channel, ask before answered.
    let asks: Vec<&serde_json::Value> = events
        .iter()
        .filter(|e| e["type"] == "orchestration_event" && e["topic"] == "human.ask")
        .collect();
    let answered: Vec<&serde_json::Value> = events
        .iter()
        .filter(|e| e["type"] == "orchestration_event" && e["topic"] == "human.answered")
        .collect();
    assert_eq!(asks.len(), 1);
    assert_eq!(asks[0]["payload"], "Proceed?");
    assert_eq!(asks[0]["source"], "q1");
    assert_eq!(answered.len(), 1);
    assert_eq!(answered[0]["payload"], "yes");
    let ask_pos = events
        .iter()
        .position(|e| e["topic"] == "human.ask")
        .unwrap();
    let answered_pos = events
        .iter()
        .position(|e| e["topic"] == "human.answered")
        .unwrap();
    assert!(ask_pos < answered_pos, "ask precedes answered");

    // 5) The terminal carries the authoritative run result: stopReason
    //    `completed` collapses to `completed` and the cost from loop.finish.
    let terminal = terminated[0];
    assert_eq!(terminal["reason"], "completed");
    assert_eq!(terminal["total_iterations"], 2);
    let cost = terminal["total_cost_usd"].as_f64().unwrap();
    assert!(
        (cost - 0.05).abs() < 1e-9,
        "terminal cost from loop.finish, got {cost}"
    );
    assert!(terminal["terminated_at"].is_u64());
    assert!(terminal["duration_ms"].is_u64());

    // 6) stdout is protocol-clean: no log noise, no human-readable summary
    //    line (that's suppressed in --rpc mode).
    for line in stdout.lines() {
        assert!(
            serde_json::from_str::<serde_json::Value>(line).is_ok(),
            "non-JSON on the RPC stdout: {line:?}"
        );
        assert!(!line.contains("autoloops summary"), "{line:?}");
    }
}

/// Cancel/control: the consumer sends a group signal to ralph mid-run (the
/// Ctrl+C equivalent: ralph's subprocess child shares its process group, so
/// the whole tree must go down without a hang) and whatever the stream
/// emitted up to that point stays well-formed.
#[cfg(unix)]
#[test]
fn rpc_run_cancel_takes_down_the_subprocess_tree() {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;

    let h = setup();
    let child = spawn_ralph(
        &h,
        &[
            ("FAKE_AUTOLOOP_SCRIPT", EARLY_EVENT),
            ("FAKE_AUTOLOOP_SLEEP_SECS", "60"),
        ],
    );
    let ralph_pid = child.id() as i32;

    // Give ralph time to reach the engine and spawn the (sleeping) fake
    // autoloop, then interrupt ralph's whole process group.
    std::thread::sleep(Duration::from_secs(4));
    killpg(Pid::from_raw(ralph_pid), Signal::SIGINT).expect("SIGINT to ralph's group");

    // Bounded wait: the group must go down promptly (no orphaned 60s child
    // keeping the wait open).
    let mut child = child;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let output = {
                        let _ = child.kill();
                        child.wait_with_output().expect("collect")
                    };
                    panic!(
                        "ralph did not exit after SIGINT (hung cancel path)\nstderr:\n{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("try_wait: {e}"),
        }
    }

    // The fake autoloop (a 60s sleeper in the same group) must be gone too —
    // no orphaned backend process.
    std::thread::sleep(Duration::from_millis(500));
    let orphans = Command::new("pgrep")
        .arg("-f")
        .arg(h.fake.display().to_string())
        .output()
        .expect("pgrep");
    assert!(
        String::from_utf8_lossy(&orphans.stdout).trim().is_empty(),
        "orphaned fake autoloop survived the cancel: {}",
        String::from_utf8_lossy(&orphans.stdout)
    );
}
