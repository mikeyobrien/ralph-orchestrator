//! Replay smoke test: one LLM hat followed by one script hat.
//!
//! The fixture (`fixtures/script-hats/llm_then_script_session.jsonl`) is a
//! session in `SessionRecorder` format. Each `_meta.iteration` record starts an
//! iteration; its `ux.terminal.write` records are what the iteration printed,
//! and its `bus.publish` records are what it emitted with `ralph emit`. The
//! test replays the iterations through a real `EventLoop` the way the loop
//! runner drives it: `build_prompt` for the model hat, `begin_script_iteration`
//! for the script hat, emitted events appended to the events file, then
//! `process_output`, `process_events_from_jsonl`, and the completion check.
//!
//! The script hat's command is not run here: like the model's output, its
//! output is replayed. Real script execution is covered by
//! `ralph-cli/tests/integration_script_hats.rs`.
//!
//! The module is named `smoke_runner_*` so `cargo test -p ralph-core
//! smoke_runner` runs it without the `recording` feature.

use ralph_core::{EventLoop, LoopContext, RalphConfig, TerminationReason};
use ralph_proto::{HatId, TerminalWrite};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// The config the fixture was recorded with. With `max_iterations: 2`, the
/// second iteration would end the run if the script run counted toward it.
/// The gate on `build.ready` checks the model's evidence before the script
/// hat sees it.
const CONFIG: &str = r#"
event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 2
hats:
  builder:
    name: "Builder"
    description: "Writes greeting.txt"
    triggers: ["work.start"]
    publishes: ["build.ready"]
    instructions: "Write greeting.txt, then emit build.ready with file=<path>."
  checker:
    name: "Checker"
    description: "Checks greeting.txt and completes the loop"
    triggers: ["build.ready"]
    publishes: ["LOOP_COMPLETE", "check.failed"]
    run: 'grep -qx hello greeting.txt && "$RALPH_BIN" emit LOOP_COMPLETE "checked after $RALPH_TRIGGER_TOPIC"'
gates:
  - event: "build.ready"
    requires:
      - field: file
"#;

/// One recorded iteration.
#[derive(Debug, Default)]
struct RecordedIteration {
    hat: String,
    output: String,
    emitted: Vec<(String, String)>,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/script-hats/llm_then_script_session.jsonl")
}

/// Splits the fixture into iterations at its `_meta.iteration` records.
fn load_iterations(path: &Path) -> Vec<RecordedIteration> {
    let content = std::fs::read_to_string(path).expect("read fixture");
    let mut iterations: Vec<RecordedIteration> = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let record: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("fixture line {}: {e}", index + 1));
        let data = &record["data"];
        match record["event"].as_str() {
            Some("_meta.iteration") => iterations.push(RecordedIteration {
                hat: data["hat"].as_str().expect("iteration hat").to_string(),
                ..RecordedIteration::default()
            }),
            Some("ux.terminal.write") => {
                let write: TerminalWrite =
                    serde_json::from_value(data.clone()).expect("terminal write");
                let bytes = write.decode_bytes().expect("base64 terminal bytes");
                iterations
                    .last_mut()
                    .expect("terminal write inside an iteration")
                    .output
                    .push_str(&String::from_utf8(bytes).expect("utf-8 output"));
            }
            Some("bus.publish") => iterations
                .last_mut()
                .expect("event inside an iteration")
                .emitted
                .push((
                    data["topic"].as_str().expect("topic").to_string(),
                    data["payload"].as_str().unwrap_or_default().to_string(),
                )),
            _ => {}
        }
    }
    iterations
}

struct Replay {
    _dir: TempDir,
    event_loop: EventLoop,
    events_path: PathBuf,
}

impl Replay {
    fn new() -> Self {
        let dir = TempDir::new().expect("temp dir");
        let mut config: RalphConfig = serde_yaml::from_str(CONFIG).expect("parse config");
        config.core.workspace_root = dir.path().to_path_buf();
        config.validate().expect("config is valid");

        let context = LoopContext::primary(dir.path().to_path_buf());
        std::fs::create_dir_all(context.ralph_dir()).expect("create .ralph");
        std::fs::write(context.current_events_marker(), ".ralph/events.jsonl")
            .expect("write events marker");
        let events_path = context.events_path();
        let mut event_loop = EventLoop::with_context(config, context);
        event_loop.initialize("write a greeting");
        Self {
            _dir: dir,
            event_loop,
            events_path,
        }
    }

    /// What `ralph emit` does during the iteration.
    fn append_emitted(&self, emitted: &[(String, String)]) {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events_path)
            .expect("open events file");
        for (topic, payload) in emitted {
            let line = serde_json::json!({
                "topic": topic,
                "payload": payload,
                "ts": "2026-11-12T00:00:00Z",
            });
            writeln!(file, "{line}").expect("append event");
        }
    }

    /// Replays one iteration as the loop runner drives it, and returns the
    /// topics of the events it consumed and the termination, if any.
    fn iteration(
        &mut self,
        recorded: &RecordedIteration,
    ) -> (Vec<String>, Option<TerminationReason>) {
        let ralph = HatId::new("ralph");
        let active = self.event_loop.get_active_hat_id();
        assert_eq!(active.as_str(), recorded.hat, "active hat");

        let consumed = if self.event_loop.is_script_hat(&active) {
            self.event_loop
                .begin_script_iteration(&active)
                .expect("script hat has triggers")
                .iter()
                .map(|event| event.topic.to_string())
                .collect()
        } else {
            let prompt = self
                .event_loop
                .build_prompt(&ralph)
                .expect("prompt for the model hat");
            assert!(
                prompt.contains("Write greeting.txt, then emit build.ready"),
                "model prompt carries the hat's instructions"
            );
            Vec::new()
        };

        self.append_emitted(&recorded.emitted);
        if let Some(reason) = self
            .event_loop
            .process_output(&ralph, &recorded.output, true)
        {
            return (consumed, Some(reason));
        }
        self.event_loop
            .process_events_from_jsonl()
            .expect("read emitted events");
        (consumed, self.event_loop.check_completion_event())
    }
}

mod smoke_runner_script_hats {
    use super::*;

    #[test]
    fn llm_hat_then_script_hat_replays_to_completion() {
        let iterations = load_iterations(&fixture_path());
        let hats: Vec<_> = iterations.iter().map(|i| i.hat.as_str()).collect();
        assert_eq!(hats, ["builder", "checker"], "fixture iterations");

        let mut replay = Replay::new();

        let (consumed, termination) = replay.iteration(&iterations[0]);
        assert!(consumed.is_empty());
        assert_eq!(termination, None, "model iteration");

        // The model's gated build.ready is the script hat's trigger.
        let (consumed, termination) = replay.iteration(&iterations[1]);
        assert_eq!(consumed, ["build.ready"]);
        assert_eq!(termination, Some(TerminationReason::CompletionPromise));

        // Two iterations, one of them a script run, so max_iterations: 2 was
        // never reached.
        let state = replay.event_loop.state();
        assert_eq!(state.iteration, 2);
        assert_eq!(state.script_runs, 1);
        assert_eq!(state.model_iterations(), 1);
    }

    #[test]
    fn missing_gate_evidence_blocks_the_script_hats_trigger() {
        let mut iterations = load_iterations(&fixture_path());
        // The model claims done without the gate's `file` field.
        iterations[0].emitted = vec![("build.ready".to_string(), "done".to_string())];

        let mut replay = Replay::new();
        let (_, termination) = replay.iteration(&iterations[0]);
        assert_eq!(termination, None);

        // build.ready was rewritten to build.blocked, which no hat triggers
        // on, so Ralph handles it and the script hat never activates.
        let active = replay.event_loop.get_active_hat_id();
        assert_eq!(active.as_str(), "ralph");
        assert!(
            replay
                .event_loop
                .begin_script_iteration(&HatId::new("checker"))
                .is_none(),
            "checker must not see the blocked event"
        );
        assert_eq!(replay.event_loop.state().script_runs, 0);
    }
}
