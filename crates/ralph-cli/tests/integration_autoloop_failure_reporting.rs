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

    fn run(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ralph"))
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
            .env_remove("JOURNAL_OUT")
            .output()
            .expect("run ralph")
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
    let mut steps = vec![
        json!({"journal": ["{\"run\":\"local-fixture\",\"topic\":\"loop.start\",\"fields\":{}}"]}),
    ];
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
                "journal": format!("{SECRET_STATE_PATH}/journal.jsonl"),
                "memory": format!("{SECRET_STATE_PATH}/memory.jsonl")
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
        output_text.contains("autoloop exited with code Some(23)"),
        "non-zero process failure was not reported:\n{output_text}"
    );
    harness.assert_fail_closed(&output);
}

#[test]
fn explicit_engine_error_detail_wins_over_malformed_event_fallback() {
    let harness = Harness::new(invocation(
        vec![
            r#"{"type":"iteration.start","runId":"rX","iteration":1,"maxIterations":1}"#,
            r#"{"type":"log","level":"error","message":"loop stop reason=error completed_iterations=0 detail=boom: native CLI not found"}"#,
            r#"{"type":"loop.finish","iterations":0,"stopReason":"error","runId":"rX","costUsd":0}"#,
        ],
        1,
        false,
    ));
    let output = harness.run();
    let output_text = combined_output(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output_text.contains("Loop terminated: Engine error: boom: native CLI not found"),
        "engine error detail missing from termination headline:\n{output_text}"
    );
    assert!(
        !output_text.contains("malformed JSONL"),
        "malformed-event fallback overrode the explicit engine error:\n{output_text}"
    );
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
