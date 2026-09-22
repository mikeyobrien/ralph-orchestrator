//! Fixture-driven fake `autoloop` binary for integration and E2E tests.
//!
//! A fixture is JSONL with one invocation object per line. Each invocation
//! contains ordered `events`, `stream`, `barrier`, `journal`, `stdout`, `stderr`,
//! `summary`, and `exit` steps. The generated executable dispatches successive
//! calls to successive fixture lines, replaying the final line after the fixture
//! is exhausted.

use serde::Deserialize;
use std::fs;
use std::io;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

/// A materialized fixture-driven fake `autoloop` executable.
#[derive(Debug)]
pub struct FakeAutoloop {
    bin_dir: PathBuf,
    argv_out: PathBuf,
    env_out: PathBuf,
    summary_out: PathBuf,
}

impl FakeAutoloop {
    /// Directory containing the generated `autoloop` executable.
    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    /// Suggested path for the generated executable's `ARGV_OUT` variable.
    pub fn argv_out(&self) -> &Path {
        &self.argv_out
    }

    /// Read arguments recorded by the most recent invocation.
    pub fn recorded_argv(&self) -> io::Result<Vec<String>> {
        Ok(fs::read_to_string(&self.argv_out)?
            .lines()
            .map(str::to_owned)
            .collect())
    }

    /// Suggested path for recording Ralph's engine-state environment exports.
    pub fn env_out(&self) -> &Path {
        &self.env_out
    }

    /// Read engine-state environment exports recorded by the latest invocation.
    pub fn recorded_engine_env(&self) -> io::Result<Vec<String>> {
        Ok(fs::read_to_string(&self.env_out)?
            .lines()
            .map(str::to_owned)
            .collect())
    }

    /// Suggested path for recording resolved journal/memory summary paths.
    pub fn summary_out(&self) -> &Path {
        &self.summary_out
    }

    /// Read resolved paths emitted by the latest summary step.
    pub fn recorded_summary_paths(&self) -> io::Result<Vec<PathBuf>> {
        Ok(fs::read_to_string(&self.summary_out)?
            .lines()
            .map(PathBuf::from)
            .collect())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Step {
    Events(EventsStep),
    Stream(StreamStep),
    Barrier(BarrierStep),
    Journal(JournalStep),
    Stdout(StdoutStep),
    Stderr(StderrStep),
    Summary(SummaryStep),
    Exit(ExitStep),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventsStep {
    events: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamStep {
    stream: Stream,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stream {
    /// Absolute path, or a path relative to the fake process workspace.
    path: PathBuf,
    lines: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StdoutStep {
    stdout: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StderrStep {
    stderr: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BarrierStep {
    barrier: Barrier,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Barrier {
    ready_env: String,
    release_env: String,
    #[serde(default)]
    then_exit: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalStep {
    journal: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SummaryStep {
    summary: Summary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    run_id: String,
    iterations: u64,
    stop_reason: String,
    #[serde(default)]
    cost_usd: Option<serde_json::Number>,
    journal: String,
    memory: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExitStep {
    exit: i32,
}

/// Build an executable fake `autoloop` from a JSONL invocation fixture.
///
/// Generated files are contained beneath `dir`. At runtime, arguments are
/// written one per line to `ARGV_OUT` and engine-state environment exports are
/// written to `ENV_OUT` when those variables are set.
pub fn build_fake_autoloop(dir: &Path, fixture: &Path) -> io::Result<FakeAutoloop> {
    let invocations = parse_fixture(fixture)?;
    if invocations.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "fake autoloop fixture contains no invocations",
        ));
    }

    let bin_dir = dir.join("bin");
    let state_dir = dir.join("state");
    let payload_dir = dir.join("payloads");
    fs::create_dir_all(&bin_dir)?;
    fs::create_dir_all(&state_dir)?;
    fs::create_dir_all(&payload_dir)?;

    for (index, invocation) in invocations.iter().enumerate() {
        let script = invocation_script(invocation, index + 1, &payload_dir)?;
        write_executable(
            &state_dir.join(format!("invocation-{}.sh", index + 1)),
            &script,
        )?;
    }

    let count_path = state_dir.join("invocation-count");
    let argv_out = state_dir.join("argv.out");
    let env_out = state_dir.join("env.out");
    let summary_out = state_dir.join("summary.out");
    let mut dispatcher = String::from("#!/bin/sh\nset -eu\n");
    dispatcher.push_str(
        "if [ -n \"${ARGV_OUT:-}\" ]; then\n  printf '%s\\n' \"$@\" > \"$ARGV_OUT\"\nfi\nif [ -n \"${ENV_OUT:-}\" ]; then\n  {\n    printf 'AUTOLOOP_STATE_DIR=%s\\n' \"${AUTOLOOP_STATE_DIR:-}\"\n    printf 'AUTOLOOP_JOURNAL_FILE=%s\\n' \"${AUTOLOOP_JOURNAL_FILE:-}\"\n    printf 'AUTOLOOP_MEMORY_FILE=%s\\n' \"${AUTOLOOP_MEMORY_FILE:-}\"\n    printf 'AUTOLOOP_TASKS_FILE=%s\\n' \"${AUTOLOOP_TASKS_FILE:-}\"\n  } > \"$ENV_OUT\"\nfi\n",
    );
    dispatcher.push_str(&format!(
        "count_path={}\ncount=0\nif [ -f \"$count_path\" ]; then count=$(cat \"$count_path\"); fi\ncount=$((count + 1))\nprintf '%s\\n' \"$count\" > \"$count_path\"\n",
        shell_quote(&count_path)
    ));
    dispatcher.push_str("case \"$count\" in\n");
    for index in 1..=invocations.len() {
        dispatcher.push_str(&format!(
            "  {index}) invocation_script={} ;;\n",
            shell_quote(&state_dir.join(format!("invocation-{index}.sh")))
        ));
    }
    dispatcher.push_str(&format!(
        "  *) invocation_script={} ;;\nesac\n",
        shell_quote(&state_dir.join(format!("invocation-{}.sh", invocations.len())))
    ));
    // Read the selected script with `sh` instead of `exec`ing it. `exec` of a
    // fixture-written file is rejected with ETXTBSY for as long as any
    // descriptor on it is open for writing, and under a parallel suite a fork
    // can leak an inherited copy of that descriptor past the writer's own
    // close. That failure happens one process deeper than `fixture_spawn` can
    // see, because the outer spawn of `bin/autoloop` succeeds; passing the
    // script to the interpreter has no such failure mode. The exit status still
    // propagates, since the invocation is the shell's last command.
    dispatcher.push_str("sh \"$invocation_script\" \"$@\"\n");
    write_executable(&bin_dir.join("autoloop"), &dispatcher)?;

    Ok(FakeAutoloop {
        bin_dir,
        argv_out,
        env_out,
        summary_out,
    })
}

fn parse_fixture(path: &Path) -> io::Result<Vec<Invocation>> {
    let contents = fs::read_to_string(path)?;
    contents
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str(line).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid fake autoloop fixture line {}: {error}", index + 1),
                )
            })
        })
        .collect()
}

fn invocation_script(
    invocation: &Invocation,
    invocation_index: usize,
    payload_dir: &Path,
) -> io::Result<String> {
    let mut script = String::from("#!/bin/sh\nset -eu\nevents_path=\n");
    let mut payload_index = 0;

    for step in &invocation.steps {
        match step {
            Step::Events(step) => {
                payload_index += 1;
                let payload = payload_dir.join(format!(
                    "invocation-{invocation_index}-step-{payload_index}-events"
                ));
                write_lines(&payload, &step.events)?;
                script.push_str(
                    "if [ -z \"$events_path\" ]; then\n  while [ \"$#\" -gt 0 ]; do\n    if [ \"$1\" = \"--events\" ]; then\n      shift\n      if [ \"$#\" -eq 0 ]; then echo 'fake autoloop: --events requires a path' >&2; exit 64; fi\n      events_path=$1\n      break\n    fi\n    shift\n  done\n  if [ -z \"$events_path\" ]; then echo 'fake autoloop: missing required --events argument' >&2; exit 64; fi\nfi\n",
                );
                script.push_str(&format!(
                    "cat {} >> \"$events_path\"\n",
                    shell_quote(&payload)
                ));
            }
            Step::Stream(step) => {
                payload_index += 1;
                let payload = payload_dir.join(format!(
                    "invocation-{invocation_index}-step-{payload_index}-stream"
                ));
                write_lines(&payload, &step.stream.lines)?;
                script.push_str(&format!(
                    "stream_path={}\nmkdir -p \"$(dirname \"$stream_path\")\"\ncat {} >> \"$stream_path\"\n",
                    stream_path_expr(&step.stream.path),
                    shell_quote(&payload),
                ));
            }
            Step::Barrier(step) => {
                validate_env_name(&step.barrier.ready_env)?;
                validate_env_name(&step.barrier.release_env)?;
                let then_exit = step
                    .barrier
                    .then_exit
                    .map(|code| format!("  exit {code}\n"))
                    .unwrap_or_default();
                script.push_str(&format!(
                    "ready_path=${{{ready}:-}}\nif [ -n \"$ready_path\" ]; then\n  release_path=${{{release}:-}}\n  if [ -z \"$release_path\" ]; then echo 'fake autoloop: {release} must be set when {ready} is set' >&2; exit 64; fi\n  touch \"$ready_path\"\n  while [ ! -e \"$release_path\" ]; do sleep 0.01; done\n{then_exit}fi\n",
                    ready = step.barrier.ready_env,
                    release = step.barrier.release_env,
                ));
            }
            Step::Journal(step) => {
                payload_index += 1;
                let payload = payload_dir.join(format!(
                    "invocation-{invocation_index}-step-{payload_index}-journal"
                ));
                write_lines(&payload, &step.journal)?;
                script.push_str("journal_path=${JOURNAL_OUT:-${AUTOLOOP_STATE_DIR:-./.autoloop}/journal.jsonl}\nmkdir -p \"$(dirname \"$journal_path\")\"\n");
                script.push_str(&format!(
                    "cat {} >> \"$journal_path\"\n",
                    shell_quote(&payload)
                ));
            }
            Step::Stdout(step) => {
                payload_index += 1;
                let payload = payload_dir.join(format!(
                    "invocation-{invocation_index}-step-{payload_index}-stdout"
                ));
                write_lines(&payload, &step.stdout)?;
                script.push_str(&format!("cat {}\n", shell_quote(&payload)));
            }
            Step::Stderr(step) => {
                payload_index += 1;
                let payload = payload_dir.join(format!(
                    "invocation-{invocation_index}-step-{payload_index}-stderr"
                ));
                write_lines(&payload, &step.stderr)?;
                script.push_str(&format!("cat {} >&2\n", shell_quote(&payload)));
            }
            Step::Summary(step) => {
                payload_index += 1;
                let payload = payload_dir.join(format!(
                    "invocation-{invocation_index}-step-{payload_index}-summary"
                ));
                fs::write(&payload, summary_prefix(&step.summary))?;
                let journal = stream_path_expr(Path::new(&step.summary.journal));
                let memory = stream_path_expr(Path::new(&step.summary.memory));
                script.push_str(&format!(
                    "cat {}\nprintf 'journal: %s\\nmemory: %s\\n' {journal} {memory}\nif [ -n \"${{SUMMARY_OUT:-}}\" ]; then printf '%s\\n%s\\n' {journal} {memory} > \"$SUMMARY_OUT\"; fi\n",
                    shell_quote(&payload)
                ));
            }
            Step::Exit(step) => script.push_str(&format!("exit {}\n", step.exit)),
        }
    }

    script.push_str("exit 0\n");
    Ok(script)
}

fn summary_prefix(summary: &Summary) -> String {
    let mut text = format!(
        "autoloops summary\n===================\nrun_id: {}\niterations: {}\nstop_reason: {}\n",
        summary.run_id, summary.iterations, summary.stop_reason
    );
    if let Some(cost_usd) = &summary.cost_usd {
        text.push_str(&format!("cost_usd: {cost_usd}\n"));
    }
    text
}

fn write_lines(path: &Path, lines: &[String]) -> io::Result<()> {
    let mut contents = lines.join("\n");
    if !lines.is_empty() {
        contents.push('\n');
    }
    fs::write(path, contents)
}

fn validate_env_name(name: &str) -> io::Result<()> {
    let mut chars = name.chars();
    let valid = matches!(chars.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid barrier environment variable name: {name}"),
        ))
    }
}

/// Shell expression for a stream path. A `${AUTOLOOP_STATE_DIR}/` prefix
/// expands at runtime like the real engine's state-dir contract: the env var
/// wins, with autoloop's standalone `.autoloop` default as the fallback.
fn stream_path_expr(path: &Path) -> String {
    let raw = path.to_string_lossy();
    match raw.strip_prefix("${AUTOLOOP_STATE_DIR}/") {
        Some(suffix) => format!(
            "\"${{AUTOLOOP_STATE_DIR:-./.autoloop}}\"/{}",
            shell_quote(Path::new(suffix))
        ),
        None => shell_quote(path),
    }
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
}

fn write_executable(path: &Path, contents: &str) -> io::Result<()> {
    // Scope the handle so the write descriptor is closed before this function
    // returns. An open write descriptor on a script makes `exec` fail with
    // ETXTBSY, and a descriptor inherited by a concurrent fork extends that
    // window past this call; `fixture_spawn` is the matching guard.
    {
        let mut file = fs::File::create(path)?;
        file.write_all(contents.as_bytes())?;
    }
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

/// How long a fixture exec may stay `ETXTBSY` before the error is surfaced.
const EXEC_BUSY_RETRY_BUDGET: Duration = Duration::from_secs(2);

/// Pause between `ETXTBSY` retries. The window is a fork's fork-to-exec span,
/// so it is short; the loop is bounded by [`EXEC_BUSY_RETRY_BUDGET`].
const EXEC_BUSY_RETRY_INTERVAL: Duration = Duration::from_millis(2);

/// Spawn a fixture-built command, retrying a transient `ETXTBSY`.
///
/// `exec` of a script fails with `ETXTBSY` for as long as any descriptor on it
/// is open for writing. Writing the fixture closes its own descriptor, but a
/// thread that forks during that write window leaks its inherited copy of the
/// descriptor into the child, which keeps the script write-locked until that
/// child execs. Under a parallel suite the next exec can land in that window,
/// so retry it instead of failing a green test. Only `bin/autoloop` is exec'd
/// by the fixture: the generated invocation scripts are read by `sh`, which
/// cannot hit this failure mode.
///
/// The retry is time-bounded on purpose: a script that stays write-locked for
/// longer than the budget is a real fault (a genuine writer, a stale
/// descriptor), and its original `ETXTBSY` is returned rather than masked.
///
/// The parameter is `&mut Command` because `Command`'s builder methods return
/// `&mut Command`, so a chained build can be handed over without rebinding.
pub fn fixture_spawn(command: &mut Command) -> io::Result<Child> {
    spawn_with_busy_retry(command, EXEC_BUSY_RETRY_BUDGET)
}

/// [`fixture_spawn`] plus `wait`, matching `Command::status`.
pub fn fixture_status(command: &mut Command) -> io::Result<ExitStatus> {
    fixture_spawn(command)?.wait()
}

/// [`fixture_spawn`] plus captured output, matching `Command::output`.
pub fn fixture_output(command: &mut Command) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    fixture_spawn(command)?.wait_with_output()
}

fn spawn_with_busy_retry(command: &mut Command, budget: Duration) -> io::Result<Child> {
    let deadline = Instant::now() + budget;
    loop {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error) if is_executable_busy(&error) && Instant::now() < deadline => {
                std::thread::sleep(EXEC_BUSY_RETRY_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_executable_busy(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::ExecutableFileBusy
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    /// Open `path` for writing (append, so the fixture bytes survive) and keep
    /// that descriptor open until the returned sender is dropped. This is the
    /// kernel state a concurrent `fork` in another test thread leaks when it
    /// inherits the fixture's write descriptor.
    fn hold_write_descriptor(
        path: &Path,
    ) -> (mpsc::Receiver<()>, mpsc::Sender<()>, thread::JoinHandle<()>) {
        let (opened_tx, opened_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let path = path.to_path_buf();
        let handle = thread::spawn(move || {
            let file = fs::OpenOptions::new().append(true).open(&path).unwrap();
            opened_tx.send(()).unwrap();
            let _ = release_rx.recv();
            drop(file);
        });
        (opened_rx, release_tx, handle)
    }

    fn fixture(dir: &Path, contents: &str) -> PathBuf {
        let path = dir.join("fixture.jsonl");
        fs::write(&path, contents).unwrap();
        path
    }

    fn command(fake: &FakeAutoloop) -> Command {
        let mut command = Command::new(fake.bin_dir().join("autoloop"));
        command.env("ARGV_OUT", fake.argv_out());
        command.env("SUMMARY_OUT", fake.summary_out());
        // Isolate from any ambient engine state configuration.
        command.env_remove("AUTOLOOP_STATE_DIR");
        command
    }

    #[test]
    fn records_argv_and_writes_events() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            r#"{"steps":[{"events":["{\"type\":\"one\"}","{\"type\":\"two\"}"]}]}"#,
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();
        let events = temp.path().join("events.ndjson");

        let output = fixture_output(command(&fake).args([
            "run",
            "--events",
            events.to_str().unwrap(),
            "--flag",
        ]))
        .unwrap();

        assert!(output.status.success());
        assert_eq!(
            fake.recorded_argv().unwrap(),
            vec!["run", "--events", events.to_str().unwrap(), "--flag"]
        );
        assert_eq!(
            fs::read_to_string(events).unwrap(),
            "{\"type\":\"one\"}\n{\"type\":\"two\"}\n"
        );
    }

    #[test]
    fn writes_backend_stream_lines_relative_to_the_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            r#"{"steps":[{"stream":{"path":".autoloop/runs/live/pi-stream.1.jsonl","lines":["{\"type\":\"one\"}","{\"type\":\"two\"}"]}}]}"#,
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        assert!(
            fixture_status(command(&fake).current_dir(&workspace))
                .unwrap()
                .success()
        );
        assert_eq!(
            fs::read_to_string(workspace.join(".autoloop/runs/live/pi-stream.1.jsonl")).unwrap(),
            "{\"type\":\"one\"}\n{\"type\":\"two\"}\n"
        );
    }

    #[test]
    fn state_dir_env_relocates_tokenized_stream_paths() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            r#"{"steps":[{"stream":{"path":"${AUTOLOOP_STATE_DIR}/runs/live/pi-stream.1.jsonl","lines":["{\"type\":\"one\"}"]}}]}"#,
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let state_dir = workspace.join(".ralph/autoloop");

        assert!(
            fixture_status(
                command(&fake)
                    .current_dir(&workspace)
                    .env("AUTOLOOP_STATE_DIR", &state_dir)
            )
            .unwrap()
            .success()
        );
        assert_eq!(
            fs::read_to_string(state_dir.join("runs/live/pi-stream.1.jsonl")).unwrap(),
            "{\"type\":\"one\"}\n"
        );
        assert!(
            !workspace.join(".autoloop").exists(),
            "state-dir override must not leave a top-level .autoloop"
        );
    }

    #[test]
    fn tokenized_stream_paths_default_to_standalone_state_dir() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            r#"{"steps":[{"stream":{"path":"${AUTOLOOP_STATE_DIR}/runs/live/pi-stream.1.jsonl","lines":["{\"type\":\"one\"}"]}}]}"#,
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        assert!(
            fixture_status(
                command(&fake)
                    .current_dir(&workspace)
                    .env_remove("AUTOLOOP_STATE_DIR")
            )
            .unwrap()
            .success()
        );
        assert_eq!(
            fs::read_to_string(workspace.join(".autoloop/runs/live/pi-stream.1.jsonl")).unwrap(),
            "{\"type\":\"one\"}\n"
        );
    }

    #[test]
    fn journal_honors_state_dir_env_when_journal_out_is_unset() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(temp.path(), r#"{"steps":[{"journal":["{\"entry\":1}"]}]}"#);
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let state_dir = workspace.join(".ralph/autoloop");

        assert!(
            fixture_status(
                command(&fake)
                    .current_dir(&workspace)
                    .env_remove("JOURNAL_OUT")
                    .env("AUTOLOOP_STATE_DIR", &state_dir)
            )
            .unwrap()
            .success()
        );
        assert_eq!(
            fs::read_to_string(state_dir.join("journal.jsonl")).unwrap(),
            "{\"entry\":1}\n"
        );
        assert!(
            !workspace.join(".autoloop").exists(),
            "state-dir override must not leave a top-level .autoloop"
        );
    }

    #[test]
    fn missing_events_argument_fails_loudly() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(temp.path(), r#"{"steps":[{"events":["{}"]}]}"#);
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();

        let output = fixture_output(command(&fake).arg("run")).unwrap();

        assert_eq!(output.status.code(), Some(64));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("fake autoloop: missing required --events argument")
        );
    }

    #[test]
    fn barrier_is_skipped_when_ready_environment_is_unset() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            r#"{"steps":[{"barrier":{"ready_env":"TEST_READY","release_env":"TEST_RELEASE"}},{"summary":{"run_id":"skipped","iterations":1,"stop_reason":"completed","journal":"/tmp/j","memory":"/tmp/m"}}]}"#,
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();

        let output = fixture_output(
            command(&fake)
                .env_remove("TEST_READY")
                .env_remove("TEST_RELEASE"),
        )
        .unwrap();

        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("run_id: skipped"));
    }

    #[test]
    fn barrier_touches_ready_and_waits_for_release() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            r#"{"steps":[{"barrier":{"ready_env":"TEST_READY","release_env":"TEST_RELEASE","then_exit":9}}]}"#,
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();
        let ready = temp.path().join("ready");
        let release = temp.path().join("release");
        let mut child = fixture_spawn(
            command(&fake)
                .env("TEST_READY", &ready)
                .env("TEST_RELEASE", &release),
        )
        .unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while !ready.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ready.exists(), "fake did not signal barrier readiness");
        assert!(child.try_wait().unwrap().is_none(), "fake did not wait");
        fs::write(release, "").unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(9));
    }

    #[test]
    fn journal_uses_default_path_and_override() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(temp.path(), r#"{"steps":[{"journal":["{\"entry\":1}"]}]}"#);
        let fake_one = build_fake_autoloop(&temp.path().join("fake-one"), &fixture).unwrap();
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        assert!(
            fixture_status(command(&fake_one).current_dir(&workspace))
                .unwrap()
                .success()
        );
        assert_eq!(
            fs::read_to_string(workspace.join(".autoloop/journal.jsonl")).unwrap(),
            "{\"entry\":1}\n"
        );

        let fake_two = build_fake_autoloop(&temp.path().join("fake-two"), &fixture).unwrap();
        let override_path = temp.path().join("custom/journal.jsonl");
        assert!(
            fixture_status(command(&fake_two).env("JOURNAL_OUT", &override_path))
                .unwrap()
                .success()
        );
        assert_eq!(
            fs::read_to_string(override_path).unwrap(),
            "{\"entry\":1}\n"
        );
    }

    #[test]
    fn emits_ordered_stdout_and_stderr_around_parseable_summary() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            r#"{"steps":[{"stdout":["before summary"]},{"stderr":["[autoloops] [info] booted","engine detail"]},{"summary":{"run_id":"with-noise","iterations":2,"stop_reason":"completed","cost_usd":0.01,"journal":"/tmp/j","memory":"/tmp/m"}},{"stdout":["after summary"]}]}"#,
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();

        let output = fixture_output(&mut command(&fake)).unwrap();

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "[autoloops] [info] booted\nengine detail\n"
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            stdout,
            concat!(
                "before summary\n",
                "autoloops summary\n",
                "===================\n",
                "run_id: with-noise\n",
                "iterations: 2\n",
                "stop_reason: completed\n",
                "cost_usd: 0.01\n",
                "journal: /tmp/j\n",
                "memory: /tmp/m\n",
                "after summary\n",
            )
        );
        assert!(stdout.contains(
            "autoloops summary\n===================\nrun_id: with-noise\niterations: 2\nstop_reason: completed\n"
        ));
    }

    #[test]
    fn summary_paths_expand_the_runtime_state_root() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            r#"{"steps":[{"summary":{"run_id":"owned","iterations":1,"stop_reason":"completed","journal":"${AUTOLOOP_STATE_DIR}/journal.jsonl","memory":"${AUTOLOOP_STATE_DIR}/memory.jsonl"}}]}"#,
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();
        let root = temp.path().join("workspace/.ralph/autoloop");

        let output = fixture_output(command(&fake).env("AUTOLOOP_STATE_DIR", &root)).unwrap();

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(&format!(
            "journal: {}",
            root.join("journal.jsonl").display()
        )));
        assert!(stdout.contains(&format!("memory: {}", root.join("memory.jsonl").display())));
        assert_eq!(
            fake.recorded_summary_paths().unwrap(),
            vec![root.join("journal.jsonl"), root.join("memory.jsonl")]
        );
    }

    #[test]
    fn summary_has_canonical_shape_and_optional_cost() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            concat!(
                r#"{"steps":[{"summary":{"run_id":"with-cost","iterations":2,"stop_reason":"completed","cost_usd":0.01,"journal":"/tmp/j","memory":"/tmp/m"}}]}"#,
                "\n",
                r#"{"steps":[{"summary":{"run_id":"without-cost","iterations":3,"stop_reason":"max_iterations","journal":"/tmp/j2","memory":"/tmp/m2"}}]}"#
            ),
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();

        let first = fixture_output(&mut command(&fake)).unwrap();
        let second = fixture_output(&mut command(&fake)).unwrap();

        assert_eq!(
            String::from_utf8(first.stdout).unwrap(),
            "autoloops summary\n===================\nrun_id: with-cost\niterations: 2\nstop_reason: completed\ncost_usd: 0.01\njournal: /tmp/j\nmemory: /tmp/m\n"
        );
        assert_eq!(
            String::from_utf8(second.stdout).unwrap(),
            "autoloops summary\n===================\nrun_id: without-cost\niterations: 3\nstop_reason: max_iterations\njournal: /tmp/j2\nmemory: /tmp/m2\n"
        );
    }

    #[test]
    fn supports_nonzero_exit_and_replays_last_invocation() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(
            temp.path(),
            concat!(
                r#"{"steps":[{"exit":7}]}"#,
                "\n",
                r#"{"steps":[{"summary":{"run_id":"last","iterations":1,"stop_reason":"completed","journal":"/tmp/j","memory":"/tmp/m"}},{"exit":3}]}"#
            ),
        );
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();

        assert_eq!(fixture_status(&mut command(&fake)).unwrap().code(), Some(7));
        for _ in 0..2 {
            let output = fixture_output(&mut command(&fake)).unwrap();
            assert_eq!(output.status.code(), Some(3));
            assert!(String::from_utf8_lossy(&output.stdout).contains("run_id: last"));
        }
    }

    #[test]
    fn fixture_exec_retries_a_transient_executable_busy() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(temp.path(), r#"{"steps":[{"exit":0}]}"#);
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();

        let (opened, release, holder) = hold_write_descriptor(&fake.bin_dir().join("autoloop"));
        opened.recv().unwrap();

        // The hazard, measured: while any descriptor holds the script open for
        // writing, a raw exec is rejected with ETXTBSY.
        let busy = Command::new(fake.bin_dir().join("autoloop"))
            .status()
            .unwrap_err();
        assert_eq!(busy.kind(), io::ErrorKind::ExecutableFileBusy);

        // The fixture helper rides the transient out rather than failing a
        // green suite: it retries until the holder releases the descriptor.
        let exec = thread::spawn(move || fixture_status(&mut command(&fake)));
        thread::sleep(Duration::from_millis(50));
        release.send(()).unwrap();
        assert!(exec.join().unwrap().unwrap().success());
        holder.join().unwrap();
    }

    #[test]
    fn fixture_exec_reports_a_persistent_executable_busy() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(temp.path(), r#"{"steps":[{"exit":0}]}"#);
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();

        let (opened, release, holder) = hold_write_descriptor(&fake.bin_dir().join("autoloop"));
        opened.recv().unwrap();

        let budget = Duration::from_millis(80);
        let started = Instant::now();
        let error = spawn_with_busy_retry(&mut command(&fake), budget).unwrap_err();
        let waited = started.elapsed();

        // Bounded, and loud: a script that stays write-locked past the budget
        // still surfaces the original error instead of being masked.
        assert_eq!(error.kind(), io::ErrorKind::ExecutableFileBusy);
        assert!(
            waited >= Duration::from_millis(50),
            "expected retries, waited {waited:?}"
        );
        assert!(
            waited < Duration::from_secs(2),
            "retry must be bounded, waited {waited:?}"
        );

        drop(release);
        holder.join().unwrap();
    }

    #[test]
    fn dispatcher_reads_the_invocation_script_instead_of_exec_ing_it() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(temp.path(), r#"{"steps":[{"stdout":["inner"]}]}"#);
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();
        // `argv_out` lives in the state dir beside the generated scripts.
        let invocation = fake.argv_out().parent().unwrap().join("invocation-1.sh");

        let (opened, release, holder) = hold_write_descriptor(&invocation);
        opened.recv().unwrap();

        // The hazard is live: exec'ing the invocation script as a file is
        // rejected while the descriptor is held, and the outer spawn of
        // `bin/autoloop` has no way to observe that inner failure.
        let busy = Command::new(&invocation).status().unwrap_err();
        assert_eq!(busy.kind(), io::ErrorKind::ExecutableFileBusy);

        // The dispatcher hands the script to `sh` instead, so the same held
        // descriptor cannot fail the invocation and the exit status survives.
        let output = fixture_output(&mut command(&fake)).unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "inner\n");

        drop(release);
        holder.join().unwrap();
    }
}
