//! Event loop tests for script hat accounting: consuming a script hat's
//! trigger events without a prompt, the `max_iterations` / `max_script_runs`
//! split, max_activations, stale-loop detection, and forced scope enforcement.

use super::*;
use std::process::Command;
use tempfile::TempDir;

/// `worker` is a model hat. `stepper` and `checker` are script hats:
/// work.next -> worker -> step.run -> stepper -> step.check -> checker -> work.next.
const HATS: &str = r#"
hats:
  worker:
    name: "Worker"
    description: "Model work"
    triggers: ["work.next"]
    publishes: ["step.run", "side.note"]
  stepper:
    name: "Stepper"
    description: "Script step"
    triggers: ["step.run"]
    publishes: ["step.check"]
    run: "true"
  checker:
    name: "Checker"
    description: "Script check"
    triggers: ["step.check"]
    publishes: ["work.next"]
    run: ["sh", "-c", "true"]
"#;

struct Harness {
    dir: TempDir,
    event_loop: EventLoop,
    events_path: PathBuf,
}

impl Harness {
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
        Self {
            dir,
            event_loop,
            events_path,
        }
    }

    fn publish(&mut self, topic: &str, payload: &str) {
        self.event_loop.bus.publish(Event::new(topic, payload));
    }

    fn emit(&mut self, topic: &str, payload: &str) -> ProcessedEvents {
        let line = serde_json::json!({
            "topic": topic,
            "payload": payload,
            "ts": chrono::Utc::now().to_rfc3339(),
        });
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events_path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
        self.event_loop.process_events_from_jsonl().unwrap()
    }

    fn active(&self) -> String {
        self.event_loop.get_active_hat_id().as_str().to_string()
    }

    /// A model iteration as the runner does it: build the prompt, then
    /// process the output.
    fn llm_iteration(&mut self) -> Option<TerminationReason> {
        let ralph = HatId::new("ralph");
        self.event_loop
            .build_prompt(&ralph)
            .expect("prompt for ralph");
        self.event_loop.process_output(&ralph, "", true)
    }

    /// A script iteration: consume the hat's triggers, then process the
    /// output. Returns the consumed events and the termination check.
    fn script_iteration(&mut self, hat: &str) -> (Vec<Event>, Option<TerminationReason>) {
        let events = self
            .event_loop
            .begin_script_iteration(&HatId::new(hat))
            .expect("script hat activates");
        let termination = self
            .event_loop
            .process_output(&HatId::new("ralph"), "", true);
        (events, termination)
    }

    fn pending_topics(&self, hat: &str) -> Vec<String> {
        self.event_loop
            .bus
            .peek_pending(&HatId::new(hat))
            .map(|events| events.iter().map(|e| e.topic.to_string()).collect())
            .unwrap_or_default()
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(self.dir.path())
            .args([
                "-c",
                "user.name=Script Test",
                "-c",
                "user.email=script@test",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed");
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }
}

#[test]
fn begin_script_iteration_consumes_only_that_hats_triggers() {
    let mut h = Harness::new("");
    h.publish("step.run", "first");
    h.publish("step.run", "second");
    h.publish("work.next", "for the worker");

    let events = h
        .event_loop
        .begin_script_iteration(&HatId::new("stepper"))
        .expect("stepper has triggers");

    let consumed: Vec<_> = events
        .iter()
        .map(|e| (e.topic.as_str(), e.payload.as_str()))
        .collect();
    assert_eq!(consumed, [("step.run", "first"), ("step.run", "second")]);
    assert!(h.pending_topics("stepper").is_empty());
    assert_eq!(h.pending_topics("worker"), ["work.next"]);
    assert_eq!(
        h.event_loop.state.last_active_hat_ids,
        [HatId::new("stepper")]
    );
    assert_eq!(
        h.event_loop.state.hat_activation_counts[&HatId::new("stepper")],
        1
    );

    // Nothing pending: the hat does not activate.
    assert!(
        h.event_loop
            .begin_script_iteration(&HatId::new("stepper"))
            .is_none()
    );
    // A model hat is not a script hat.
    assert!(
        h.event_loop
            .begin_script_iteration(&HatId::new("worker"))
            .is_none()
    );
    assert_eq!(h.pending_topics("worker"), ["work.next"]);
}

#[test]
fn model_iteration_leaves_an_active_script_hats_triggers_pending() {
    let mut h = Harness::new("");
    h.publish("work.next", "model work");
    h.publish("step.run", "script work");
    // stepper sorts before worker, so the runner would pick the script first;
    // a model iteration that runs anyway must not swallow its triggers.
    assert_eq!(h.active(), "stepper");

    h.event_loop.build_prompt(&HatId::new("ralph")).unwrap();

    assert_eq!(h.pending_topics("stepper"), ["step.run"]);
    assert!(h.pending_topics("worker").is_empty());
    assert_eq!(
        h.event_loop.state.last_active_hat_ids,
        [HatId::new("worker")]
    );
    assert!(
        !h.event_loop
            .state
            .hat_activation_counts
            .contains_key(&HatId::new("stepper"))
    );
    assert_eq!(h.active(), "stepper");
}

#[test]
fn script_iterations_do_not_count_toward_max_iterations() {
    let mut h = Harness::new("event_loop:\n  max_iterations: 5\n  max_script_runs: 50\n");
    h.publish("step.run", "round 0");

    for round in 1..=5 {
        // Two script iterations, then one model iteration.
        assert_eq!(h.active(), "stepper");
        let (_, termination) = h.script_iteration("stepper");
        assert_eq!(termination, None, "round {round}: stepper");
        h.emit("step.check", &format!("round {round}"));

        assert_eq!(h.active(), "checker");
        let (_, termination) = h.script_iteration("checker");
        assert_eq!(termination, None, "round {round}: checker");
        h.emit("work.next", &format!("round {round}"));

        assert_eq!(h.active(), "worker");
        let termination = h.llm_iteration();
        if round < 5 {
            assert_eq!(termination, None, "round {round}: model iteration");
            h.emit("step.run", &format!("round {round}"));
        } else {
            assert_eq!(termination, Some(TerminationReason::MaxIterations));
        }
    }

    // Every iteration got a number; 10 of the 15 were script runs.
    assert_eq!(h.event_loop.state.iteration, 15);
    assert_eq!(h.event_loop.state.script_runs, 10);
}

#[test]
fn script_hats_triggering_each_other_stop_at_max_script_runs() {
    let hats = r#"
hats:
  ping:
    name: "Ping"
    description: "Ping"
    triggers: ["ping"]
    publishes: ["pong"]
    run: "true"
  pong:
    name: "Pong"
    description: "Pong"
    triggers: ["pong"]
    publishes: ["ping"]
    run: "true"
"#;
    let mut h = Harness::with_hats(hats, "event_loop:\n  max_script_runs: 4\n");
    h.publish("ping", "0");

    let mut terminations = Vec::new();
    for n in 1..=4 {
        let hat = h.active();
        let (_, termination) = h.script_iteration(&hat);
        terminations.push(termination);
        let next = if hat == "ping" { "pong" } else { "ping" };
        h.emit(next, &n.to_string());
    }

    assert_eq!(
        terminations,
        [None, None, None, Some(TerminationReason::MaxScriptRuns)]
    );
    assert_eq!(
        h.event_loop.check_termination(),
        Some(TerminationReason::MaxScriptRuns)
    );
    assert_eq!(TerminationReason::MaxScriptRuns.as_str(), "max_script_runs");
    assert_eq!(TerminationReason::MaxScriptRuns.exit_code(), 2);
}

#[test]
fn a_script_hat_repeating_itself_trips_stale_loop_detection() {
    let hats = r#"
hats:
  tick:
    name: "Tick"
    description: "Ticks"
    triggers: ["tick"]
    publishes: ["tick"]
    run: "true"
"#;
    let mut h = Harness::with_hats(hats, "");
    h.publish("tick", "same");

    let mut terminations = Vec::new();
    for _ in 0..3 {
        let (_, termination) = h.script_iteration("tick");
        h.emit("tick", "same");
        terminations.push(termination.or_else(|| h.event_loop.check_termination()));
    }

    assert_eq!(
        terminations,
        [None, None, Some(TerminationReason::LoopStale)]
    );
}

#[test]
fn max_activations_applies_to_script_hats() {
    let hats = r#"
hats:
  stepper:
    name: "Stepper"
    description: "Script step"
    triggers: ["step.run"]
    publishes: ["step.check"]
    run: "true"
    max_activations: 2
"#;
    let mut h = Harness::with_hats(hats, "");
    for n in 1..=2 {
        h.publish("step.run", &n.to_string());
        h.script_iteration("stepper");
    }

    h.publish("step.run", "3");
    assert!(
        h.event_loop
            .begin_script_iteration(&HatId::new("stepper"))
            .is_none(),
        "an exhausted script hat does not run"
    );
    assert!(h.pending_topics("stepper").is_empty(), "triggers dropped");
    assert_eq!(h.pending_topics("ralph"), ["stepper.exhausted"]);
    assert_eq!(h.event_loop.state.script_runs, 2);
}

#[test]
fn script_hat_scope_is_enforced_even_when_enforce_hat_scope_is_off() {
    let mut h = Harness::new("");
    assert!(!h.event_loop.config.event_loop.enforce_hat_scope);

    h.publish("step.run", "go");
    h.script_iteration("stepper");
    h.emit("work.next", "skipping the checker");
    h.emit("step.check", "in scope");

    assert!(
        h.pending_topics("worker").is_empty(),
        "out-of-scope event dropped"
    );
    assert_eq!(h.pending_topics("checker"), ["step.check"]);
    let violations: Vec<_> = h
        .event_loop
        .bus
        .peek_pending(&HatId::new("ralph"))
        .into_iter()
        .flatten()
        .filter(|e| e.topic.as_str() == "stepper.scope_violation")
        .map(|e| e.payload.clone())
        .collect();
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("work.next"), "{violations:?}");

    // A model iteration afterwards is not scope-enforced.
    h.event_loop.bus.take_pending(&HatId::new("ralph"));
    h.event_loop.bus.take_pending(&HatId::new("checker"));
    h.publish("work.next", "model");
    h.llm_iteration();
    h.emit("step.check", "outside worker's publishes, but allowed");
    assert_eq!(h.pending_topics("checker"), ["step.check"]);
}

#[test]
fn script_iterations_record_start_head_for_new_commit_gates() {
    let gate = r#"
gates:
  - event: "step.check"
    requires:
      - field: commit
        verify: commit
"#;
    let mut h = Harness::new(gate);
    h.git(&["init", "-q", "-b", "main"]);
    h.git(&["commit", "-q", "--allow-empty", "-m", "start"]);

    h.publish("step.run", "go");
    h.script_iteration("stepper");
    h.git(&["commit", "-q", "--allow-empty", "-m", "new work"]);
    let new_commit = h.git(&["rev-parse", "HEAD"]);
    h.emit("step.check", &format!("commit={new_commit}"));

    assert_eq!(
        h.pending_topics("checker"),
        ["step.check"],
        "a commit made during the script iteration is new"
    );
}

#[test]
fn script_runs_survive_a_continue() {
    let mut h = Harness::new("");
    h.publish("step.run", "go");
    h.script_iteration("stepper");
    let path = h.dir.path().join("loop-state.json");
    h.event_loop.save_loop_state(&path).unwrap();

    let mut resumed = Harness::new("");
    resumed.event_loop.restore_loop_state(&path).unwrap();
    assert_eq!(resumed.event_loop.state.iteration, 1);
    assert_eq!(resumed.event_loop.state.script_runs, 1);

    // State files written before script hats existed still load.
    std::fs::write(
        &path,
        r#"{"iteration": 3, "total_cost_usd": 0.0, "last_hat": null}"#,
    )
    .unwrap();
    resumed.event_loop.restore_loop_state(&path).unwrap();
    assert_eq!(resumed.event_loop.state.script_runs, 0);
}

/// Hats for the failure tests: `stepper.failed` is routed to `fixer`.
const FAILURE_HATS: &str = r#"
hats:
  stepper:
    name: "Stepper"
    description: "Script step"
    triggers: ["step.run"]
    publishes: ["step.check"]
    run: "true"
  fixer:
    name: "Fixer"
    description: "Handles script failures"
    triggers: ["stepper.failed"]
    publishes: ["step.run"]
"#;

impl Harness {
    /// Appends an event line the way `ralph emit` does, without reading it.
    fn append_emit(&self, topic: &str, payload: &str) {
        let line = serde_json::json!({
            "topic": topic,
            "payload": payload,
            "ts": chrono::Utc::now().to_rfc3339(),
        });
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events_path)
            .unwrap();
        writeln!(file, "{line}").unwrap();
    }

    fn pending_payloads(&self, hat: &str) -> Vec<String> {
        self.event_loop
            .bus
            .peek_pending(&HatId::new(hat))
            .map(|events| events.iter().map(|e| e.payload.clone()).collect())
            .unwrap_or_default()
    }
}

#[test]
fn script_failure_messages_name_the_cause() {
    let cases = [
        (
            ScriptFailure::Exited {
                code: Some(3),
                stderr: String::new(),
            },
            "command exited 3",
        ),
        (
            ScriptFailure::Exited {
                code: None,
                stderr: String::new(),
            },
            "command was killed by a signal",
        ),
        (
            ScriptFailure::TimedOut {
                seconds: 7,
                stderr: String::new(),
            },
            "command timed out after 7s",
        ),
        (
            ScriptFailure::CouldNotRun {
                error: "not found".to_string(),
            },
            "could not run command: not found",
        ),
        (ScriptFailure::NoEvents, "command exited 0 without emitting"),
    ];
    for (failure, expected) in cases {
        assert_eq!(failure.to_string(), expected);
    }
    assert!(!ScriptFailure::NoEvents.counts_as_failure());
    assert!(
        ScriptFailure::Exited {
            code: Some(1),
            stderr: String::new()
        }
        .counts_as_failure()
    );
}

#[test]
fn failed_script_publishes_hat_failed_with_exit_code_and_stderr_tail() {
    let mut h = Harness::with_hats(FAILURE_HATS, "");
    h.publish("step.run", "go");
    h.script_iteration("stepper");

    // 6000 bytes of stderr ending in a marker; multi-byte chars around the cut.
    let stderr = format!("{}{}END", "é".repeat(2000), "x".repeat(1997));
    h.event_loop.publish_script_failure(
        &HatId::new("stepper"),
        &ScriptFailure::Exited {
            code: Some(2),
            stderr: stderr.clone(),
        },
        0,
    );

    let payloads = h.pending_payloads("fixer");
    assert_eq!(payloads.len(), 1, "stepper.failed routes to its subscriber");
    let payload = &payloads[0];
    assert!(payload.starts_with("command exited 2"), "{payload}");
    assert!(payload.ends_with("END"), "{payload}");
    let (_, tail) = payload.split_once("stderr (last 4 KiB):\n").unwrap();
    assert!(
        tail.len() <= SCRIPT_STDERR_TAIL_BYTES,
        "tail is {} bytes",
        tail.len()
    );
    assert!(stderr.ends_with(tail));
}

#[test]
fn failed_script_payload_reports_discarded_events() {
    let mut h = Harness::with_hats(FAILURE_HATS, "");
    h.event_loop.publish_script_failure(
        &HatId::new("stepper"),
        &ScriptFailure::TimedOut {
            seconds: 1,
            stderr: String::new(),
        },
        2,
    );
    assert_eq!(
        h.pending_payloads("fixer"),
        ["command timed out after 1s\n2 emitted event(s) discarded"]
    );
}

#[test]
fn discard_unread_events_drops_a_failed_scripts_emissions() {
    let mut h = Harness::with_hats(FAILURE_HATS, "");
    h.publish("step.run", "go");
    h.script_iteration("stepper");
    h.append_emit("step.check", "partial");
    h.append_emit("iteration.summary", "{}");

    assert_eq!(h.event_loop.discard_unread_events().unwrap(), 1);
    let processed = h.event_loop.process_events_from_jsonl().unwrap();
    assert!(!processed.had_events, "discarded events are not read again");
    assert!(!h.event_loop.has_pending_events());
}

#[test]
fn script_iterations_are_labelled_in_diagnostics() {
    let mut h = Harness::new("");
    h.publish("work.next", "go");
    h.llm_iteration();
    h.publish("step.run", "go");
    h.script_iteration("stepper");

    let session = h
        .event_loop
        .diagnostics
        .session_dir()
        .expect("diagnostics enabled")
        .to_path_buf();
    let selected: Vec<(String, String)> =
        std::fs::read_to_string(session.join("orchestration.jsonl"))
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|entry| entry["event"]["type"] == "hat_selected")
            .map(|entry| {
                (
                    entry["event"]["hat"].as_str().unwrap_or("").to_string(),
                    entry["event"]["reason"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect();
    assert_eq!(
        selected,
        [
            ("ralph".to_string(), "process_output".to_string()),
            ("stepper".to_string(), "script".to_string()),
        ]
    );
}

#[test]
fn promise_events_are_reported_although_ralph_does_not_publish_them() {
    let mut h = Harness::new(
        "event_loop:\n  completion_promise: \"LOOP_COMPLETE\"\n  cancellation_promise: \"loop.cancel\"\n",
    );

    let completion = h.emit("LOOP_COMPLETE", "done");
    assert!(!completion.had_events, "the promise is not published");
    assert!(completion.had_promise_events);

    let cancel = h.emit("loop.cancel", "stop");
    assert!(!cancel.had_events);
    assert!(cancel.had_promise_events);

    let ordinary = h.emit("work.next", "go");
    assert!(ordinary.had_events);
    assert!(!ordinary.had_promise_events);
}
