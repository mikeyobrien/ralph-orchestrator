#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use ralph_core::{
    HistoryEventType, LoopEntry, LoopHistory, LoopLock, LoopRegistry, MergeQueue, MergeState,
};

use tempfile::TempDir;

const AUTOLOOPS_TOML: &str = r#"
[event_loop]
max_iterations = 1
completion_event = "task.complete"
"#;

const TOPOLOGY_TOML: &str = r#"
name = "prompt-delivery-test"
completion = "task.complete"

[[role]]
id = "worker"
emits = ["task.complete"]
prompt_file = "roles/worker.md"

[handoff]
"loop.start" = ["worker"]
"#;

struct Harness {
    workspace: TempDir,
    home: TempDir,
    argv_out: PathBuf,
    crash_ready: PathBuf,
    crash_release: PathBuf,
    bin_dir: PathBuf,
}

impl Harness {
    fn new(config_prompt_file: Option<&str>) -> Self {
        let workspace = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");
        let preset = workspace.path().join("preset");
        fs::create_dir_all(preset.join("roles")).expect("create preset roles dir");
        fs::write(preset.join("autoloops.toml"), AUTOLOOPS_TOML).expect("write autoloops.toml");
        fs::write(preset.join("topology.toml"), TOPOLOGY_TOML).expect("write topology.toml");
        fs::write(preset.join("roles/worker.md"), "Complete the test.").expect("write role prompt");

        let prompt_file_config = config_prompt_file
            .map(|path| format!("event_loop:\n  prompt_file: {path}\n"))
            .unwrap_or_default();
        fs::write(
            workspace.path().join("ralph.yml"),
            format!(
                "core:\n  engine: autoloop\n  autoloop_preset: preset\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n{prompt_file_config}"
            ),
        )
        .expect("write ralph.yml");

        fs::write(workspace.path().join("README.md"), "prompt delivery test\n")
            .expect("write README");
        run_git(workspace.path(), &["init", "--quiet"]);
        run_git(
            workspace.path(),
            &["add", "README.md", "ralph.yml", "preset"],
        );
        run_git(
            workspace.path(),
            &[
                "-c",
                "user.name=Ralph Test",
                "-c",
                "user.email=ralph@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "initial",
            ],
        );

        let bin_dir = workspace.path().join("bin");
        fs::create_dir(&bin_dir).expect("create fake bin dir");
        let fake_autoloop = bin_dir.join("autoloop");
        fs::write(
            &fake_autoloop,
            r#"#!/bin/sh
set -eu
: "${ARGV_OUT:?ARGV_OUT must be set}"
printf '%s\n' "$@" > "$ARGV_OUT"
if [ -n "${CRASH_READY:-}" ]; then
  : "${CRASH_RELEASE:?CRASH_RELEASE must be set in crash mode}"
  touch "$CRASH_READY"
  while [ ! -e "$CRASH_RELEASE" ]; do
    sleep 0.01
  done
  exit 1
fi
cat <<'SUMMARY'
autoloops summary
===================
run_id: run-test
iterations: 1
stop_reason: completed
journal: /tmp/autoloop-test-journal.jsonl
memory: /tmp/autoloop-test-memory.jsonl
SUMMARY
"#,
        )
        .expect("write fake autoloop");
        let mut permissions = fs::metadata(&fake_autoloop)
            .expect("fake autoloop metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_autoloop, permissions).expect("make fake autoloop executable");

        let argv_out = workspace.path().join("autoloop-argv.txt");
        let crash_ready = workspace.path().join("autoloop-crash-ready");
        let crash_release = workspace.path().join("autoloop-crash-release");
        Self {
            workspace,
            home,
            argv_out,
            crash_ready,
            crash_release,
            bin_dir,
        }
    }

    fn command(&self, prompt_args: &[&str]) -> Command {
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.bin_dir.clone()];
        paths.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(paths).expect("construct PATH");

        let mut args = vec!["--color", "never", "--config", "ralph.yml", "run"];
        args.extend_from_slice(prompt_args);
        args.extend_from_slice(&["--no-tui", "--skip-preflight"]);

        let mut command = Command::new(env!("CARGO_BIN_EXE_ralph"));
        command
            .args(args)
            .current_dir(self.workspace.path())
            .env("PATH", path)
            .env("ARGV_OUT", &self.argv_out)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID")
            .env_remove("CRASH_READY")
            .env_remove("CRASH_RELEASE")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn run(&self, prompt_args: &[&str]) -> Output {
        self.command(prompt_args).output().expect("execute ralph")
    }

    fn recorded_argv(&self) -> Vec<String> {
        fs::read_to_string(&self.argv_out)
            .expect("fake autoloop should record argv")
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("execute git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "ralph failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_recorded_prompt(harness: &Harness, expected: &str) {
    let argv = harness.recorded_argv();
    assert!(argv.len() >= 3, "unexpected autoloop argv: {argv:?}");
    assert_eq!(argv[0], "run", "unexpected autoloop argv: {argv:?}");
    assert_eq!(
        fs::canonicalize(&argv[1]).expect("canonicalize recorded preset path"),
        fs::canonicalize(harness.workspace.path().join("preset"))
            .expect("canonicalize expected preset path"),
        "unexpected autoloop argv: {argv:?}"
    );
    assert_eq!(argv[2], expected, "unexpected autoloop argv: {argv:?}");
}

#[test]
fn crash_runs_completion_coordination() {
    const LOOP_ID: &str = "ralph-crash-coordination-test";

    let harness = Harness::new(None);
    let queue = MergeQueue::new(harness.workspace.path());
    queue.enqueue(LOOP_ID, "crash coordination test").unwrap();

    let mut command = harness.command(&["-p", "crash after startup"]);
    command
        .env("RALPH_MERGE_LOOP_ID", LOOP_ID)
        .env("CRASH_READY", &harness.crash_ready)
        .env("CRASH_RELEASE", &harness.crash_release);
    let mut child = command.spawn().expect("spawn ralph");

    let deadline = Instant::now() + Duration::from_secs(10);
    while !harness.crash_ready.exists() {
        if let Some(status) = child.try_wait().expect("poll ralph") {
            panic!("ralph exited before fake autoloop reached crash barrier: {status}");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("timed out waiting for fake autoloop crash barrier");
        }
        thread::sleep(Duration::from_millis(10));
    }

    let ralph_pid = child.id();
    let mut registry_entry = LoopEntry::with_id(
        LOOP_ID,
        "crash coordination test",
        None::<String>,
        harness.workspace.path().display().to_string(),
    );
    registry_entry.pid = ralph_pid;
    LoopRegistry::new(harness.workspace.path())
        .register(registry_entry)
        .expect("seed live registry entry");

    fs::write(&harness.crash_release, "release\n").expect("release fake autoloop");
    let output = child.wait_with_output().expect("wait for ralph");

    assert!(
        !output.status.success(),
        "ralph should preserve the autoloop crash as a nonzero exit"
    );

    let history = LoopHistory::new(harness.workspace.path().join(".ralph/history.jsonl"));
    let history_events = history.read_all().expect("read loop history");
    assert!(
        history_events.iter().any(|event| matches!(
            &event.event_type,
            HistoryEventType::LoopCompleted { reason } if reason == "validation_failure"
        )),
        "missing validation_failure completion record; events: {history_events:?}; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        LoopRegistry::new(harness.workspace.path())
            .get(LOOP_ID)
            .expect("read loop registry")
            .is_none(),
        "completion coordination left the live registry entry behind"
    );

    let queue_entry = queue
        .get_entry(LOOP_ID)
        .expect("read merge queue")
        .expect("seeded merge queue entry should remain present");
    assert_eq!(
        queue_entry.state,
        MergeState::NeedsReview,
        "failed merge run should be dispositioned for review"
    );

    let reacquired_lock = LoopLock::try_acquire(harness.workspace.path(), "post-crash probe")
        .expect("primary loop lock should be reacquirable after ralph exits");
    drop(reacquired_lock);
}

#[test]
fn run_inline_prompt_reaches_autoloop_as_positional_argument() {
    let harness = Harness::new(None);

    let output = harness.run(&["-p", "text"]);

    assert_success(&output);
    assert_recorded_prompt(&harness, "text");
}

#[test]
fn run_prompt_file_contents_reach_autoloop_as_positional_argument() {
    let harness = Harness::new(None);
    fs::write(
        harness.workspace.path().join("objective.md"),
        "file prompt contents",
    )
    .expect("write prompt file");

    let output = harness.run(&["-P", "objective.md"]);

    assert_success(&output);
    assert_recorded_prompt(&harness, "file prompt contents");
}

#[test]
fn run_inline_prompt_overrides_stale_configured_prompt_file() {
    let harness = Harness::new(Some("stale-prompt.md"));
    fs::write(
        harness.workspace.path().join("stale-prompt.md"),
        "decoy file prompt",
    )
    .expect("write stale prompt file");

    let output = harness.run(&["-p", "inline wins"]);

    assert_success(&output);
    let argv = harness.recorded_argv();
    assert_recorded_prompt(&harness, "inline wins");
    assert!(
        !argv.iter().any(|arg| arg.contains("decoy file prompt")),
        "stale prompt leaked into autoloop argv: {argv:?}"
    );
}
