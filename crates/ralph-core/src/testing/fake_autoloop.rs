//! Fixture-driven fake `autoloop` binary for integration and E2E tests.
//!
//! A fixture is JSONL with one invocation object per line. Each invocation
//! contains ordered `events`, `barrier`, `journal`, `summary`, and `exit`
//! steps. The generated executable dispatches successive calls to successive
//! fixture lines, replaying the final line after the fixture is exhausted.

use serde::Deserialize;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// A materialized fixture-driven fake `autoloop` executable.
#[derive(Debug)]
pub struct FakeAutoloop {
    bin_dir: PathBuf,
    argv_out: PathBuf,
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
    Barrier(BarrierStep),
    Journal(JournalStep),
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
/// written one per line to `ARGV_OUT` when that variable is set.
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
    let mut dispatcher = String::from("#!/bin/sh\nset -eu\n");
    dispatcher.push_str(
        "if [ -n \"${ARGV_OUT:-}\" ]; then\n  printf '%s\\n' \"$@\" > \"$ARGV_OUT\"\nfi\n",
    );
    dispatcher.push_str(&format!(
        "count_path={}\ncount=0\nif [ -f \"$count_path\" ]; then count=$(cat \"$count_path\"); fi\ncount=$((count + 1))\nprintf '%s\\n' \"$count\" > \"$count_path\"\n",
        shell_quote(&count_path)
    ));
    dispatcher.push_str("case \"$count\" in\n");
    for index in 1..=invocations.len() {
        dispatcher.push_str(&format!(
            "  {index}) exec {} \"$@\" ;;\n",
            shell_quote(&state_dir.join(format!("invocation-{index}.sh")))
        ));
    }
    dispatcher.push_str(&format!(
        "  *) exec {} \"$@\" ;;\nesac\n",
        shell_quote(&state_dir.join(format!("invocation-{}.sh", invocations.len())))
    ));
    write_executable(&bin_dir.join("autoloop"), &dispatcher)?;

    Ok(FakeAutoloop { bin_dir, argv_out })
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
                script.push_str("journal_path=${JOURNAL_OUT:-./.autoloop/journal.jsonl}\nmkdir -p \"$(dirname \"$journal_path\")\"\n");
                script.push_str(&format!(
                    "cat {} >> \"$journal_path\"\n",
                    shell_quote(&payload)
                ));
            }
            Step::Summary(step) => {
                payload_index += 1;
                let payload = payload_dir.join(format!(
                    "invocation-{invocation_index}-step-{payload_index}-summary"
                ));
                fs::write(&payload, summary_text(&step.summary))?;
                script.push_str(&format!("cat {}\n", shell_quote(&payload)));
            }
            Step::Exit(step) => script.push_str(&format!("exit {}\n", step.exit)),
        }
    }

    script.push_str("exit 0\n");
    Ok(script)
}

fn summary_text(summary: &Summary) -> String {
    let mut text = format!(
        "autoloops summary\n===================\nrun_id: {}\niterations: {}\nstop_reason: {}\n",
        summary.run_id, summary.iterations, summary.stop_reason
    );
    if let Some(cost_usd) = &summary.cost_usd {
        text.push_str(&format!("cost_usd: {cost_usd}\n"));
    }
    text.push_str(&format!(
        "journal: {}\nmemory: {}\n",
        summary.journal, summary.memory
    ));
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

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
}

fn write_executable(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};

    fn fixture(dir: &Path, contents: &str) -> PathBuf {
        let path = dir.join("fixture.jsonl");
        fs::write(&path, contents).unwrap();
        path
    }

    fn command(fake: &FakeAutoloop) -> Command {
        let mut command = Command::new(fake.bin_dir().join("autoloop"));
        command.env("ARGV_OUT", fake.argv_out());
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

        let output = command(&fake)
            .args(["run", "--events", events.to_str().unwrap(), "--flag"])
            .output()
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
    fn missing_events_argument_fails_loudly() {
        let temp = tempfile::tempdir().unwrap();
        let fixture = fixture(temp.path(), r#"{"steps":[{"events":["{}"]}]}"#);
        let fake = build_fake_autoloop(&temp.path().join("fake"), &fixture).unwrap();

        let output = command(&fake).arg("run").output().unwrap();

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

        let output = command(&fake)
            .env_remove("TEST_READY")
            .env_remove("TEST_RELEASE")
            .output()
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
        let mut child = command(&fake)
            .env("TEST_READY", &ready)
            .env("TEST_RELEASE", &release)
            .spawn()
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
            command(&fake_one)
                .current_dir(&workspace)
                .status()
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
            command(&fake_two)
                .env("JOURNAL_OUT", &override_path)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            fs::read_to_string(override_path).unwrap(),
            "{\"entry\":1}\n"
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

        let first = command(&fake).output().unwrap();
        let second = command(&fake).output().unwrap();

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

        assert_eq!(command(&fake).status().unwrap().code(), Some(7));
        for _ in 0..2 {
            let output = command(&fake).stdout(Stdio::piped()).output().unwrap();
            assert_eq!(output.status.code(), Some(3));
            assert!(String::from_utf8_lossy(&output.stdout).contains("run_id: last"));
        }
    }
}
