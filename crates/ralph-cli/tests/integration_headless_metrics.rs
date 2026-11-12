//! Headless (`--autonomous`) Claude iterations must account for the
//! cost and tokens their stream-json reports.
//!
//! The backend is the real `claude` backend definition with its command pointed
//! at a script that replays a recorded stream (`fixtures/claude_headless_iteration.jsonl`),
//! so the run goes through the non-PTY `CliExecutor` path exactly as a live
//! headless Claude run does, with no API call.

#![cfg(unix)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Output;
use tempfile::TempDir;

const STREAM: &str = include_str!("fixtures/claude_headless_iteration.jsonl");

/// Values the recorded stream reports, as the PTY path already accounts them:
/// cost and turns from the `result` line, input as the peak per-turn live
/// context (input + cache_creation + cache_read), output summed across turns,
/// cache fields as per-turn peaks.
const COST_USD: f64 = 0.2934;
const NUM_TURNS: u64 = 2;
const PEAK_INPUT_TOKENS: u64 = 8 + 200 + 22_000;
const OUTPUT_TOKENS: u64 = 350 + 260;
const PEAK_CACHE_READ: u64 = 22_000;
const PEAK_CACHE_WRITE: u64 = 4_100;
/// `modelUsage.claude-opus-5-5.contextWindow` on the result line.
const STREAM_CONTEXT_WINDOW: u64 = 1_000_000;

struct Workspace {
    dir: TempDir,
}

impl Workspace {
    fn new(event_loop: &str) -> Self {
        let dir = TempDir::new().expect("temp dir");
        let ws = Self { dir };
        std::fs::write(ws.path().join("stream.jsonl"), STREAM).expect("write stream");
        let script = ws.path().join("fake-claude.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\ncat >/dev/null\necho launched >> backend-ran.txt\ncat stream.jsonl\n",
        )
        .expect("write backend");
        let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).expect("chmod");
        std::fs::write(
            ws.path().join("ralph.yml"),
            format!(
                "cli:\n  backend: claude\n  command: \"./fake-claude.sh\"\nevent_loop:\n  completion_promise: \"LOOP_COMPLETE\"\n{event_loop}"
            ),
        )
        .expect("write config");
        ws
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn run(&self) -> Output {
        self.run_with_env(&[])
    }

    fn run_with_env(&self, env: &[(&str, &str)]) -> Output {
        common::ralph_command()
            .envs(env.iter().copied())
            .args([
                "run",
                "--autonomous",
                "--skip-preflight",
                "--prompt",
                "read the plan",
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

    fn events(&self) -> Vec<serde_json::Value> {
        let marker = self.read(".ralph/current-events");
        self.read(marker.trim())
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    fn summaries(&self) -> Vec<serde_json::Value> {
        self.events()
            .into_iter()
            .filter(|record| record["topic"] == "iteration.summary")
            .map(|record| serde_json::from_str(record["payload"].as_str().unwrap_or("{}")).unwrap())
            .collect()
    }

    fn termination(&self) -> String {
        self.events()
            .into_iter()
            .filter(|record| record["topic"] == "loop.terminate")
            .map(|record| record["payload"].as_str().unwrap_or_default().to_string())
            .next_back()
            .expect("loop.terminate logged")
    }

    fn backend_launches(&self) -> usize {
        std::fs::read_to_string(self.path().join("backend-ran.txt"))
            .map_or(0, |launches| launches.lines().count())
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
fn headless_claude_iteration_summary_carries_stream_cost_and_tokens() {
    let ws = Workspace::new("  max_iterations: 2\n");

    let output = ws.run();

    assert_eq!(ws.backend_launches(), 2, "{}", describe(&output));
    let summaries = ws.summaries();
    assert_eq!(summaries.len(), 2, "{summaries:?}\n{}", describe(&output));
    for summary in &summaries {
        assert_eq!(summary["backend"], "claude", "{summary}");
        assert_eq!(summary["cost_usd"], COST_USD, "{summary}");
        assert_eq!(summary["num_turns"], NUM_TURNS, "{summary}");
        assert_eq!(summary["input_tokens"], PEAK_INPUT_TOKENS, "{summary}");
        assert_eq!(summary["output_tokens"], OUTPUT_TOKENS, "{summary}");
        assert_eq!(summary["cache_read_tokens"], PEAK_CACHE_READ, "{summary}");
        assert_eq!(summary["cache_write_tokens"], PEAK_CACHE_WRITE, "{summary}");
        // The live context occupancy is the peak per-turn input, the same
        // figure the PTY path records for peak-context tracking.
        assert_eq!(summary["context_tokens"], PEAK_INPUT_TOKENS, "{summary}");
        // The result line's modelUsage reports the model's real window; it
        // wins over the claude backend's 200000 default.
        assert_eq!(
            summary["context_window"], STREAM_CONTEXT_WINDOW,
            "{summary}"
        );
    }

    // The loop's cumulative cost is the sum of both iterations, and the
    // per-iteration footer's running total already includes the iteration it
    // reports on.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("cost=+${COST_USD:.2} (cum ${COST_USD:.2})")),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "cost=+${COST_USD:.2} (cum ${:.2})",
            COST_USD * 2.0
        )),
        "{stdout}"
    );
    let loop_summary = ws.read(".ralph/agent/summary.md");
    assert!(
        loop_summary.contains(&format!("**Est. cost:** ${:.2}", COST_USD * 2.0)),
        "{loop_summary}"
    );
}

#[test]
fn headless_claude_max_cost_terminates_after_first_iteration() {
    let ws = Workspace::new("  max_iterations: 5\n  max_cost_usd: 0.10\n");

    let output = ws.run();

    assert_eq!(output.status.code(), Some(2), "{}", describe(&output));
    assert_eq!(ws.backend_launches(), 1, "{}", describe(&output));
    let termination = ws.termination();
    assert!(termination.contains("max_cost"), "{termination}");
    let summaries = ws.summaries();
    assert_eq!(summaries.len(), 1, "{summaries:?}");
    assert_eq!(summaries[0]["cost_usd"], COST_USD, "{}", summaries[0]);
}

#[test]
fn headless_claude_iteration_tokens_reach_performance_diagnostics() {
    let ws = Workspace::new("  max_iterations: 1\n");

    let output = ws.run_with_env(&[("RALPH_DIAGNOSTICS", "1")]);

    let sessions: Vec<_> = std::fs::read_dir(ws.path().join(".ralph/diagnostics"))
        .unwrap_or_else(|e| panic!("diagnostics dir: {e}\n{}", describe(&output)))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    let entries: Vec<serde_json::Value> =
        std::fs::read_to_string(sessions[0].join("performance.jsonl"))
            .expect("performance.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).expect("performance entry"))
            .collect();

    let tokens = entries
        .iter()
        .find(|entry| entry["metric"]["type"] == "token_count")
        .unwrap_or_else(|| panic!("no token_count entry: {entries:?}"));
    assert_eq!(tokens["iteration"], 1, "{tokens}");
    assert_eq!(tokens["metric"]["input"], PEAK_INPUT_TOKENS, "{tokens}");
    assert_eq!(tokens["metric"]["output"], OUTPUT_TOKENS, "{tokens}");
    assert!(
        entries
            .iter()
            .any(|entry| entry["metric"]["type"] == "iteration_duration"),
        "{entries:?}"
    );
}
