//! Subprocess-TUI-consumer tests for the autoloop engine (#342/#343): the
//! exact event reader the in-process TUI runs (`run_autoloop_event_reader`)
//! driven by a **real subprocess** producing the `--events` NDJSON stream.
//!
//! This is the consumer-side counterpart to `crates/ralph-cli`'s
//! `--rpc` subprocess tests: where RPC tests read `RpcEvent` JSON lines off
//! stdout, these feed the same stream into `TuiState` and assert the state
//! the TUI would render — without a tty, a render loop, or any other brittle
//! UI assumption (state inspection, not pixels).
//!
//! The fake producer is a POSIX shell script appended to the events file as a
//! subprocess, then either exits cleanly (happy path) or is killed mid-run
//! (crash/cancel path) — mirroring what happens when `ralph run` kills the
//! autoloop child on user quit.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ralph_tui::{TuiState, run_autoloop_event_reader};
use tokio::sync::watch;

/// The consumer under test, mirroring `run_autoloop_with_tui`'s wiring:
/// a `TuiState` marked `autoloop_source` + the async event reader task.
fn start_consumer(
    events_path: PathBuf,
    state: Arc<Mutex<TuiState>>,
) -> (watch::Sender<bool>, tokio::task::JoinHandle<()>) {
    let state = state.clone();
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let handle = tokio::spawn(async move {
        // The evolved reader contract (#342) carries workspace/engine-state
        // roots and role display names; this consumer passes empty roots and
        // no role names (none are emitted by the scripted stream).
        run_autoloop_event_reader(
            events_path,
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            state,
            cancel_rx,
            std::collections::HashMap::new(),
        )
        .await;
    });
    (cancel_tx, handle)
}

/// Poll the state lock until `f` holds or the deadline passes.
fn wait_for(state: &Arc<Mutex<TuiState>>, what: &str, f: impl Fn(&TuiState) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if f(&state.lock().unwrap()) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Write + chmod a fake producer script that appends each line of
/// `events` to `--events <path>` (the same contract the real autoloop
/// honors) and then either exits or, when `sleep_secs > 0`, keeps living.
fn write_fake_producer(bin_dir: &std::path::Path, events: &str, sleep_secs: u32) -> PathBuf {
    let body = events
        .lines()
        .map(|l| format!("printf '%s\\n' '{l}' >> \"$events_path\""))
        .collect::<Vec<_>>()
        .join("\n");
    let sleep = if sleep_secs > 0 {
        format!("sleep {sleep_secs}")
    } else {
        String::new()
    };
    let script = format!(
        r#"#!/usr/bin/env sh
prev=""
for arg in "$@"; do
  if [ "$prev" = "--events" ]; then
    events_path="$arg"
  fi
  prev="$arg"
done
{body}
{sleep}
exit 0
"#
    );
    let path = bin_dir.join("fake-producer");
    fs::write(&path, script).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

fn spawn_producer(script: &PathBuf, events_path: &PathBuf) -> std::process::Child {
    Command::new(script)
        .arg("run")
        .arg("/tmp/preset")
        .arg("test prompt")
        .arg("--events")
        .arg(events_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn fake producer")
}

#[tokio::test]
async fn tui_consumer_renders_a_completed_autoloop_run_from_a_subprocess() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join("autoloop-events.ndjson");
    let producer = write_fake_producer(
        dir.path(),
        r#"{"type":"iteration.start","iteration":1,"maxIterations":2,"runId":"r1"}
{"type":"progress","runId":"r1","iteration":1,"emittedTopic":"build.task","outcome":"continue:routed_event","allowedRoles":["coder"]}
{"type":"backend.output","runId":"r1","iteration":1,"output":"iteration one output"}
{"type":"iteration.start","iteration":2,"maxIterations":2,"runId":"r1"}
{"type":"iteration.banner","iteration":2,"runId":"r1","allowedRoles":["coder"]}
{"type":"backend.output","runId":"r1","iteration":2,"output":"iteration two output"}
{"type":"loop.finish","runId":"r1","iterations":2,"stopReason":"completed","costUsd":0.08}"#,
        0,
    );

    let state = Arc::new(Mutex::new({
        let mut s = TuiState::new();
        s.autoloop_source = true;
        s
    }));
    let (cancel_tx, reader) = start_consumer(events_path.clone(), state.clone());

    let mut child = spawn_producer(&producer, &events_path);
    child.wait().expect("producer exits");

    // The reader final-drains on cancel; signal it now that the producer is
    // done — exactly what the TUI path does when the subprocess exits. Await
    // the reader so the (single-threaded test) runtime drives its final drain
    // to completion before we inspect the resulting state.
    cancel_tx.send(true).unwrap();
    reader.await.expect("reader task returns");

    // The consumer must converge: 2 iterations, the terminal cost, loop
    // completed, and the per-iteration content it would render.
    wait_for(&state, "loop_completed", |s| s.loop_completed);

    let s = state.lock().unwrap();
    assert_eq!(s.iteration, 2, "final iteration number");
    assert_eq!(s.max_iterations, Some(2));
    assert_eq!(s.total_iterations(), 2, "two iteration buffers");
    assert_eq!(
        s.iterations[1].hat_display.as_deref(),
        Some("coder"),
        "second iteration labelled with the role from the last progress event"
    );
    assert_eq!(s.final_cost_usd, Some(0.08));
    assert!(
        s.final_loop_elapsed.is_some(),
        "elapsed frozen on completion"
    );
    let mut all_text = String::new();
    for it in &s.iterations {
        let lines = it.lines.lock().unwrap();
        for line in lines.iter() {
            all_text.push_str(&line.to_string());
        }
    }
    drop(s);
    assert!(all_text.contains("iteration one output"));
    assert!(all_text.contains("iteration two output"));
    // Evolved #342 reader contract: the routing line lives in the iteration's
    // live region and is reconciled away when the authoritative
    // backend.output arrives — verify reconciliation, not retention.
    assert!(
        !all_text.contains("\u{2192} build.task"),
        "live-region routing line must be reconciled away on backend.output"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn tui_consumer_survives_a_killed_producer_and_reports_the_missing_result() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join("autoloop-events.ndjson");
    let producer = write_fake_producer(
        dir.path(),
        r#"{"type":"iteration.start","iteration":1,"maxIterations":5,"runId":"r1"}"#,
        60, // keeps living until we kill it
    );

    let state = Arc::new(Mutex::new({
        let mut s = TuiState::new();
        s.autoloop_source = true;
        s
    }));
    let (cancel_tx, reader) = start_consumer(events_path.clone(), state.clone());

    let mut child = spawn_producer(&producer, &events_path);
    // Let it write the one early event.
    std::thread::sleep(Duration::from_millis(300));
    // Simulate ralph killing the autoloop child on user quit: a hard kill,
    // so no terminal event ever arrives. (Child::kill is SIGKILL on unix.)
    child.kill().expect("kill the producer subprocess");
    child.wait().expect("reap the producer");
    // Signal the reader the way the TUI path does after the kill.
    cancel_tx.send(true).unwrap();

    // The reader must return (no hang) and the consumer must surface that
    // the run ended without a result — the same graceful degradation a real
    // TUI user sees after Ctrl+C.
    reader.await.expect("reader task returns without panicking");

    wait_for(&state, "loop_completed", |s| s.loop_completed);
    let s = state.lock().unwrap();
    let all_text = {
        let mut text = String::new();
        for it in &s.iterations {
            let lines = it.lines.lock().unwrap();
            for line in lines.iter() {
                text.push_str(&line.to_string());
            }
        }
        text
    };
    assert!(
        all_text.contains("run ended before reporting a result"),
        "missing-result line rendered: {all_text:?}"
    );
    assert_eq!(s.iteration, 1);
    assert!(
        s.final_cost_usd.is_none(),
        "no cost when the run never finished"
    );
}
