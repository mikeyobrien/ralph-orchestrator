#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ralph_core::testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop};
use tempfile::TempDir;

const RALPH_YML: &str = r#"
core:
  engine: autoloop
  scratchpad: missing/scratchpad.md
cli:
  backend: claude
event_loop:
  max_iterations: 2
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

const LOOP_ID: &str = "existing-coordination-loop";
const RUN_ID: &str = "run-344-native-resume";

struct Harness {
    workspace: TempDir,
    home: TempDir,
    fake_autoloop: FakeAutoloop,
    path: OsString,
    scratchpad: PathBuf,
}

impl Harness {
    fn new() -> Self {
        Self::with_fixture("headless_voice.jsonl")
    }

    fn with_fixture(fixture_name: &str) -> Self {
        let workspace = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");
        fs::write(workspace.path().join("ralph.yml"), RALPH_YML).expect("write ralph.yml");
        fs::write(workspace.path().join("README.md"), "continue test\n").expect("write README");
        fs::create_dir(workspace.path().join(".ralph")).expect("create coordination directory");
        fs::write(
            workspace.path().join(".ralph/current-loop-id"),
            format!("{LOOP_ID}\n"),
        )
        .expect("write current loop marker");
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
        let scratchpad = workspace.path().join("missing/scratchpad.md");

        Self {
            workspace,
            home,
            fake_autoloop,
            path,
            scratchpad,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ralph"));
        command
            .current_dir(self.workspace.path())
            .env("PATH", &self.path)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("ARGV_OUT", self.fake_autoloop.argv_out())
            .env("ENV_OUT", self.fake_autoloop.env_out())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID");
        command
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

fn rendered(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn run_continue_without_scratchpad_completes_and_reuses_coordination_loop() {
    let harness = Harness::new();
    assert!(
        !harness.scratchpad.exists(),
        "test precondition: configured scratchpad must not exist before run"
    );

    let output = harness
        .command()
        .args([
            "--color",
            "never",
            "--config",
            "ralph.yml",
            "run",
            "--continue",
            "--no-tui",
            "--skip-preflight",
            "--max-iterations",
            "2",
            "-p",
            "continue coordination",
        ])
        .output()
        .expect("run ralph --continue");
    let text = rendered(&output);

    assert!(output.status.success(), "continue failed: {text}");
    assert!(
        text.contains("Iteration 1/2 started | Planning Lead")
            && text.contains("Loop terminated: Completion promise detected"),
        "fake autoloop did not complete through headless path: {text}"
    );
    assert!(
        !text.contains("scratchpad not found"),
        "obsolete scratchpad gate was reached: {text}"
    );
    assert_eq!(
        fs::read_to_string(harness.workspace.path().join(".ralph/current-loop-id"))
            .expect("read current loop marker")
            .trim(),
        LOOP_ID,
        "--continue must preserve coordination loop identity"
    );
}

/// A recorded engine run (`.ralph/current-run-id`) makes `ralph resume`
/// continue that native autoloop run instead of refusing.
#[test]
fn resume_continues_recorded_native_run_without_redoing_work() {
    let harness = Harness::with_fixture("resume_native.jsonl");
    fs::write(
        harness.workspace.path().join(".ralph/current-run-id"),
        format!("{RUN_ID}\n"),
    )
    .expect("write current run marker");

    let output = harness
        .command()
        .args(["--color", "never", "resume"])
        .output()
        .expect("run ralph resume");
    let text = rendered(&output);

    assert!(output.status.success(), "resume failed: {text}");
    assert!(
        text.contains("fake autoloop: resume invocation reached"),
        "native engine resume did not run to completion: {text}"
    );

    // The resume subprocess must receive exactly `resume <run-id>`.
    let argv = fs::read_to_string(harness.fake_autoloop.argv_out()).expect("read argv out");
    let args: Vec<&str> = argv.lines().collect();
    assert_eq!(
        args,
        ["resume", RUN_ID],
        "unexpected autoloop argv: {args:?}"
    );

    // The Ralph-owned engine state root must be exported so autoloop resumes
    // the exact run, not a re-discovered one.
    let env = fs::read_to_string(harness.fake_autoloop.env_out()).expect("read env out");
    let state_dir = harness
        .workspace
        .path()
        .join(".ralph/autoloop")
        .canonicalize()
        .expect("canonicalize state dir");
    assert!(
        env.contains(&format!("AUTOLOOP_STATE_DIR={}", state_dir.display())),
        "resume did not export the owned engine state root: {env}"
    );
    assert!(
        env.contains(&format!(
            "AUTOLOOP_JOURNAL_FILE={}",
            state_dir.join("journal.jsonl").display()
        )),
        "resume did not export the owned journal file: {env}"
    );
}

/// Without a recorded run, `ralph resume` fails cleanly and never launches
/// the engine process.
#[test]
fn resume_without_recorded_run_fails_without_launching_engine() {
    let harness = Harness::with_fixture("resume_native.jsonl");
    assert!(!harness.fake_autoloop.argv_out().exists());

    let output = harness
        .command()
        .args(["--color", "never", "resume"])
        .output()
        .expect("run ralph resume");
    let text = rendered(&output);

    assert!(
        !output.status.success(),
        "resume unexpectedly succeeded: {text}"
    );
    assert!(
        text.contains("no engine run to resume"),
        "expected a clean no-run error: {text}"
    );
    assert!(
        !text.contains("scratchpad not found"),
        "resume reported obsolete scratchpad error: {text}"
    );
    assert!(
        !harness.fake_autoloop.argv_out().exists(),
        "resume must not launch autoloop when there is no recorded run"
    );
}
