//! Event loop tests for event gates: blocked rewriting, completion gating,
//! wave and robot-wait paths, and unchanged behavior without gates.

use super::*;
use crate::gates::{GateCommandOutcome, GateCommandRequest, GateCommandRunner, StartHead};
use std::process::Command;
use std::sync::Mutex;
use tempfile::TempDir;

const HATS: &str = r#"
hats:
  builder:
    name: "Builder"
    description: "Builds"
    triggers: ["build.task", "foundation.blocked", "review.blocked", "merge.blocked"]
    publishes: ["foundation.done", "review.ready", "merge.done"]
  reviewer:
    name: "Reviewer"
    description: "Reviews"
    triggers: ["foundation.done", "review.ready", "merge.done"]
    publishes: ["review.passed"]
"#;

/// Gate runner that records requests and returns a fixed outcome.
struct RecordingRunner {
    outcome: GateCommandOutcome,
    calls: Arc<Mutex<Vec<GateCommandRequest>>>,
}

impl GateCommandRunner for RecordingRunner {
    fn run(&self, request: GateCommandRequest) -> Result<GateCommandOutcome, String> {
        self.calls.lock().unwrap().push(request);
        Ok(self.outcome.clone())
    }
}

struct Harness {
    dir: TempDir,
    event_loop: EventLoop,
    events_path: PathBuf,
}

impl Harness {
    /// An event loop over a fresh git repo, with `extra_yaml` (gates, event
    /// loop settings) appended to the builder/reviewer hats.
    fn new(extra_yaml: &str) -> Self {
        Self::with_hats(HATS, extra_yaml)
    }

    fn with_hats(hats_yaml: &str, extra_yaml: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut config: RalphConfig =
            serde_yaml::from_str(&format!("{hats_yaml}{extra_yaml}")).unwrap();
        config.core.workspace_root = dir.path().to_path_buf();
        config.validate().expect("test config is valid");
        let diagnostics =
            crate::diagnostics::DiagnosticsCollector::with_enabled(dir.path(), true).unwrap();
        let context = LoopContext::primary(dir.path().to_path_buf());
        std::fs::create_dir_all(context.ralph_dir()).unwrap();
        let events_path = context.events_path();
        std::fs::write(context.current_events_marker(), ".ralph/events.jsonl").unwrap();
        let mut event_loop = EventLoop::with_context_and_diagnostics(config, context, diagnostics);
        event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);
        let harness = Self {
            dir,
            event_loop,
            events_path,
        };
        harness.git(&["init", "-q", "-b", "main"]);
        harness
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(self.dir.path())
            .args([
                "-c",
                "user.name=Gate Test",
                "-c",
                "user.email=gate@test",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn commit(&self, message: &str) -> String {
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    fn use_runner(&mut self, exit_code: i32, stderr: &str) -> Arc<Mutex<Vec<GateCommandRequest>>> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        self.event_loop.gate_runner = Box::new(RecordingRunner {
            outcome: GateCommandOutcome {
                exit_code: Some(exit_code),
                timed_out: false,
                stderr: stderr.to_string(),
            },
            calls: Arc::clone(&calls),
        });
        calls
    }

    /// Starts an iteration the way the runner does.
    fn start_iteration(&mut self) {
        let _ = self.event_loop.build_prompt(&HatId::new("ralph"));
    }

    fn write_event(&self, line: serde_json::Value) {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events_path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
    }

    fn emit(&mut self, topic: &str, payload: &str) -> ProcessedEvents {
        self.write_event(serde_json::json!({
            "topic": topic,
            "payload": payload,
            "ts": chrono::Utc::now().to_rfc3339(),
        }));
        self.event_loop.process_events_from_jsonl().unwrap()
    }

    /// Topic and payload of every event pending for `hat`.
    fn pending(&self, hat: &str) -> Vec<(String, String)> {
        self.event_loop
            .bus
            .peek_pending(&HatId::new(hat))
            .map(|events| {
                events
                    .iter()
                    .map(|e| (e.topic.as_str().to_string(), e.payload.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn pending_topics(&self, hat: &str) -> Vec<String> {
        self.pending(hat)
            .into_iter()
            .map(|(topic, _)| topic)
            .collect()
    }

    /// Reasons of every `BackpressureTriggered` diagnostics entry.
    fn backpressure_reasons(&self) -> Vec<String> {
        let session = self
            .event_loop
            .diagnostics
            .session_dir()
            .expect("diagnostics enabled");
        let Ok(contents) = std::fs::read_to_string(session.join("orchestration.jsonl")) else {
            return Vec::new();
        };
        contents
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|entry| entry["event"]["type"] == "backpressure_triggered")
            .map(|entry| entry["event"]["reason"].as_str().unwrap_or("").to_string())
            .collect()
    }
}

const COMMIT_GATE: &str = r#"
gates:
  - event: "foundation.done"
    requires:
      - field: commit
        verify: commit
  - event: "merge.done"
    requires:
      - field: merged
        verify: { commit: { reachable_from: "HEAD", new: false } }
"#;

#[test]
fn not_a_commit_is_blocked_and_the_original_is_not_routed() {
    let mut h = Harness::new(COMMIT_GATE);
    h.commit("start");
    h.start_iteration();

    let processed = h.emit("foundation.done", "commit=deadbeef summary=done");

    assert!(
        processed.had_events,
        "the blocked event still counts as an event"
    );
    assert!(
        h.pending("reviewer").is_empty(),
        "foundation.done must not reach the reviewer"
    );
    let builder = h.pending("builder");
    assert_eq!(builder.len(), 1, "{builder:?}");
    let (topic, payload) = &builder[0];
    assert_eq!(topic, "foundation.blocked");
    assert!(
        payload.contains("field `commit`: not a commit"),
        "{payload}"
    );
    assert!(
        payload.ends_with("Original payload:\ncommit=deadbeef summary=done"),
        "{payload}"
    );
    let reasons = h.backpressure_reasons();
    assert!(
        reasons
            .iter()
            .any(|r| r.contains("Gate on `foundation.done` failed") && r.contains("not a commit")),
        "{reasons:?}"
    );
}

#[test]
fn passing_event_routes_unchanged() {
    let mut h = Harness::new(COMMIT_GATE);
    h.commit("start");
    h.start_iteration();
    let work = h.commit("work");

    h.emit("foundation.done", &format!("commit={work}"));

    assert_eq!(
        h.pending("reviewer"),
        vec![("foundation.done".to_string(), format!("commit={work}"))]
    );
    assert!(h.pending("builder").is_empty());
    assert!(h.backpressure_reasons().is_empty());
}

#[test]
fn commit_from_before_the_iteration_is_not_new_unless_new_is_false() {
    let mut h = Harness::new(COMMIT_GATE);
    let old = h.commit("old");
    h.start_iteration();

    h.emit("foundation.done", &format!("commit={old}"));
    let builder = h.pending("builder");
    assert_eq!(builder.len(), 1, "{builder:?}");
    assert_eq!(builder[0].0, "foundation.blocked");
    assert!(builder[0].1.contains("not new"), "{}", builder[0].1);

    // `merge.done` sets `new: false`, so the same old commit passes.
    h.emit("merge.done", &format!("merged={old}"));
    assert_eq!(h.pending_topics("reviewer"), vec!["merge.done"]);
}

#[test]
fn each_iteration_records_its_own_start_head() {
    let mut h = Harness::new(COMMIT_GATE);
    h.commit("first");
    h.start_iteration();
    let work = h.commit("work");
    // Same commit, reported again in the next iteration: no longer new.
    h.start_iteration();
    assert_eq!(
        h.event_loop.iteration_start_head,
        Some(StartHead::Commit(work.clone()))
    );
    h.emit("foundation.done", &format!("commit={work}"));
    assert_eq!(h.pending_topics("builder"), vec!["foundation.blocked"]);
}

#[test]
fn unborn_start_head_makes_any_reachable_commit_new() {
    let mut h = Harness::new(COMMIT_GATE);
    h.start_iteration();
    assert_eq!(h.event_loop.iteration_start_head, Some(StartHead::Unborn));
    let first = h.commit("first");
    h.emit("foundation.done", &format!("commit={first}"));
    assert_eq!(h.pending_topics("reviewer"), vec!["foundation.done"]);
}

#[test]
fn new_check_without_a_recorded_head_fails_closed() {
    // Events processed before any iteration started cannot prove newness.
    let mut h = Harness::new(COMMIT_GATE);
    let commit = h.commit("work");
    assert_eq!(h.event_loop.iteration_start_head, None);

    h.emit("foundation.done", &format!("commit={commit}"));

    let builder = h.pending("builder");
    assert_eq!(builder.len(), 1, "{builder:?}");
    assert!(builder[0].1.contains("git failed"), "{}", builder[0].1);
    assert!(
        builder[0]
            .1
            .contains("HEAD was not recorded at iteration start"),
        "{}",
        builder[0].1
    );
    assert!(h.pending("reviewer").is_empty());
}

const REVIEW_GATE: &str = r#"
gates:
  - event: "review.ready"
    requires:
      - field: tests
        status: passed
      - command: ["sh", "-c", "echo 'test_parse failed' >&2; exit 1"]
        timeout: 30
    blocked: "review.blocked"
"#;

#[test]
fn failing_command_blocks_with_exit_code_and_stderr_tail() {
    // Real HookExecutor-backed runner.
    let mut h = Harness::new(REVIEW_GATE);
    h.start_iteration();

    h.emit("review.ready", "tests=passed");

    let builder = h.pending("builder");
    assert_eq!(builder.len(), 1, "{builder:?}");
    let (topic, payload) = &builder[0];
    assert_eq!(topic, "review.blocked");
    assert!(payload.contains("command exited 1"), "{payload}");
    assert!(payload.contains("test_parse failed"), "{payload}");
    assert!(h.pending("reviewer").is_empty());
}

#[test]
fn command_receives_the_event_and_does_not_run_after_a_field_failure() {
    let mut h = Harness::new(REVIEW_GATE);
    let calls = h.use_runner(0, "");
    h.start_iteration();

    h.emit("review.ready", "tests=failed");
    assert!(
        calls.lock().unwrap().is_empty(),
        "command ran after a failed field"
    );
    let builder = h.pending("builder");
    assert_eq!(builder.len(), 1, "{builder:?}");
    assert!(
        builder[0].1.contains("field `tests`: status"),
        "{}",
        builder[0].1
    );

    h.emit("review.ready", "tests=passed");
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].env["RALPH_EVENT_TOPIC"], "review.ready");
    assert_eq!(calls[0].env["RALPH_EVENT_PAYLOAD"], "tests=passed");
    assert_eq!(calls[0].workspace_root, h.dir.path());
    assert_eq!(h.pending_topics("reviewer"), vec!["review.ready"]);
}

#[test]
fn blocked_text_is_bounded_but_keeps_the_original_payload() {
    let mut h = Harness::new(REVIEW_GATE);
    h.use_runner(2, &"e".repeat(64 * 1024));
    h.start_iteration();
    let payload = format!("tests=passed {}", "p".repeat(20_000));

    h.emit("review.ready", &payload);

    let builder = h.pending("builder");
    let (_, blocked) = &builder[0];
    let (reason, original) = blocked.split_once("\n\nOriginal payload:\n").unwrap();
    assert!(reason.contains("command exited 2"), "{reason}");
    assert!(reason.len() < 9 * 1024, "reason is {} bytes", reason.len());
    assert_eq!(original, payload);
}

const COMPLETION_GATE: &str = r#"
gates:
  - event: "LOOP_COMPLETE"
    requires:
      - field: tests
        status: passed
"#;

#[test]
fn failing_completion_gate_does_not_complete() {
    let mut h = Harness::new(COMPLETION_GATE);
    h.start_iteration();

    h.emit("LOOP_COMPLETE", "all done");

    assert!(!h.event_loop.state.completion_requested);
    assert_eq!(h.event_loop.check_completion_event(), None);
    let ralph = h.pending("ralph");
    assert_eq!(ralph.len(), 1, "{ralph:?}");
    assert_eq!(ralph[0].0, "LOOP_COMPLETE.blocked");
    assert!(ralph[0].1.contains("missing"), "{}", ralph[0].1);

    // The output text also carried the promise (OpenCode prints both): the
    // fallback must not complete either, nor report the block twice.
    h.event_loop.request_completion_from_text_fallback();
    assert_eq!(h.event_loop.check_completion_event(), None);
    assert_eq!(h.pending("ralph").len(), 1);

    h.start_iteration();
    h.emit("LOOP_COMPLETE", "tests=passed");
    assert_eq!(
        h.event_loop.check_completion_event(),
        Some(TerminationReason::CompletionPromise)
    );
}

#[test]
fn text_fallback_reuses_a_passed_completion_gate() {
    // The emitted completion passes its gate but is declined for another
    // reason (a required event is missing). The output text also carries the
    // promise: the fallback must not re-gate the empty text and report a
    // false block, nor run the command a second time.
    let mut h = Harness::new(
        r#"
event_loop:
  required_events: ["review.passed"]
gates:
  - event: "LOOP_COMPLETE"
    requires:
      - field: tests
        status: passed
      - command: ["true"]
"#,
    );
    let calls = h.use_runner(0, "");
    h.start_iteration();

    h.emit("LOOP_COMPLETE", "tests=passed");
    assert!(h.event_loop.state.completion_requested);
    assert_eq!(h.event_loop.check_completion_event(), None);
    assert_eq!(h.pending_topics("ralph"), vec!["task.resume"]);

    h.event_loop.request_completion_from_text_fallback();

    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!(
        h.backpressure_reasons().is_empty(),
        "{:?}",
        h.backpressure_reasons()
    );
    let topics = h.pending_topics("ralph");
    assert!(
        !topics.iter().any(|t| t == "LOOP_COMPLETE.blocked"),
        "{topics:?}"
    );
    // Same as without a gate: the fallback requests completion and the
    // required-events check declines it again.
    assert!(h.event_loop.state.completion_requested);
    assert_eq!(h.event_loop.check_completion_event(), None);

    // A new iteration forgets the outcome: text alone is gated again.
    h.start_iteration();
    h.event_loop.request_completion_from_text_fallback();
    assert!(!h.event_loop.state.completion_requested);
    assert!(
        h.pending_topics("ralph")
            .iter()
            .any(|t| t == "LOOP_COMPLETE.blocked")
    );
}

#[test]
fn completion_blocked_once_stays_blocked_for_the_text_fallback() {
    // A later passing completion in the same iteration does not clear an
    // earlier block: the fallback still refuses without reporting again.
    let mut h = Harness::new(COMPLETION_GATE);
    h.start_iteration();

    h.emit("LOOP_COMPLETE", "all done");
    h.emit("LOOP_COMPLETE", "tests=passed");
    let reasons = h.backpressure_reasons().len();

    h.event_loop.state.completion_requested = false;
    h.event_loop.request_completion_from_text_fallback();

    assert!(!h.event_loop.state.completion_requested);
    assert_eq!(h.backpressure_reasons().len(), reasons);
}

#[test]
fn completion_promise_in_text_alone_is_gated() {
    // A backend that only prints the promise (no `ralph emit`) carries no
    // payload, so a field gate blocks it.
    let mut h = Harness::new(COMPLETION_GATE);
    h.start_iteration();

    h.event_loop.request_completion_from_text_fallback();

    assert_eq!(h.event_loop.check_completion_event(), None);
    assert_eq!(h.pending_topics("ralph"), vec!["LOOP_COMPLETE.blocked"]);
}

#[test]
fn completion_promise_in_text_passes_a_command_only_gate() {
    let mut h = Harness::new(
        r#"
gates:
  - event: "LOOP_COMPLETE"
    requires:
      - command: ["true"]
"#,
    );
    let calls = h.use_runner(0, "");
    h.start_iteration();

    h.event_loop.request_completion_from_text_fallback();

    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(
        h.event_loop.check_completion_event(),
        Some(TerminationReason::CompletionPromise)
    );
}

#[test]
fn gated_cancel_that_fails_does_not_cancel() {
    let mut h = Harness::new(
        r#"
gates:
  - event: "loop.cancel"
    requires:
      - field: reason
"#,
    );
    h.start_iteration();
    h.emit("loop.cancel", "");
    assert!(!h.event_loop.state.cancellation_requested);
    assert_eq!(h.pending_topics("ralph"), vec!["loop.blocked"]);
}

#[test]
fn wave_dispatch_events_are_gated() {
    let hats = r#"
hats:
  dispatcher:
    name: "Dispatcher"
    description: "Dispatches"
    triggers: ["work.start", "review.blocked"]
    publishes: ["review.file"]
  reviewer:
    name: "Reviewer"
    description: "Reviews"
    triggers: ["review.file"]
    publishes: ["review.done"]
    concurrency: 3
"#;
    let gate = r#"
gates:
  - event: "review.file"
    requires:
      - field: path
"#;
    let mut h = Harness::with_hats(hats, gate);
    h.start_iteration();
    for (index, payload) in ["path=src/a.rs", "no path here", "path=src/c.rs"]
        .iter()
        .enumerate()
    {
        h.write_event(serde_json::json!({
            "topic": "review.file",
            "payload": payload,
            "ts": chrono::Utc::now().to_rfc3339(),
            "wave_id": "w-1",
            "wave_index": index,
            "wave_total": 3,
        }));
    }

    let result = h.event_loop.process_events_from_jsonl_with_waves().unwrap();

    let dispatched: Vec<_> = result
        .wave_events
        .iter()
        .map(|e| e.payload.clone().unwrap_or_default())
        .collect();
    assert_eq!(dispatched, vec!["path=src/a.rs", "path=src/c.rs"]);
    assert!(result.processed.had_events);
    let dispatcher = h.pending("dispatcher");
    assert_eq!(dispatcher.len(), 1, "{dispatcher:?}");
    assert_eq!(dispatcher[0].0, "review.blocked");
    assert!(
        dispatcher[0].1.contains("no path here"),
        "{}",
        dispatcher[0].1
    );
}

/// Robot service whose question is answered after the agent appended a
/// (gated) completion event during the wait.
struct AnsweringRobot {
    during_wait: serde_json::Value,
}

impl RobotService for AnsweringRobot {
    fn send_question(&self, _payload: &str) -> anyhow::Result<i32> {
        Ok(1)
    }

    fn wait_for_response(&self, events_path: &Path) -> anyhow::Result<Option<String>> {
        let mut file = std::fs::OpenOptions::new().append(true).open(events_path)?;
        writeln!(file, "{}", self.during_wait)?;
        Ok(Some("go ahead".to_string()))
    }

    fn send_checkin(
        &self,
        _iteration: u32,
        _elapsed: Duration,
        _context: Option<&CheckinContext>,
    ) -> anyhow::Result<i32> {
        Ok(0)
    }

    fn timeout_secs(&self) -> u64 {
        1
    }

    fn shutdown_flag(&self) -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    fn stop(self: Box<Self>) {}
}

#[test]
fn events_arriving_during_a_robot_wait_are_gated() {
    let mut h = Harness::new(COMPLETION_GATE);
    h.event_loop.set_robot_service(Box::new(AnsweringRobot {
        during_wait: serde_json::json!({
            "topic": "LOOP_COMPLETE",
            "payload": "done, trust me",
            "ts": chrono::Utc::now().to_rfc3339(),
        }),
    }));
    h.start_iteration();

    h.emit("human.interact", "Ship it?");

    assert!(!h.event_loop.state.completion_requested);
    assert_eq!(h.event_loop.check_completion_event(), None);
    let ralph = h.pending_topics("ralph");
    assert!(
        ralph.contains(&"LOOP_COMPLETE.blocked".to_string()),
        "{ralph:?}"
    );
}

#[test]
fn no_gates_means_unchanged_behavior() {
    let mut h = Harness::new("");
    h.commit("start");
    h.start_iteration();
    // No gate needs HEAD, so none is recorded and no git process runs.
    assert_eq!(h.event_loop.iteration_start_head, None);

    h.emit("foundation.done", "commit=deadbeef");
    h.emit("LOOP_COMPLETE", "done");

    assert_eq!(
        h.pending("reviewer"),
        vec![("foundation.done".to_string(), "commit=deadbeef".to_string())]
    );
    assert!(h.backpressure_reasons().is_empty());
    assert_eq!(
        h.event_loop.check_completion_event(),
        Some(TerminationReason::CompletionPromise)
    );
}

#[test]
fn gates_without_new_checks_do_not_record_head() {
    let mut h = Harness::new(REVIEW_GATE);
    h.start_iteration();
    assert_eq!(h.event_loop.iteration_start_head, None);
}

#[test]
fn repeated_identical_blocks_trip_stale_loop_detection() {
    let mut h = Harness::new(COMMIT_GATE);
    h.commit("start");
    for _ in 0..3 {
        h.start_iteration();
        h.emit("foundation.done", "commit=deadbeef");
    }
    assert_eq!(
        h.event_loop.check_termination(),
        Some(TerminationReason::LoopStale)
    );
}

#[test]
fn gate_on_build_done_feeds_task_thrashing_detection() {
    // A gate on `build.done` blocks to `build.blocked`, which the existing
    // per-task thrashing detection counts (task id = first payload line).
    let mut h = Harness::new(
        r#"
gates:
  - event: "build.done"
    requires:
      - field: commit
        verify: commit
"#,
    );
    h.commit("start");
    h.start_iteration();
    for attempt in 0..3 {
        h.emit("build.done", &format!("commit=nope{attempt}"));
    }
    assert!(
        h.event_loop
            .state
            .abandoned_tasks
            .iter()
            .any(|task| task.starts_with("Gate on `build.done` failed")),
        "{:?}",
        h.event_loop.state.abandoned_tasks
    );
}
