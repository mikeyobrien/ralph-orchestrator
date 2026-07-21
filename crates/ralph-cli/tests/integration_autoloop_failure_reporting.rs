#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ralph_core::testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop};
use serde_json::{Value, json};
use tempfile::TempDir;

const SECRET_PROMPT: &str = "PROMPT_SECRET_NEVER_RENDER_7f6c2a";
const SECRET_STATE_PATH: &str = "/ABSOLUTE_SECRET_STATE";
const SECRET_STDERR: &str = "RAW_CHILD_STDERR_SECRET_4de9b1";
const SECRET_STDOUT: &str = "RAW_CHILD_STDOUT_SECRET_1a2b3c";
const SECRET_PRESET: &str = "PRESET_PATH_SECRET_88aef0";
const SECRET_TERMINAL_DETAIL: &str = "TERMINAL_DETAIL_TOKEN_/private/sentinel_91ca";
const RALPH_YML: &str = r#"
core:
  engine: autoloop
cli:
  backend: claude
event_loop:
  max_iterations: 1
  completion_promise: LOOP_COMPLETE
hats:
  planner:
    name: "Planner"
    description: "Plans the test run"
    triggers: ["loop.start"]
    publishes: ["task.complete"]
    instructions: "Complete the test."
features:
  auto_merge: false
"#;

struct Harness {
    workspace: TempDir,
    home: TempDir,
    fake_autoloop: FakeAutoloop,
    path: OsString,
}

impl Harness {
    fn new(invocation: Value) -> Self {
        let workspace = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");
        fs::write(workspace.path().join("ralph.yml"), RALPH_YML).expect("write ralph.yml");
        fs::write(
            workspace.path().join("README.md"),
            "failure reporting test\n",
        )
        .expect("write README");
        run_git(workspace.path(), &["init", "--quiet"]);
        run_git(workspace.path(), &["add", "README.md", "ralph.yml"]);
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

        let fixture = workspace.path().join("failure-fixture.jsonl");
        let dependency_probe = json!({
            "steps": [
                {"stdout": ["autoloop 0.10.0"]},
                {"exit": 0}
            ]
        });
        fs::write(
            &fixture,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&dependency_probe).expect("serialize dependency probe"),
                serde_json::to_string(&invocation).expect("serialize fixture")
            ),
        )
        .expect("write fake fixture");
        let fake_autoloop = build_fake_autoloop(&workspace.path().join("fake-autoloop"), &fixture)
            .expect("build fixture-driven fake autoloop");
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![fake_autoloop.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(paths).expect("construct PATH");

        Self {
            workspace,
            home,
            fake_autoloop,
            path,
        }
    }

    fn owned_root(&self) -> PathBuf {
        self.workspace.path().join(".ralph/autoloop")
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ralph"));
        command
            .args([
                "--color",
                "never",
                "--config",
                "ralph.yml",
                "run",
                "--no-tui",
                "--skip-preflight",
                "--max-iterations",
                "1",
                "-p",
                SECRET_PROMPT,
            ])
            .current_dir(self.workspace.path())
            .env("PATH", &self.path)
            .env("ENV_OUT", self.fake_autoloop.env_out())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("NO_COLOR")
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_DIAGNOSTICS")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID")
            .env_remove("JOURNAL_OUT");
        command
    }

    fn run(&self) -> Output {
        self.command().output().expect("run ralph")
    }

    fn run_with_diagnostics(&self) -> Output {
        self.command()
            .env("RALPH_DIAGNOSTICS", "1")
            .output()
            .expect("run ralph with diagnostics")
    }

    fn assert_fail_closed(&self, output: &Output) {
        let output_text = combined_output(output);
        let legacy_root = self.workspace.path().join(".autoloop");
        assert_eq!(
            fs::symlink_metadata(&legacy_root)
                .expect_err("legacy state path must not exist as a file, directory, or symlink")
                .kind(),
            std::io::ErrorKind::NotFound,
            "unexpected filesystem result for {}",
            legacy_root.display()
        );
        assert!(
            !legacy_root.join("runs/fallback-run").exists(),
            "legacy run fallback was created at {}",
            legacy_root.join("runs/fallback-run").display()
        );

        let owned_root = self.owned_root();
        assert!(
            owned_root.is_dir(),
            "Ralph-owned engine root was not created at {}",
            owned_root.display()
        );
        assert!(
            owned_root.join("journal.jsonl").is_file(),
            "fake engine state was not confined to {}",
            owned_root.display()
        );
        assert!(
            !owned_root.join("runs").exists(),
            "an unusable or absent run ID created run-scoped state under {}",
            owned_root.display()
        );

        for forbidden in [
            SECRET_PROMPT,
            SECRET_STATE_PATH,
            self.workspace.path().to_string_lossy().as_ref(),
            self.home.path().to_string_lossy().as_ref(),
            owned_root.to_string_lossy().as_ref(),
            self.fake_autoloop.bin_dir().to_string_lossy().as_ref(),
        ] {
            assert!(
                !output_text.contains(forbidden),
                "captured output exposed forbidden marker {forbidden:?}:\n{output_text}"
            );
        }
    }
}

fn invocation(events: Vec<&str>, exit: i32, with_summary: bool) -> Value {
    invocation_with_raw_output(events, exit, with_summary, None, None)
}

fn invocation_with_raw_output(
    events: Vec<&str>,
    exit: i32,
    with_summary: bool,
    stdout: Option<&str>,
    stderr: Option<&str>,
) -> Value {
    let mut steps = vec![
        json!({"journal": ["{\"run\":\"local-fixture\",\"topic\":\"loop.start\",\"fields\":{}}"]}),
    ];
    if let Some(stdout) = stdout {
        steps.push(json!({"stdout": [stdout]}));
    }
    if let Some(stderr) = stderr {
        steps.push(json!({"stderr": [stderr]}));
    }
    if !events.is_empty() {
        steps.push(json!({"events": events}));
    }
    if with_summary {
        steps.push(json!({
            "summary": {
                "run_id": "summary-only-run",
                "iterations": 1,
                "stop_reason": "completed",
                "cost_usd": 0,
                "journal": "${AUTOLOOP_STATE_DIR}/journal.jsonl",
                "memory": "${AUTOLOOP_STATE_DIR}/memory.jsonl"
            }
        }));
    }
    steps.push(json!({"exit": exit}));
    json!({"steps": steps})
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

fn all_file_contents(root: &Path) -> String {
    let mut contents = String::new();
    let Ok(entries) = fs::read_dir(root) else {
        return contents;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            contents.push_str(&all_file_contents(&path));
        } else if let Ok(file) = fs::read_to_string(path) {
            contents.push_str(&file);
        }
    }
    contents
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn nonzero_engine_exit_keeps_state_owned_and_output_private() {
    let harness = Harness::new(invocation(Vec::new(), 23, false));
    let output = harness.run();

    let output_text = combined_output(&output);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        output_text.contains("autoloop (PATH lookup): run failed (exit code Some(23))"),
        "non-zero process failure was not reported:\n{output_text}"
    );
    harness.assert_fail_closed(&output);
}

#[test]
fn successful_process_with_malformed_events_fails_closed() {
    let harness = Harness::new(invocation(
        vec![
            r#"{"type":"loop.finish","iterations":1,"stopReason":"completed","runId":"rX","costUsd":0}"#,
            "malformed record after a valid completion",
        ],
        0,
        true,
    ));
    let output = harness.run();
    let output_text = combined_output(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output_text.contains("Loop terminated: Too many malformed JSONL events"),
        "successful process trusted a stream containing malformed JSONL:\n{output_text}"
    );
    assert!(
        output_text.contains("malformed structured event stream"),
        "public error did not retain the safe failure category:\n{output_text}"
    );
    harness.assert_fail_closed(&output);
}

#[test]
fn terminal_error_detail_never_reaches_output_or_tracing() {
    let terminal_log = format!(
        "{{\"type\":\"log\",\"level\":\"error\",\"message\":\"loop stop reason=error completed_iterations=0 detail={SECRET_TERMINAL_DETAIL}\"}}"
    );
    let harness = Harness::new(invocation(
        vec![
            r#"{"type":"iteration.start","runId":"rX","iteration":1,"maxIterations":1}"#,
            &terminal_log,
            r#"{"type":"loop.finish","iterations":0,"stopReason":"error","runId":"rX","costUsd":0}"#,
        ],
        1,
        false,
    ));
    let output = harness.run_with_diagnostics();
    let output_text = combined_output(&output);
    let diagnostics = all_file_contents(&harness.workspace.path().join(".ralph/diagnostics"));

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output_text.contains("Loop terminated: Engine error: error"),
        "safe engine error category missing:\n{output_text}"
    );
    assert!(!output_text.contains(SECRET_TERMINAL_DETAIL));
    assert!(!diagnostics.contains(SECRET_TERMINAL_DETAIL));
    harness.assert_fail_closed(&output);
}

#[test]
fn nonzero_raw_stdout_and_stderr_are_redacted_from_output_and_tracing() {
    let harness = Harness::new(invocation_with_raw_output(
        Vec::new(),
        41,
        false,
        Some(&format!("{SECRET_STDOUT} {SECRET_STATE_PATH}")),
        Some(&format!(
            "[autoloops] [error] {SECRET_STDERR} {SECRET_STATE_PATH}"
        )),
    ));
    let output = harness.run_with_diagnostics();
    let output_text = combined_output(&output);
    let diagnostics = all_file_contents(&harness.workspace.path().join(".ralph/diagnostics"));

    assert_eq!(output.status.code(), Some(1));
    assert!(output_text.contains("autoloop (PATH lookup): run failed (exit code Some(41))"));
    // Raw child stdout AND stderr are untrusted on a non-zero exit: neither the
    // generic error, the rendered output, nor the diagnostic tracing may leak them.
    for secret in [SECRET_STDOUT, SECRET_STDERR, SECRET_STATE_PATH] {
        assert!(!output_text.contains(secret), "output exposed {secret}");
        assert!(!diagnostics.contains(secret), "tracing exposed {secret}");
    }
    harness.assert_fail_closed(&output);
}

#[test]
fn malformed_events_keep_state_owned_and_output_private() {
    let harness = Harness::new(invocation(
        vec!["{\"type\":\"iteration.start\"", "definitely not JSON"],
        1,
        false,
    ));
    let output = harness.run();
    let output_text = combined_output(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output_text.contains("Loop terminated: Too many malformed JSONL events"),
        "missing malformed-event fallback headline:\n{output_text}"
    );
    harness.assert_fail_closed(&output);
}

#[test]
fn missing_preset_error_does_not_expose_its_physical_path() {
    let harness = Harness::new(invocation(Vec::new(), 0, false));
    fs::write(
        harness.workspace.path().join("ralph.yml"),
        format!(
            "core:\n  engine: autoloop\n  autoloop_preset: {SECRET_PRESET}\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n"
        ),
    )
    .expect("write missing-preset config");

    let output = harness.run();
    let output_text = combined_output(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(output_text.contains("configured Autoloop preset is unavailable or invalid"));
    assert!(!output_text.contains(SECRET_PRESET));
    assert!(!output_text.contains(&harness.workspace.path().display().to_string()));
}

#[test]
fn symlinked_journal_descendant_is_rejected_before_fake_process_mutates_external_file() {
    use std::os::unix::fs::symlink;

    let harness = Harness::new(invocation(Vec::new(), 0, false));
    let external = tempfile::NamedTempFile::new().expect("external journal target");
    fs::write(external.path(), "unchanged\n").expect("seed external target");
    fs::create_dir_all(harness.owned_root()).expect("create owned root");
    symlink(external.path(), harness.owned_root().join("journal.jsonl"))
        .expect("symlink journal outside workspace");

    let output = harness.run();
    let output_text = combined_output(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(output_text.contains("Ralph-owned engine state contains a symlink"));
    assert!(!output_text.contains(&external.path().display().to_string()));
    assert_eq!(
        fs::read_to_string(external.path()).expect("read external target"),
        "unchanged\n",
        "the fake process mutated an external journal through the symlink"
    );
}

#[test]
fn status_zero_summary_with_outside_paths_fails_closed_and_private() {
    let mut invocation = invocation(Vec::new(), 0, false);
    invocation["steps"]
        .as_array_mut()
        .expect("fixture steps")
        .insert(
            1,
            json!({
                "summary": {
                    "run_id": "outside-summary",
                    "iterations": 1,
                    "stop_reason": "completed",
                    "cost_usd": 0,
                    "journal": format!("{SECRET_STATE_PATH}/journal.jsonl"),
                    "memory": format!("{SECRET_STATE_PATH}/memory.jsonl")
                }
            }),
        );
    let harness = Harness::new(invocation);

    let output = harness.run_with_diagnostics();
    let output_text = combined_output(&output);
    let diagnostics = all_file_contents(&harness.workspace.path().join(".ralph/diagnostics"));

    assert_eq!(output.status.code(), Some(1));
    assert!(output_text.contains("expected Ralph-owned state files"));
    assert!(!output_text.contains(SECRET_STATE_PATH));
    assert!(!diagnostics.contains(SECRET_STATE_PATH));
    harness.assert_fail_closed(&output);
}

#[test]
fn status_zero_unknown_stop_reason_is_redacted_and_fails_closed() {
    let secret_reason = "TOKEN_SECRET_/physical/workspace";
    let mut invocation = invocation(Vec::new(), 0, false);
    invocation["steps"]
        .as_array_mut()
        .expect("fixture steps")
        .insert(
            1,
            json!({
                "summary": {
                    "run_id": "unknown-stop",
                    "iterations": 1,
                    "stop_reason": secret_reason,
                    "cost_usd": 0,
                    "journal": "${AUTOLOOP_STATE_DIR}/journal.jsonl",
                    "memory": "${AUTOLOOP_STATE_DIR}/memory.jsonl"
                }
            }),
        );
    let harness = Harness::new(invocation);

    let output = harness.run_with_diagnostics();
    let output_text = combined_output(&output);
    let diagnostics = all_file_contents(&harness.workspace.path().join(".ralph/diagnostics"));

    assert_eq!(output.status.code(), Some(1));
    assert!(output_text.contains("unknown engine stop reason"));
    assert!(!output_text.contains(secret_reason));
    assert!(!diagnostics.contains(secret_reason));
    harness.assert_fail_closed(&output);
}

#[test]
fn symlinked_owned_root_is_rejected_before_external_state_is_written() {
    use std::os::unix::fs::symlink;

    let harness = Harness::new(invocation(Vec::new(), 0, false));
    let external = tempfile::tempdir().expect("external target");
    symlink(external.path(), harness.workspace.path().join(".ralph"))
        .expect("symlink .ralph outside workspace");

    let output = harness.run();
    let output_text = combined_output(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(output_text.contains("Ralph-owned engine state path contains a symlink"));
    assert!(!output_text.contains(&external.path().display().to_string()));
    assert_eq!(
        fs::read_dir(external.path())
            .expect("read external target")
            .count(),
        0,
        "Ralph created state through an unsafe .ralph symlink"
    );
    assert!(
        !harness.fake_autoloop.argv_out().exists(),
        "Autoloop should not run with an unsafe state root"
    );
}

#[test]
fn absent_run_id_observation_keeps_state_owned_and_output_private() {
    let harness = Harness::new(invocation(
        vec![
            r#"{"type":"loop.start","maxIterations":1}"#,
            r#"{"type":"loop.finish","iterations":1,"stopReason":"completed"}"#,
        ],
        0,
        true,
    ));
    let output = harness.run();

    assert!(output.status.success(), "{}", combined_output(&output));
    harness.assert_fail_closed(&output);
}

#[test]
fn empty_run_id_observation_keeps_state_owned_and_output_private() {
    let harness = Harness::new(invocation(
        vec![
            r#"{"type":"loop.start","runId":"","maxIterations":1}"#,
            r#"{"type":"loop.finish","runId":"","iterations":1,"stopReason":"completed"}"#,
        ],
        0,
        true,
    ));
    let output = harness.run();

    assert!(output.status.success(), "{}", combined_output(&output));
    harness.assert_fail_closed(&output);
}

#[test]
fn missing_lifecycle_run_id_keeps_state_owned_and_output_private() {
    let harness = Harness::new(invocation(
        vec![r#"{"type":"log","level":"info","message":"local fake progress"}"#],
        0,
        true,
    ));
    let output = harness.run();

    assert!(output.status.success(), "{}", combined_output(&output));
    harness.assert_fail_closed(&output);
}
