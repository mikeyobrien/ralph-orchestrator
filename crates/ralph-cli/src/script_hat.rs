//! Script hats: running a hat's `run:` command instead of a model.
//!
//! The command runs through `HookExecutor` (timeout, output capture, process
//! group), with the trigger events in its environment. It publishes with
//! `"$RALPH_BIN" emit`, so its events come back through the normal JSONL path
//! (scope enforcement, gates, completion). The loop runner turns a failure
//! into `<hat>.failed`; see `ScriptFailure`.

use ralph_core::{
    HookExecutor, HookExecutorContract, HookRunRequest, HookRunResult, ScriptFailure,
    sanitize_for_process,
};
use ralph_proto::{Event, HatId};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

/// Backend label for script iterations in the TUI, RPC, and `iteration.summary`.
pub(crate) const SCRIPT_BACKEND: &str = "script";

/// Cap for `RALPH_TRIGGER_PAYLOAD`. The full payloads are in
/// `RALPH_TRIGGER_EVENTS_FILE`; a single env string over 128 KiB fails spawn.
pub(crate) const TRIGGER_PAYLOAD_ENV_MAX_BYTES: usize = 64 * 1024;

/// Cap for topic and hat-id env values (Ralph's gate convention).
const SHORT_ENV_MAX_BYTES: usize = 1024;

/// Everything one script hat run needs.
pub(crate) struct ScriptHatRequest<'a> {
    pub hat_id: &'a HatId,
    pub argv: Vec<String>,
    /// The consumed trigger events, in arrival order.
    pub triggers: &'a [Event],
    pub iteration: u32,
    pub loop_id: &'a str,
    pub workspace_root: &'a Path,
    /// Absolute path of the events file `ralph emit` should append to.
    pub events_file: &'a Path,
    pub timeout_secs: u64,
    pub max_output_bytes: u64,
}

/// What a script hat run did.
pub(crate) struct ScriptHatReport {
    /// `None` when the command exited 0. `ScriptFailure::NoEvents` is decided
    /// later, once the runner knows whether the command emitted anything.
    pub failure: Option<ScriptFailure>,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

impl ScriptHatReport {
    /// Whether the run counts as a success for `consecutive_failures`.
    pub(crate) fn succeeded(&self) -> bool {
        self.failure
            .as_ref()
            .is_none_or(|failure| !failure.counts_as_failure())
    }

    /// Output lines for the console: stdout, stderr, then a status line.
    pub(crate) fn display_lines(&self, hat_id: &HatId) -> Vec<String> {
        let mut lines: Vec<String> = self.stdout.lines().map(str::to_string).collect();
        lines.extend(self.stderr.lines().map(str::to_string));
        lines.push(self.status_line(hat_id));
        lines
    }

    /// The same output as markdown for stream handlers (TUI, RPC), which
    /// render text as markdown: command output goes in a code block so it
    /// shows verbatim.
    pub(crate) fn display_markdown(&self, hat_id: &HatId) -> String {
        let mut output = String::new();
        for stream in [&self.stdout, &self.stderr] {
            if !stream.is_empty() {
                output.push_str(stream);
                if !stream.ends_with('\n') {
                    output.push('\n');
                }
            }
        }
        let status = self.status_line(hat_id);
        if output.is_empty() {
            format!("{status}\n")
        } else {
            format!("```\n{output}```\n\n{status}\n")
        }
    }

    fn status_line(&self, hat_id: &HatId) -> String {
        let status = match &self.failure {
            None => "command exited 0".to_string(),
            Some(failure) => failure.to_string(),
        };
        format!(
            "[script {}] {status} in {:.1}s",
            hat_id.as_str(),
            self.duration.as_secs_f64()
        )
    }
}

/// Runs a script hat's command and reports how it went. Never fails: a
/// command that cannot run is reported as `ScriptFailure::CouldNotRun`.
pub(crate) async fn execute_script_hat(request: ScriptHatRequest<'_>) -> ScriptHatReport {
    let started = Instant::now();
    let could_not_run = |error: String| ScriptHatReport {
        failure: Some(ScriptFailure::CouldNotRun { error }),
        stdout: String::new(),
        stderr: String::new(),
        duration: started.elapsed(),
    };

    // Kept alive until the command has finished; deleted on drop.
    let triggers_file = match write_triggers_file(request.triggers) {
        Ok(file) => file,
        Err(e) => return could_not_run(format!("could not write trigger events file: {e}")),
    };
    let timeout_secs = request.timeout_secs;
    let hook_request = build_hook_request(&request, triggers_file.path());

    let result = tokio::task::spawn_blocking(move || HookExecutor::new().run(hook_request)).await;
    drop(triggers_file);

    match result {
        Ok(Ok(result)) => report_from_result(result, timeout_secs, started.elapsed()),
        Ok(Err(e)) => could_not_run(e.to_string()),
        Err(e) => could_not_run(format!("script runner task failed: {e}")),
    }
}

fn report_from_result(
    result: HookRunResult,
    timeout_secs: u64,
    duration: Duration,
) -> ScriptHatReport {
    let mut stderr = result.stderr.content;
    if result.stderr.truncated {
        stderr.push_str("\n[stderr truncated at max_output_bytes]");
    }
    let failure = if result.timed_out {
        Some(ScriptFailure::TimedOut {
            seconds: timeout_secs,
            stderr: stderr.clone(),
        })
    } else if result.exit_code == Some(0) {
        None
    } else {
        Some(ScriptFailure::Exited {
            code: result.exit_code,
            stderr: stderr.clone(),
        })
    };
    ScriptHatReport {
        failure,
        stdout: result.stdout.content,
        stderr,
        duration,
    }
}

/// Writes every consumed trigger event, untruncated, as JSONL
/// (`{"topic", "payload"}` plus `source` when known).
fn write_triggers_file(triggers: &[Event]) -> std::io::Result<tempfile::NamedTempFile> {
    let mut file = tempfile::Builder::new()
        .prefix("ralph-script-triggers-")
        .suffix(".jsonl")
        .tempfile()?;
    for event in triggers {
        let mut line = serde_json::json!({
            "topic": event.topic.as_str(),
            "payload": event.payload,
        });
        if let Some(source) = &event.source {
            line["source"] = serde_json::Value::String(source.as_str().to_string());
        }
        writeln!(file, "{line}")?;
    }
    file.flush()?;
    Ok(file)
}

fn build_hook_request(request: &ScriptHatRequest<'_>, triggers_file: &Path) -> HookRunRequest {
    let hat = sanitize_for_process(request.hat_id.as_str(), SHORT_ENV_MAX_BYTES);
    let (topic, payload) = request.triggers.first().map_or_else(
        || (String::new(), String::new()),
        |event| {
            (
                sanitize_for_process(event.topic.as_str(), SHORT_ENV_MAX_BYTES),
                sanitize_for_process(&event.payload, TRIGGER_PAYLOAD_ENV_MAX_BYTES),
            )
        },
    );

    let mut env = HashMap::new();
    if let Ok(current_exe) = std::env::current_exe() {
        // `ralph` on PATH resolves to this binary, as for model hats.
        if let Some(bin_dir) = current_exe.parent() {
            let mut paths = vec![bin_dir.to_path_buf()];
            if let Some(existing) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&existing));
            }
            if let Ok(joined) = std::env::join_paths(paths) {
                env.insert("PATH".to_string(), joined.to_string_lossy().into_owned());
            }
        }
        env.insert(
            "RALPH_BIN".to_string(),
            current_exe.to_string_lossy().into_owned(),
        );
    }
    env.insert(
        "RALPH_WORKSPACE_ROOT".to_string(),
        path_env(request.workspace_root),
    );
    env.insert(
        "RALPH_EVENTS_FILE".to_string(),
        path_env(request.events_file),
    );
    env.insert("RALPH_HAT".to_string(), hat.clone());
    env.insert("RALPH_ITERATION".to_string(), request.iteration.to_string());
    env.insert(
        "RALPH_LOOP_ID".to_string(),
        sanitize_for_process(request.loop_id, SHORT_ENV_MAX_BYTES),
    );
    env.insert("RALPH_TRIGGER_TOPIC".to_string(), topic.clone());
    env.insert("RALPH_TRIGGER_PAYLOAD".to_string(), payload);
    env.insert(
        "RALPH_TRIGGER_EVENTS_FILE".to_string(),
        path_env(triggers_file),
    );

    HookRunRequest {
        phase_event: "script.run".to_string(),
        hook_name: hat.clone(),
        command: request.argv.clone(),
        workspace_root: request.workspace_root.to_path_buf(),
        cwd: None,
        env,
        timeout_seconds: request.timeout_secs,
        max_output_bytes: request.max_output_bytes,
        // Small on purpose: the contract is the environment, and a command
        // that never reads stdin must not block on a full pipe.
        stdin_payload: serde_json::json!({
            "hat": hat,
            "iteration": request.iteration,
            "trigger_topic": topic,
        }),
        // Script hats run inside iterations, after Ralph's signal handlers
        // are installed, so a timeout can kill the command's whole tree.
        own_process_group: true,
    }
}

fn path_env(path: &Path) -> String {
    sanitize_for_process(&path.to_string_lossy(), usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'a>(
        hat_id: &'a HatId,
        triggers: &'a [Event],
        workspace: &'a Path,
        events_file: &'a Path,
        argv: &[&str],
    ) -> ScriptHatRequest<'a> {
        ScriptHatRequest {
            hat_id,
            argv: argv.iter().map(|s| (*s).to_string()).collect(),
            triggers,
            iteration: 4,
            loop_id: "loop-1",
            workspace_root: workspace,
            events_file,
            timeout_secs: 5,
            max_output_bytes: 64 * 1024,
        }
    }

    #[tokio::test]
    async fn trigger_payload_env_is_capped_and_nul_free_but_the_file_has_everything() {
        let dir = tempfile::tempdir().unwrap();
        let hat = HatId::new("stepper");
        let big = format!("a\0b{}", "x".repeat(200 * 1024));
        let triggers = [
            Event::new("step.run", big.clone()),
            Event::new("step.again", "second"),
        ];
        let events_file = dir.path().join("events.jsonl");
        let report = execute_script_hat(request(
            &hat,
            &triggers,
            dir.path(),
            &events_file,
            &[
                "sh",
                "-c",
                "printf '%s' \"${#RALPH_TRIGGER_PAYLOAD}\" > len.txt; cp \"$RALPH_TRIGGER_EVENTS_FILE\" triggers.jsonl",
            ],
        ))
        .await;

        assert!(report.failure.is_none(), "{:?}", report.failure);
        let len: usize = std::fs::read_to_string(dir.path().join("len.txt"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(len, TRIGGER_PAYLOAD_ENV_MAX_BYTES);
        let lines: Vec<serde_json::Value> =
            std::fs::read_to_string(dir.path().join("triggers.jsonl"))
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["payload"], big.as_str(), "file is untruncated");
        assert_eq!(lines[1]["topic"], "step.again");
    }

    #[test]
    fn stream_output_fences_command_output_so_markdown_renders_it_verbatim() {
        let hat = HatId::new("stepper");
        let report = ScriptHatReport {
            failure: None,
            stdout: "# not a heading".to_string(),
            stderr: "warn\n".to_string(),
            duration: Duration::from_millis(100),
        };
        assert_eq!(
            report.display_markdown(&hat),
            "```\n# not a heading\nwarn\n```\n\n[script stepper] command exited 0 in 0.1s\n"
        );

        let quiet = ScriptHatReport {
            stdout: String::new(),
            stderr: String::new(),
            ..report
        };
        assert_eq!(
            quiet.display_markdown(&hat),
            "[script stepper] command exited 0 in 0.1s\n"
        );
    }

    #[tokio::test]
    async fn a_command_that_cannot_start_is_reported_not_raised() {
        let dir = tempfile::tempdir().unwrap();
        let hat = HatId::new("stepper");
        let triggers = [Event::new("step.run", "go")];
        let events_file = dir.path().join("events.jsonl");
        let report = execute_script_hat(request(
            &hat,
            &triggers,
            dir.path(),
            &events_file,
            &["./does-not-exist.sh"],
        ))
        .await;

        assert!(!report.succeeded());
        match report.failure {
            Some(ScriptFailure::CouldNotRun { error }) => {
                assert!(error.contains("does-not-exist.sh"), "{error}");
            }
            other => panic!("expected CouldNotRun, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn non_zero_exit_keeps_the_exit_code_and_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let hat = HatId::new("stepper");
        let triggers = [Event::new("step.run", "go")];
        let events_file = dir.path().join("events.jsonl");
        let report = execute_script_hat(request(
            &hat,
            &triggers,
            dir.path(),
            &events_file,
            &["sh", "-c", "echo out; echo bad >&2; exit 9"],
        ))
        .await;

        assert_eq!(
            report.failure,
            Some(ScriptFailure::Exited {
                code: Some(9),
                stderr: "bad\n".to_string()
            })
        );
        assert_eq!(
            report
                .display_lines(&hat)
                .last()
                .unwrap()
                .split(" in ")
                .next(),
            Some("[script stepper] command exited 9")
        );
    }
}
