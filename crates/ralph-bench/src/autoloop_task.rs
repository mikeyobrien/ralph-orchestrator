use anyhow::{Context, Result};
use ralph_adapters::{AutoloopBin, AutoloopRunner};
use ralph_core::{Record, SessionRecorder, TaskDefinition, TaskWorkspace};
use ralph_proto::TerminalWrite;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const PRESET_DIR: &str = ".ralph/bench-autoloop-preset";
const EVENTS_FILE: &str = ".ralph/bench-autoloop-events.ndjson";

pub async fn run_task_loop(
    task: &TaskDefinition,
    workspace: &TaskWorkspace,
    record_path: Option<&PathBuf>,
    record_ux: bool,
) -> Result<(u32, String)> {
    run_task_loop_with_bin(
        task,
        workspace,
        record_path,
        record_ux,
        AutoloopBin::PathLookup,
    )
    .await
}

async fn run_task_loop_with_bin(
    task: &TaskDefinition,
    workspace: &TaskWorkspace,
    record_path: Option<&PathBuf>,
    record_ux: bool,
    bin: AutoloopBin,
) -> Result<(u32, String)> {
    let prompt_path = workspace.path().join("PROMPT.md");
    let prompt = fs::read_to_string(&prompt_path)
        .with_context(|| format!("Failed to read prompt file: {prompt_path:?}"))?;

    let preset = workspace.path().join(PRESET_DIR);
    write_preset(task, &preset).context("Failed to generate benchmark Autoloop preset")?;

    let events_path = workspace.path().join(EVENTS_FILE);
    let _ = fs::remove_file(&events_path);
    let max_runtime = format!("{}s", task.timeout_seconds);
    let runner = AutoloopRunner::new(preset, prompt, workspace.path())
        .bin(bin)
        .max_iterations(task.max_iterations)
        .set_override("event_loop.max_runtime", &max_runtime)
        .set_override("event_loop.completion_promise", &task.completion_promise)
        .events_path(events_path.clone());

    let started = Instant::now();
    let summary = tokio::task::spawn_blocking(move || runner.run())
        .await
        .context("Autoloop benchmark task panicked")?
        .context("Autoloop benchmark task failed")?;

    if let Some(path) = record_path {
        let events = fs::read_to_string(&events_path)
            .with_context(|| format!("Failed to read Autoloop events from {events_path:?}"))?;
        let file = File::create(path)
            .with_context(|| format!("Failed to create recording file: {path:?}"))?;
        write_recording_with_elapsed(
            task,
            &events,
            record_ux,
            started.elapsed().as_secs_f64(),
            BufWriter::new(file),
        )?;
    }

    Ok((summary.iterations, summary.stop_reason))
}

fn write_preset(task: &TaskDefinition, preset: &Path) -> Result<()> {
    fs::create_dir_all(preset.join("roles"))?;
    fs::write(
        preset.join("autoloops.toml"),
        format!(
            "event_loop.max_iterations = {}\n\
             event_loop.max_runtime = {}\n\
             event_loop.completion_event = \"task.complete\"\n\
             event_loop.completion_promise = {}\n\n\
             harness.instructions_file = \"harness.md\"\n",
            task.max_iterations,
            toml_string(&format!("{}s", task.timeout_seconds)),
            toml_string(&task.completion_promise),
        ),
    )?;
    fs::write(
        preset.join("topology.toml"),
        "name = \"ralph-bench\"\ncompletion = \"task.complete\"\n\n\
         [[role]]\nid = \"ralph\"\nemits = [\"task.complete\"]\n\
         prompt_file = \"roles/ralph.md\"\n\n\
         [handoff]\n\"loop.start\" = [\"ralph\"]\n",
    )?;
    fs::write(
        preset.join("roles/ralph.md"),
        format!(
            "Complete the benchmark objective in the current workspace. Verify the work before finishing. \
             Emit `task.complete` or end your output with `{}` only when complete.\n",
            task.completion_promise
        ),
    )?;
    fs::write(
        preset.join("harness.md"),
        "Work only inside the benchmark workspace. Preserve deterministic task inputs.\n",
    )?;
    Ok(())
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
fn write_recording(
    task: &TaskDefinition,
    events: &str,
    record_ux: bool,
    writer: impl Write,
) -> Result<()> {
    write_recording_with_elapsed(task, events, record_ux, 0.0, writer)
}

fn write_recording_with_elapsed(
    task: &TaskDefinition,
    events: &str,
    record_ux: bool,
    elapsed_secs: f64,
    writer: impl Write,
) -> Result<()> {
    let recorder = SessionRecorder::new(writer);
    recorder.record_meta(Record::meta_loop_start(
        "PROMPT.md",
        task.max_iterations,
        Some("cli"),
    ));

    let mut iterations = 0;
    let mut termination_reason = "Stopped".to_string();
    let mut ux_writes = 0;
    for line in events.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            tracing::warn!("Skipping malformed Autoloop event while recording");
            continue;
        };
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        if kind == "iteration.start"
            && let Some(iteration) = value.get("iteration").and_then(Value::as_u64)
        {
            iterations = iterations.max(iteration as u32);
            recorder.record_meta(Record::meta_iteration(iteration as u32, 0, "autoloop"));
        }
        if matches!(kind, "loop.finish" | "summary") {
            if let Some(count) = value.get("iterations").and_then(Value::as_u64) {
                iterations = count as u32;
            }
            if let Some(reason) = value.get("stopReason").and_then(Value::as_str) {
                termination_reason = reason.to_string();
            }
        }
        if record_ux
            && kind == "backend.output"
            && let Some(output) = value.get("output").and_then(Value::as_str)
        {
            let offset_ms = recorder.elapsed().as_millis() as u64;
            recorder.record_meta(Record::new(
                "ux.terminal.write",
                TerminalWrite::new(output.as_bytes(), true, offset_ms),
            ));
            ux_writes += 1;
        }

        recorder.record_meta(Record::new(format!("autoloop.{kind}"), value));
    }

    recorder.record_meta(Record::meta_termination(
        &termination_reason,
        iterations,
        elapsed_secs,
        ux_writes,
    ));
    recorder
        .flush()
        .context("Failed to flush session recording")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_core::{SessionPlayer, Verification};
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::io::BufReader;
    use std::io::Cursor;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn task() -> TaskDefinition {
        TaskDefinition::builder("fixture", "PROMPT.md", "BENCH_COMPLETE")
            .max_iterations(7)
            .timeout_seconds(13)
            .expected_iterations(2)
            .verification(Verification::new("test -f result.txt"))
            .build()
    }

    #[cfg(unix)]
    fn workspace(task: &TaskDefinition) -> (tempfile::TempDir, TaskWorkspace) {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = TaskWorkspace::create(task, temp.path()).expect("workspace");
        fs::write(workspace.path().join("PROMPT.md"), "write result.txt").expect("prompt");
        (temp, workspace)
    }

    #[cfg(unix)]
    fn fake_autoloop(dir: &std::path::Path, exit_code: i32) -> PathBuf {
        let path = dir.join("fake-autoloop.sh");
        let script = format!(
            r#"#!/usr/bin/env bash
set -eu
printf '%s\n' "$@" > "$PWD/autoloop-argv.txt"
events=''
while [ "$#" -gt 0 ]; do
  if [ "$1" = '--events' ]; then events="$2"; shift 2; else shift; fi
done
if [ {exit_code} -ne 0 ]; then
  echo 'fixture failure' >&2
  exit {exit_code}
fi
mkdir -p "$(dirname "$events")" "$PWD/.autoloop"
printf '%s\n' \
  '{{"type":"iteration.start","iteration":1,"maxIterations":7,"runId":"bench-fixture"}}' \
  '{{"type":"backend.output","iteration":1,"runId":"bench-fixture","output":"fixture output\n"}}' \
  '{{"type":"iteration.start","iteration":2,"maxIterations":7,"runId":"bench-fixture"}}' \
  '{{"type":"loop.finish","iterations":2,"stopReason":"completed","runId":"bench-fixture"}}' > "$events"
touch "$PWD/.autoloop/journal.jsonl" "$PWD/.autoloop/memory.jsonl" "$PWD/result.txt"
printf '%s\n' \
  'autoloops summary' \
  '===================' \
  'run_id: bench-fixture' \
  'iterations: 2' \
  'stop_reason: completed' \
  "journal: $PWD/.autoloop/journal.jsonl" \
  "memory: $PWD/.autoloop/memory.jsonl"
"#
        );
        fs::write(&path, script).expect("fake autoloop");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runs_autoloop_with_task_limits_and_replayable_recording() {
        let task = task();
        let (_temp, workspace) = workspace(&task);
        let bin = fake_autoloop(workspace.path(), 0);
        let recording = workspace.path().join("session.jsonl");

        let result = run_task_loop_with_bin(
            &task,
            &workspace,
            Some(&recording),
            true,
            AutoloopBin::Explicit(bin),
        )
        .await
        .expect("benchmark run");

        assert_eq!(result, (2, "completed".to_string()));
        let preset = fs::read_to_string(
            workspace
                .path()
                .join(".ralph/bench-autoloop-preset/autoloops.toml"),
        )
        .expect("generated config");
        assert!(preset.contains("event_loop.max_iterations = 7"));
        assert!(preset.contains("event_loop.max_runtime = \"13s\""));
        assert!(preset.contains("event_loop.completion_promise = \"BENCH_COMPLETE\""));

        let argv = fs::read_to_string(workspace.path().join("autoloop-argv.txt")).unwrap();
        assert!(argv.lines().any(|arg| arg == "run"));
        assert!(argv.lines().any(|arg| arg == "write result.txt"));
        assert!(argv.lines().any(|arg| arg == "event_loop.max_iterations=7"));
        assert!(argv.lines().any(|arg| arg == "event_loop.max_runtime=13s"));

        let file = fs::File::open(&recording).expect("recording");
        let mut player = SessionPlayer::from_reader(BufReader::new(file)).expect("replay format");
        assert_eq!(player.metadata_events().len(), 4);
        assert_eq!(player.filter_by_event("autoloop.").len(), 4);
        let mut replayed = Vec::new();
        player.replay_terminal(&mut replayed).expect("replay");
        assert_eq!(replayed, b"fixture output\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn preserves_nonzero_autoloop_exit_as_an_error() {
        let task = task();
        let (_temp, workspace) = workspace(&task);
        let bin = fake_autoloop(workspace.path(), 17);

        let error =
            run_task_loop_with_bin(&task, &workspace, None, false, AutoloopBin::Explicit(bin))
                .await
                .expect_err("non-zero autoloop must fail");

        let message = format!("{error:#}");
        assert!(message.contains("code Some(17)"), "{message}");
        // Note: as of integration/v3-prerelease, AutoloopRunner's error surface
        // is "{command} failed (exit code {code:?}); verify the engine
        // configuration and retry" — it does not include the child's stderr.
        // The bench harness must not require stderr content in the error chain.
        assert!(message.contains("failed (exit code Some(17))"), "{message}");
    }

    #[test]
    fn recording_without_ux_remains_valid_and_has_no_terminal_writes() {
        let events = concat!(
            "{\"type\":\"iteration.start\",\"iteration\":1,\"runId\":\"r1\"}\n",
            "not json\n",
            "{\"type\":\"backend.output\",\"iteration\":1,\"runId\":\"r1\",\"output\":\"hidden\"}\n",
            "{\"type\":\"loop.finish\",\"iterations\":1,\"stopReason\":\"completed\",\"runId\":\"r1\"}\n",
        );
        let task = task();
        let mut output = Vec::new();
        write_recording(&task, events, false, &mut output).expect("recording");
        let player = SessionPlayer::from_reader(Cursor::new(output)).expect("replay format");
        assert!(player.terminal_writes().is_empty());
        assert_eq!(player.filter_by_event("autoloop.").len(), 3);
    }
}
