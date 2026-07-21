#![cfg(unix)]

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, kill, killpg};
use nix::unistd::Pid;
use ralph_core::{
    HistoryEventType, LoopEntry, LoopHistory, LoopLock, LoopRegistry, MergeQueue, MergeState, Task,
    testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop},
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
    fake_autoloop: FakeAutoloop,
    crash_ready: PathBuf,
    crash_release: PathBuf,
    restart_ready: PathBuf,
    restart_release: PathBuf,
}

impl Harness {
    fn new(config_prompt_file: Option<&str>) -> Self {
        Self::new_with_event_loop(config_prompt_file, &[])
    }

    fn new_with_event_loop(
        config_prompt_file: Option<&str>,
        event_loop_fields: &[(&str, &str)],
    ) -> Self {
        let workspace = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");
        let preset = workspace.path().join("preset");
        fs::create_dir_all(preset.join("roles")).expect("create preset roles dir");
        fs::write(preset.join("autoloops.toml"), AUTOLOOPS_TOML).expect("write autoloops.toml");
        fs::write(preset.join("topology.toml"), TOPOLOGY_TOML).expect("write topology.toml");
        fs::write(preset.join("roles/worker.md"), "Complete the test.").expect("write role prompt");

        let mut event_loop_config = String::new();
        if config_prompt_file.is_some() || !event_loop_fields.is_empty() {
            event_loop_config.push_str("event_loop:\n");
            if let Some(path) = config_prompt_file {
                event_loop_config.push_str(&format!("  prompt_file: {path}\n"));
            }
            for (key, value) in event_loop_fields {
                event_loop_config.push_str(&format!("  {key}: {value}\n"));
            }
        }
        fs::write(
            workspace.path().join("ralph.yml"),
            format!(
                "core:\n  engine: autoloop\n  autoloop_preset: preset\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n{event_loop_config}"
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

        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/autoloop/prompt_delivery.jsonl");
        let fake_autoloop = build_fake_autoloop(&workspace.path().join("fake-autoloop"), &fixture)
            .expect("build fixture-driven fake autoloop");

        let crash_ready = workspace.path().join("autoloop-crash-ready");
        let crash_release = workspace.path().join("autoloop-crash-release");
        let restart_ready = workspace.path().join("autoloop-restart-ready");
        let restart_release = workspace.path().join("autoloop-restart-release");
        Self {
            workspace,
            home,
            fake_autoloop,
            crash_ready,
            crash_release,
            restart_ready,
            restart_release,
        }
    }

    fn command(&self, prompt_args: &[&str]) -> Command {
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.fake_autoloop.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(paths).expect("construct PATH");

        let mut args = vec!["--color", "never", "--config", "ralph.yml", "run"];
        args.extend_from_slice(prompt_args);
        if !prompt_args
            .iter()
            .any(|arg| matches!(*arg, "--autonomous" | "-a"))
        {
            args.push("--no-tui");
        }
        args.push("--skip-preflight");

        let mut command = Command::new(env!("CARGO_BIN_EXE_ralph"));
        command
            .args(args)
            .current_dir(self.workspace.path())
            .env("PATH", path)
            .env("ARGV_OUT", self.fake_autoloop.argv_out())
            .env("ENV_OUT", self.fake_autoloop.env_out())
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

    fn preflight_failure_budget(&self) -> Output {
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.fake_autoloop.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(paths).expect("construct PATH");

        Command::new(env!("CARGO_BIN_EXE_ralph"))
            .args([
                "--color",
                "never",
                "--config",
                "ralph.yml",
                "preflight",
                "--check",
                "failure-budget",
            ])
            .current_dir(self.workspace.path())
            .env("PATH", path)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .output()
            .expect("execute failure-budget preflight")
    }

    fn task(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ralph"))
            .args(["--color", "never", "tools", "task"])
            .args(args)
            .arg("--root")
            .arg(self.workspace.path())
            .current_dir(self.workspace.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .output()
            .expect("execute ralph tools task")
    }

    fn recorded_argv(&self) -> Vec<String> {
        self.fake_autoloop
            .recorded_argv()
            .expect("fake autoloop should record argv")
    }

    fn assert_owned_engine_env(&self, output: &Output) {
        let root = self
            .workspace
            .path()
            .canonicalize()
            .expect("canonicalize workspace")
            .join(".ralph/autoloop");
        let expected = [
            format!("AUTOLOOP_STATE_DIR={}", root.display()),
            format!(
                "AUTOLOOP_JOURNAL_FILE={}",
                root.join("journal.jsonl").display()
            ),
            format!(
                "AUTOLOOP_MEMORY_FILE={}",
                root.join("memory.jsonl").display()
            ),
            format!("AUTOLOOP_TASKS_FILE={}", root.join("tasks.jsonl").display()),
        ];
        assert_eq!(
            self.fake_autoloop
                .recorded_engine_env()
                .expect("fake autoloop should record engine environment"),
            expected,
        );
        let argv = self.recorded_argv();
        for override_arg in [
            format!("core.state_dir={}", root.display()),
            format!("core.journal_file={}", root.join("journal.jsonl").display()),
            format!("core.memory_file={}", root.join("memory.jsonl").display()),
            format!("core.tasks_file={}", root.join("tasks.jsonl").display()),
        ] {
            assert!(
                argv.contains(&override_arg),
                "missing engine ownership override {override_arg:?} in {argv:?}"
            );
        }
        assert!(
            !self.workspace.path().join(".autoloop").exists(),
            "Ralph launch created a top-level .autoloop directory"
        );
        let normal_output = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !normal_output.contains(&root.display().to_string()),
            "normal output leaked the physical engine-state path:\n{normal_output}"
        );
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

fn kill_process_group(pid: u32) {
    let pid = Pid::from_raw(pid.try_into().expect("Ralph PID fits i32"));
    if let Err(error) = killpg(pid, Signal::SIGKILL) {
        assert_eq!(
            error,
            nix::errno::Errno::ESRCH,
            "failed to clean Ralph process group"
        );
    }
}

fn wait_for_barrier(child: &mut Child, ready: &Path, label: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        if let Some(status) = child.try_wait().expect("poll ralph") {
            kill_process_group(child.id());
            panic!("ralph exited before {label} barrier: {status}");
        }
        if Instant::now() >= deadline {
            let pid = child.id();
            let _ = child.kill();
            let _ = child.wait();
            kill_process_group(pid);
            panic!("timed out waiting for {label} barrier");
        }
        thread::sleep(Duration::from_millis(10));
    }
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
    assert_eq!(argv.last().map(String::as_str), Some(expected));
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
fn interrupted_loop_lock_not_stuck() {
    let harness = Harness::new(None);

    let mut first_command = harness.command(&["-p", "first blocked run"]);
    first_command
        .process_group(0)
        .env("CRASH_READY", &harness.crash_ready)
        .env("CRASH_RELEASE", &harness.crash_release)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut first = first_command.spawn().expect("spawn first ralph");
    let first_pid = first.id();
    wait_for_barrier(&mut first, &harness.crash_ready, "first run");

    let first_metadata =
        LoopLock::read_existing(harness.workspace.path()).expect("read first lock metadata");
    let first_locked = LoopLock::is_locked(harness.workspace.path()).expect("probe first lock");

    let first_process = Pid::from_raw(first_pid.try_into().expect("Ralph PID fits i32"));
    kill(first_process, Signal::SIGKILL).expect("SIGKILL first Ralph process");
    let first_status = first.wait().expect("reap SIGKILLed Ralph process");
    assert!(
        !first_status.success(),
        "SIGKILLed Ralph process unexpectedly succeeded"
    );
    // Ralph's fake-autoloop child is deliberately left no release path. Kill the
    // remaining process group after Ralph is reaped so it cannot leak from the test.
    kill_process_group(first_pid);

    let first_metadata = first_metadata.unwrap_or_else(|| {
        panic!(
            "first run should create lock metadata; fake argv: {:?}",
            harness.recorded_argv()
        )
    });
    assert_eq!(
        first_metadata.pid, first_pid,
        "first lock metadata must identify the Ralph process"
    );
    assert!(
        first_locked,
        "first Ralph process should hold the loop lock"
    );

    let mut second_command = harness.command(&["-p", "replacement run"]);
    second_command
        .process_group(0)
        .env("CRASH_READY", &harness.restart_ready)
        .env("CRASH_RELEASE", &harness.restart_release);
    let mut second = second_command.spawn().expect("spawn replacement ralph");
    wait_for_barrier(&mut second, &harness.restart_ready, "replacement run");

    let second_pid = second.id();
    let second_metadata =
        LoopLock::read_existing(harness.workspace.path()).expect("read replacement lock metadata");
    let second_locked =
        LoopLock::is_locked(harness.workspace.path()).expect("probe replacement lock");

    fs::write(&harness.restart_release, "release\n").expect("release replacement fake autoloop");
    let second_output = second
        .wait_with_output()
        .expect("wait for replacement Ralph");
    assert_success(&second_output);

    let second_metadata = second_metadata.expect("replacement run should rewrite lock metadata");
    assert_eq!(
        second_metadata.pid, second_pid,
        "replacement run must overwrite stale metadata with its Ralph PID"
    );
    assert_ne!(
        second_metadata.pid, first_metadata.pid,
        "replacement run must not retain stale PID metadata"
    );
    assert!(
        second_locked,
        "replacement Ralph process should hold the loop lock"
    );
    assert!(
        !LoopLock::is_locked(harness.workspace.path()).expect("probe released lock"),
        "loop lock remained held after replacement run completed"
    );
    let reacquired = LoopLock::try_acquire(harness.workspace.path(), "post-SIGKILL probe")
        .expect("loop lock should be reacquirable after replacement run");
    drop(reacquired);
}

#[test]
fn run_inline_prompt_reaches_autoloop_as_positional_argument() {
    let harness = Harness::new(None);

    let output = harness.run(&["-p", "text"]);

    assert_success(&output);
    assert_recorded_prompt(&harness, "text");
    harness.assert_owned_engine_env(&output);
}

#[test]
fn generated_preset_forwards_pi_backend_selection() {
    let harness = Harness::new(None);
    fs::write(
        harness.workspace.path().join("ralph.yml"),
        "core:\n  engine: autoloop\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n",
    )
    .expect("write generated-preset ralph.yml");

    let output = harness.run(&["--autonomous", "-b", "pi", "-p", "backend forwarding"]);

    assert_success(&output);
    let argv = harness.recorded_argv();
    assert!(argv.len() >= 3, "unexpected autoloop argv: {argv:?}");
    assert_eq!(argv[0], "run", "unexpected autoloop argv: {argv:?}");
    let autoloops_toml = fs::read_to_string(Path::new(&argv[1]).join("autoloops.toml"))
        .expect("read generated autoloops.toml from recorded preset argv");
    assert!(
        autoloops_toml.contains("backend.kind = \"pi\""),
        "generated preset did not select the pi backend:\n{autoloops_toml}"
    );
    assert!(
        autoloops_toml.contains("backend.command = \"pi\""),
        "generated preset did not select the pi command:\n{autoloops_toml}"
    );
    harness.assert_owned_engine_env(&output);
}

#[test]
fn explicit_preset_rejects_cli_backend_without_running_autoloop() {
    let harness = Harness::new(None);

    let output = harness.run(&["--autonomous", "-b", "claude", "-p", "conflicting backend"]);

    assert!(
        !output.status.success(),
        "ralph should reject a CLI backend combined with an explicit preset"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("explicit preset owns backend selection"),
        "conflict error did not explain backend ownership:\n{stderr}"
    );
    assert!(
        !harness.fake_autoloop.argv_out().exists(),
        "fake autoloop must not run when backend selection conflicts with an explicit preset"
    );
}

#[test]
fn fresh_run_marker_isolates_open_tasks_from_previous_run() {
    let harness = Harness::new(None);
    let marker = harness.workspace.path().join(".ralph/current-loop-id");

    let first_run = harness.run(&["-p", "first run"]);
    assert_success(&first_run);
    let first_loop_id = fs::read_to_string(&marker).expect("first run should write loop marker");
    let first_loop_id = first_loop_id.trim();
    assert!(
        !first_loop_id.is_empty(),
        "first run marker must not be blank"
    );

    let add = harness.task(&["add", "Task from first autoloop run"]);
    assert_success(&add);

    let second_run = harness.run(&["-p", "second run"]);
    assert_success(&second_run);
    let second_loop_id = fs::read_to_string(&marker).expect("second run should write loop marker");
    let second_loop_id = second_loop_id.trim();
    assert!(
        !second_loop_id.is_empty(),
        "second run marker must not be blank"
    );
    assert_ne!(
        second_loop_id, first_loop_id,
        "a fresh run must replace the previous loop marker"
    );

    let ready = harness.task(&["ready", "--format", "json"]);
    assert_success(&ready);
    let ready_tasks: Vec<Task> =
        serde_json::from_slice(&ready.stdout).expect("parse current-loop ready tasks");
    assert!(
        ready_tasks.is_empty(),
        "the second run must not see the first run's task: {ready_tasks:?}"
    );

    let all_ready = harness.task(&["ready", "--all", "--format", "json"]);
    assert_success(&all_ready);
    let all_ready_tasks: Vec<Task> =
        serde_json::from_slice(&all_ready.stdout).expect("parse all ready tasks");
    assert_eq!(all_ready_tasks.len(), 1);
    assert_eq!(all_ready_tasks[0].title, "Task from first autoloop run");
    assert_eq!(all_ready_tasks[0].loop_id.as_deref(), Some(first_loop_id));
}

#[test]
fn completion_warning_reports_open_ralph_tasks_without_gating_success() {
    let harness = Harness::new(None);

    let initial_run = harness.run(&["-p", "establish loop identity"]);
    assert_success(&initial_run);

    let add = harness.task(&["add", "Open task at autoloop completion"]);
    assert_success(&add);
    fs::write(
        harness.workspace.path().join(".ralph/agent/scratchpad.md"),
        "# Continue-state fixture\n",
    )
    .expect("write continue scratchpad fixture");

    let continued_run = harness.run(&["-p", "complete with open task", "--continue"]);
    assert_success(&continued_run);

    let process_output = format!(
        "{}{}",
        String::from_utf8_lossy(&continued_run.stdout),
        String::from_utf8_lossy(&continued_run.stderr)
    );
    assert!(
        process_output.contains("WARNING: Loop completed with 1 open Ralph task(s)"),
        "open-task observation warning was not visible:\n{process_output}"
    );
    assert!(
        process_output.contains("did not participate in loop completion"),
        "warning did not explain its non-gating semantics:\n{process_output}"
    );
    assert!(
        !process_output.contains("autoloop completed"),
        "engine-first task warning leaked:\n{process_output}"
    );
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

#[test]
fn explicit_preset_receives_cli_and_config_budget_overrides_before_prompt() {
    let harness = Harness::new_with_event_loop(
        None,
        &[
            ("max_runtime_seconds", "12"),
            ("max_cost_usd", "3.5"),
            ("max_consecutive_failures", "2"),
        ],
    );

    let output = harness.run(&["-p", "budgeted prompt", "--max-iterations", "7"]);

    assert_success(&output);
    let argv = harness.recorded_argv();
    let prompt_position = argv
        .iter()
        .position(|arg| arg == "budgeted prompt")
        .expect("prompt should be present");
    for expected in [
        "event_loop.max_iterations=7",
        "event_loop.max_runtime=12000",
        "event_loop.max_cost_usd=3.5",
    ] {
        let position = argv
            .iter()
            .position(|arg| arg == expected)
            .unwrap_or_else(|| panic!("missing {expected:?} in autoloop argv: {argv:?}"));
        assert_eq!(argv[position - 1], "--set", "unexpected argv: {argv:?}");
        assert!(
            position < prompt_position,
            "override {expected:?} must precede prompt: {argv:?}"
        );
    }
    assert_eq!(prompt_position, argv.len() - 1, "unexpected argv: {argv:?}");
    assert!(
        !argv
            .iter()
            .any(|arg| arg.contains("max_consecutive_failures")),
        "unsupported consecutive-failure override leaked into autoloop argv: {argv:?}"
    );

    let preflight = harness.preflight_failure_budget();
    assert_success(&preflight);
    let preflight_output = format!(
        "{}{}",
        String::from_utf8_lossy(&preflight.stdout),
        String::from_utf8_lossy(&preflight.stderr)
    );
    assert!(
        preflight_output.contains(
            "event_loop.max_consecutive_failures=2 is not enforced; the loop has no equivalent consecutive-failure budget"
        ),
        "prominent unsupported-budget warning was not visible:\n{preflight_output}"
    );
    assert!(
        !preflight_output.contains("0.10.x") && !preflight_output.contains("0.9.2"),
        "engine version leaked into budget warning:\n{preflight_output}"
    );
}
