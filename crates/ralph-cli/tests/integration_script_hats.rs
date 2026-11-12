//! End-to-end tests for script hats: real `ralph run` with configs made of
//! script hats only. The configured backend is a sentinel that records any
//! launch, so these runs prove no model process starts.

#![cfg(unix)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::{Duration, Instant};
use tempfile::TempDir;

struct Workspace {
    dir: TempDir,
}

impl Workspace {
    /// A workspace whose `ralph.yml` is the sentinel backend plus `rest`.
    fn new(rest: &str) -> Self {
        let dir = TempDir::new().expect("temp dir");
        let ws = Self { dir };
        ws.script(
            "backend.sh",
            "cat >/dev/null\necho launched >> backend-ran.txt\nexit 1",
        );
        std::fs::write(
            ws.path().join("ralph.yml"),
            format!(
                "cli:\n  backend: custom\n  command: \"./backend.sh\"\n  prompt_mode: stdin\n{rest}"
            ),
        )
        .expect("write config");
        ws
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod");
        path
    }

    fn run(&self) -> Output {
        common::ralph_command()
            .args([
                "run",
                "--no-tui",
                "--skip-preflight",
                "--prompt",
                "script hats",
            ])
            .current_dir(self.path())
            .env("HOME", self.path())
            .env("USERPROFILE", self.path())
            .output()
            .expect("execute ralph")
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.path().join(name))
            .unwrap_or_else(|e| panic!("read {name}: {e}"))
    }

    fn exists(&self, name: &str) -> bool {
        self.path().join(name).exists()
    }

    /// The payload of the run's `loop.terminate` event.
    fn termination(&self) -> String {
        let marker = self.read(".ralph/current-events");
        let events = self.read(marker.trim());
        events
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|record| record["topic"] == "loop.terminate")
            .map(|record| record["payload"].as_str().unwrap_or_default().to_string())
            .next_back()
            .expect("loop.terminate logged")
    }

    /// `iteration.summary` payloads, in order.
    fn summaries(&self) -> Vec<serde_json::Value> {
        let marker = self.read(".ralph/current-events");
        self.read(marker.trim())
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|record| record["topic"] == "iteration.summary")
            .map(|record| serde_json::from_str(record["payload"].as_str().unwrap_or("{}")).unwrap())
            .collect()
    }
}

fn describe(output: &Output) -> String {
    format!(
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn script_hats_route_emitted_and_default_events_without_a_backend() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 1
hats:
  starter:
    name: "Starter"
    description: "Records its environment and emits step.a"
    triggers: ["work.start"]
    publishes: ["step.a"]
    run: ["./starter.sh"]
  quiet:
    name: "Quiet"
    description: "Emits nothing; default_publishes carries on"
    triggers: ["step.a"]
    publishes: ["step.b"]
    default_publishes: "step.b"
    run: "echo quiet ran >> quiet.txt"
  finisher:
    name: "Finisher"
    description: "Completes the loop"
    triggers: ["step.b"]
    publishes: ["LOOP_COMPLETE"]
    run: "\"$RALPH_BIN\" emit LOOP_COMPLETE \"done after $RALPH_TRIGGER_TOPIC\""
"#,
    );
    ws.script(
        "starter.sh",
        r#"{
  echo "hat=$RALPH_HAT"
  echo "iteration=$RALPH_ITERATION"
  echo "loop=$RALPH_LOOP_ID"
  echo "topic=$RALPH_TRIGGER_TOPIC"
  echo "payload=$RALPH_TRIGGER_PAYLOAD"
  echo "cwd=$(pwd)"
} > env.txt
cp "$RALPH_TRIGGER_EVENTS_FILE" triggers.jsonl
"$RALPH_BIN" emit step.a "from starter""#,
    );

    let output = ws.run();

    assert!(output.status.success(), "{}", describe(&output));
    assert!(
        !ws.exists("backend-ran.txt"),
        "no backend process may start"
    );

    let env = ws.read("env.txt");
    assert!(env.contains("hat=starter\n"), "{env}");
    assert!(env.contains("iteration=1\n"), "{env}");
    assert!(env.contains("loop=primary"), "{env}");
    assert!(env.contains("topic=work.start\n"), "{env}");
    assert!(env.contains("payload=script hats\n"), "{env}");
    let cwd = ws.path().canonicalize().unwrap();
    assert!(env.contains(&format!("cwd={}\n", cwd.display())), "{env}");

    let triggers: Vec<serde_json::Value> = ws
        .read("triggers.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0]["topic"], "work.start");
    assert_eq!(triggers[0]["payload"], "script hats");

    assert_eq!(ws.read("quiet.txt"), "quiet ran\n");
    assert!(
        ws.termination().contains("completed"),
        "{}",
        ws.termination()
    );

    // Three script iterations, past max_iterations: 1, each costing nothing.
    let summaries = ws.summaries();
    assert_eq!(summaries.len(), 3, "{summaries:?}");
    for summary in &summaries {
        assert_eq!(summary["backend"], "script", "{summary}");
        assert_eq!(summary["cost_usd"], 0.0, "{summary}");
    }
}

#[test]
fn failed_script_publishes_hat_failed_and_drops_its_emissions() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
hats:
  worker:
    name: "Worker"
    description: "Emits, then fails"
    triggers: ["work.start"]
    publishes: ["work.done"]
    run: "echo boom-on-stderr >&2; \"$RALPH_BIN\" emit work.done early; exit 3"
  leak:
    name: "Leak"
    description: "Must never run: work.done came from a failed script"
    triggers: ["work.done"]
    publishes: ["LOOP_COMPLETE"]
    run: "touch leaked.txt; \"$RALPH_BIN\" emit LOOP_COMPLETE leaked"
  reporter:
    name: "Reporter"
    description: "Records the failure and completes"
    triggers: ["worker.failed"]
    publishes: ["LOOP_COMPLETE"]
    run: "printf '%s' \"$RALPH_TRIGGER_PAYLOAD\" > failed.txt; \"$RALPH_BIN\" emit LOOP_COMPLETE reported"
"#,
    );

    let output = ws.run();

    assert!(output.status.success(), "{}", describe(&output));
    assert!(
        !ws.exists("backend-ran.txt"),
        "no backend process may start"
    );
    assert!(
        !ws.exists("leaked.txt"),
        "a failed script's events must not route"
    );
    let failed = ws.read("failed.txt");
    assert!(failed.starts_with("command exited 3"), "{failed}");
    assert!(failed.contains("1 emitted event(s) discarded"), "{failed}");
    assert!(failed.contains("boom-on-stderr"), "{failed}");
}

#[test]
fn script_exiting_zero_without_events_publishes_hat_failed() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
hats:
  silent:
    name: "Silent"
    description: "Exits 0 without emitting and has no default_publishes"
    triggers: ["work.start"]
    publishes: ["work.done"]
    run: "true"
  reporter:
    name: "Reporter"
    description: "Records the failure and completes"
    triggers: ["silent.failed"]
    publishes: ["LOOP_COMPLETE"]
    run: "printf '%s' \"$RALPH_TRIGGER_PAYLOAD\" > failed.txt; \"$RALPH_BIN\" emit LOOP_COMPLETE reported"
"#,
    );

    let output = ws.run();

    assert!(output.status.success(), "{}", describe(&output));
    assert!(!ws.exists("backend-ran.txt"));
    assert_eq!(ws.read("failed.txt"), "command exited 0 without emitting");
}

/// A script that emits only a promise did emit: it must not get
/// `<hat>.failed` ("exited 0 without emitting"), nor its `default_publishes`.
const SPURIOUS_FAILURE: &str = "Script hat failed";

#[test]
fn script_emitting_only_an_accepted_completion_promise_has_not_failed() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
hats:
  finisher:
    name: "Finisher"
    description: "Completes the loop"
    triggers: ["work.start"]
    publishes: ["LOOP_COMPLETE"]
    run: "\"$RALPH_BIN\" emit LOOP_COMPLETE done"
"#,
    );

    let output = ws.run();

    assert!(output.status.success(), "{}", describe(&output));
    assert!(
        ws.termination().contains("completed"),
        "{}",
        ws.termination()
    );
    assert!(
        !describe(&output).contains(SPURIOUS_FAILURE),
        "{}",
        describe(&output)
    );
}

#[test]
fn script_emitting_only_a_cancel_promise_has_not_failed() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
  cancellation_promise: "loop.cancel"
hats:
  stopper:
    name: "Stopper"
    description: "Cancels the loop"
    triggers: ["work.start"]
    publishes: ["loop.cancel"]
    run: "\"$RALPH_BIN\" emit loop.cancel stop"
"#,
    );

    let output = ws.run();

    assert!(output.status.success(), "{}", describe(&output));
    assert!(!ws.exists("backend-ran.txt"));
    assert!(
        ws.termination().contains("cancelled"),
        "{}",
        ws.termination()
    );
    assert!(
        !describe(&output).contains(SPURIOUS_FAILURE),
        "{}",
        describe(&output)
    );
}

#[test]
fn script_emitting_only_a_declined_completion_has_not_failed() {
    // review.passed is never seen, so the completion is declined and Ralph
    // takes over with task.resume. The script still emitted: no failure.
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
  required_events: ["review.passed"]
  max_iterations: 1
hats:
  finisher:
    name: "Finisher"
    description: "Tries to complete too early"
    triggers: ["work.start"]
    publishes: ["LOOP_COMPLETE"]
    run: "\"$RALPH_BIN\" emit LOOP_COMPLETE done"
"#,
    );

    let output = ws.run();

    assert!(
        !describe(&output).contains(SPURIOUS_FAILURE),
        "{}",
        describe(&output)
    );
    assert_ne!(ws.termination(), "", "the run must still end");
}

#[test]
fn declined_completion_does_not_inject_default_publishes() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
  required_events: ["review.passed"]
  max_iterations: 2
hats:
  finisher:
    name: "Finisher"
    description: "Tries to complete too early"
    triggers: ["work.start"]
    publishes: ["LOOP_COMPLETE", "step.fallback"]
    default_publishes: "step.fallback"
    run: "\"$RALPH_BIN\" emit LOOP_COMPLETE done"
  watcher:
    name: "Watcher"
    description: "Must never run: finisher did emit"
    triggers: ["step.fallback", "finisher.failed"]
    publishes: ["step.watched"]
    run: "touch watched.txt; \"$RALPH_BIN\" emit step.watched seen"
"#,
    );
    // Ralph handles task.resume through the backend; let it succeed quietly
    // so the watcher would get its turn if step.fallback were pending.
    ws.script(
        "backend.sh",
        "cat >/dev/null\necho launched >> backend-ran.txt\nexit 0",
    );

    let output = ws.run();

    assert!(!ws.exists("watched.txt"), "{}", describe(&output));
    assert!(ws.exists("backend-ran.txt"), "{}", describe(&output));
}
#[test]
fn script_timeout_kills_the_command_and_publishes_hat_failed() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
hats:
  slow:
    name: "Slow"
    description: "Outlives its timeout"
    triggers: ["work.start"]
    publishes: ["work.done"]
    timeout: 1
    run: "echo waiting >&2; sleep 30; \"$RALPH_BIN\" emit work.done late"
  reporter:
    name: "Reporter"
    description: "Records the failure and completes"
    triggers: ["slow.failed"]
    publishes: ["LOOP_COMPLETE"]
    run: "printf '%s' \"$RALPH_TRIGGER_PAYLOAD\" > failed.txt; \"$RALPH_BIN\" emit LOOP_COMPLETE reported"
"#,
    );

    let started = Instant::now();
    let output = ws.run();

    assert!(output.status.success(), "{}", describe(&output));
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "timeout did not stop the command: {:?}",
        started.elapsed()
    );
    let failed = ws.read("failed.txt");
    assert!(failed.starts_with("command timed out after 1s"), "{failed}");
    assert!(failed.contains("waiting"), "{failed}");
}

#[test]
fn repeated_script_failures_stop_at_max_consecutive_failures() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
  max_consecutive_failures: 2
hats:
  flaky:
    name: "Flaky"
    description: "Fails, and retries on its own failure"
    triggers: ["work.start", "flaky.failed"]
    publishes: ["work.done"]
    run: "echo \"$RALPH_ITERATION\" >> runs.txt; exit 1"
"#,
    );

    let output = ws.run();

    assert_eq!(output.status.code(), Some(1), "{}", describe(&output));
    assert!(!ws.exists("backend-ran.txt"));
    assert_eq!(ws.read("runs.txt"), "1\n2\n");
    let termination = ws.termination();
    assert!(
        termination.contains("consecutive_failures"),
        "{termination}"
    );
}

#[test]
fn script_hats_triggering_each_other_stop_at_max_script_runs() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
  max_script_runs: 6
hats:
  ping:
    name: "Ping"
    description: "Answers pong"
    triggers: ["work.start", "pong.done"]
    publishes: ["ping.done"]
    run: "echo ping >> runs.txt; \"$RALPH_BIN\" emit ping.done \"ping $RALPH_ITERATION\""
  pong:
    name: "Pong"
    description: "Answers ping"
    triggers: ["ping.done"]
    publishes: ["pong.done"]
    run: "echo pong >> runs.txt; \"$RALPH_BIN\" emit pong.done \"pong $RALPH_ITERATION\""
"#,
    );

    let output = ws.run();

    assert_eq!(output.status.code(), Some(2), "{}", describe(&output));
    assert!(!ws.exists("backend-ran.txt"));
    let termination = ws.termination();
    assert!(
        termination.contains("max_script_runs"),
        "{termination}\n{}",
        describe(&output)
    );
    assert_eq!(ws.read("runs.txt").lines().count(), 6);
}

#[test]
fn script_emitting_outside_publishes_is_a_scope_violation() {
    let ws = Workspace::new(
        r#"event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
  enforce_hat_scope: false
hats:
  stray:
    name: "Stray"
    description: "Emits a topic it does not publish"
    triggers: ["work.start"]
    publishes: ["work.done"]
    run: "\"$RALPH_BIN\" emit other.topic nope"
  other:
    name: "Other"
    description: "Must never run"
    triggers: ["other.topic"]
    publishes: ["LOOP_COMPLETE"]
    run: "touch leaked.txt; \"$RALPH_BIN\" emit LOOP_COMPLETE leaked"
  reporter:
    name: "Reporter"
    description: "Records the violation and completes"
    triggers: ["stray.scope_violation"]
    publishes: ["LOOP_COMPLETE"]
    run: "printf '%s' \"$RALPH_TRIGGER_PAYLOAD\" > violation.txt; \"$RALPH_BIN\" emit LOOP_COMPLETE reported"
"#,
    );

    let output = ws.run();

    assert!(output.status.success(), "{}", describe(&output));
    assert!(!ws.exists("leaked.txt"));
    assert!(ws.read("violation.txt").contains("other.topic"));
}
