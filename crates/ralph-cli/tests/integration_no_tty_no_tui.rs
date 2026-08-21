#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use ralph_core::testing::fake_autoloop::build_fake_autoloop;

const FALLBACK_NOTICE: &str = "No interactive terminal detected; using headless mode.";

const RALPH_YML: &str = r#"
core:
  engine: autoloop
cli:
  backend: claude
event_loop:
  max_iterations: 2
  completion_promise: LOOP_COMPLETE
hats:
  planner:
    name: "Planning Lead"
    description: "Plans the work"
    triggers: ["loop.start"]
    publishes: ["build.task"]
    instructions: "Plan the work."
  builder:
    name: "Build Crew"
    description: "Builds the work"
    triggers: ["build.task"]
    publishes: ["task.complete"]
    instructions: "Build the work."
features:
  auto_merge: false
"#;

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
fn no_tty_no_tui() {
    let workspace = tempfile::tempdir().expect("workspace temp dir");
    let home = tempfile::tempdir().expect("home temp dir");
    fs::write(workspace.path().join("ralph.yml"), RALPH_YML).expect("write ralph.yml");
    fs::write(
        workspace.path().join("README.md"),
        "non-TTY fallback test\n",
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

    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/autoloop/headless_voice.jsonl");
    let fake_autoloop = build_fake_autoloop(&workspace.path().join("fake-autoloop"), &fixture)
        .expect("build fixture-driven fake autoloop");
    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![fake_autoloop.bin_dir().to_path_buf()];
    paths.extend(std::env::split_paths(&inherited_path));
    let path = std::env::join_paths(paths).expect("construct PATH");

    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "--color",
            "never",
            "--config",
            "ralph.yml",
            "run",
            "--skip-preflight",
            "--max-iterations",
            "2",
            "-p",
            "exercise non-TTY fallback",
        ])
        .current_dir(workspace.path())
        .env("PATH", path)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env_remove("NO_COLOR")
        .env_remove("RALPH_CONFIG")
        .env_remove("RALPH_DIAGNOSTICS")
        .env_remove("RALPH_WORKSPACE_ROOT")
        .env_remove("RALPH_MERGE_LOOP_ID")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run ralph without a terminal");
    let output_text = combined_output(&output);

    assert!(
        output.status.success(),
        "non-TTY default run failed with {}; output:\n{output_text}",
        output.status
    );
    assert_eq!(
        output_text.matches(FALLBACK_NOTICE).count(),
        1,
        "fallback notice must appear exactly once:\n{output_text}"
    );
    assert!(
        output_text.contains("Iteration 2/2 finished | Build Crew | emitted task.complete"),
        "headless completion evidence missing:\n{output_text}"
    );
    for forbidden in [
        "Device not configured",
        "TUI render loop failed",
        "autoloop TUI run failed",
    ] {
        assert!(
            !output_text.contains(forbidden),
            "raw-mode failure leaked ({forbidden}):\n{output_text}"
        );
    }
}
