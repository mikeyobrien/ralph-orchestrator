#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use ralph_core::testing::fake_autoloop::build_fake_autoloop;
use tempfile::TempDir;

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
    path: OsString,
}

impl Harness {
    fn new(fixture_name: &str) -> Self {
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

        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/autoloop")
            .join(fixture_name);
        let fake_autoloop = build_fake_autoloop(&workspace.path().join("fake-autoloop"), &fixture)
            .expect("build fixture-driven fake autoloop");
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![fake_autoloop.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited_path));
        let path = std::env::join_paths(paths).expect("construct PATH");

        Self {
            workspace,
            home,
            path,
        }
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
                "exercise failure reporting",
            ])
            .current_dir(self.workspace.path())
            .env("PATH", &self.path)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("NO_COLOR")
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_DIAGNOSTICS")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID")
            .output()
            .expect("run ralph")
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

fn combined_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

// Headless and TUI both consume the RunStats produced by autoloop_engine::run;
// these public CLI tests lock the shared termination-mapping seam.
#[test]
fn explicit_engine_error_detail_wins_over_malformed_event_fallback() {
    let output = Harness::new("failure_engine_error.jsonl").run();
    let output_text = combined_output(&output);

    assert_eq!(
        output.status.code(),
        Some(1),
        "unexpected exit status; output:\n{output_text}"
    );
    assert!(
        output_text.contains("Loop terminated: Engine error: boom: native CLI not found"),
        "engine error detail missing from termination headline:\n{output_text}"
    );
    assert!(
        !output_text.contains("malformed JSONL"),
        "malformed-event fallback overrode the explicit engine error:\n{output_text}"
    );
}

#[test]
fn malformed_event_fallback_remains_when_engine_provides_no_terminal_reason() {
    let output = Harness::new("failure_malformed_only.jsonl").run();
    let output_text = combined_output(&output);

    assert_eq!(
        output.status.code(),
        Some(1),
        "unexpected exit status; output:\n{output_text}"
    );
    assert!(
        output_text.contains("Loop terminated: Too many malformed JSONL events"),
        "missing malformed-event fallback headline:\n{output_text}"
    );
}
