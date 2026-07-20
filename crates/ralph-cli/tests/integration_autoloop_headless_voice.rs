#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use insta::{assert_snapshot, with_settings};
use ralph_core::testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop};
use tempfile::TempDir;

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
  preflight:
    enabled: true
    skip: [config, hooks, backend, telegram, git, paths, tools]
"#;

struct Harness {
    workspace: TempDir,
    home: TempDir,
    fake_autoloop: FakeAutoloop,
    path: OsString,
}

impl Harness {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");
        fs::write(workspace.path().join("ralph.yml"), RALPH_YML).expect("write ralph.yml");
        fs::write(workspace.path().join("README.md"), "headless voice test\n")
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
            .join("tests/fixtures/autoloop/headless_voice.jsonl");
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

    fn run(&self, verbose: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ralph"));
        if verbose {
            command.arg("-v");
        }
        command
            .args([
                "--color",
                "never",
                "--config",
                "ralph.yml",
                "run",
                "--no-tui",
                "--max-iterations",
                "2",
                "-p",
                "exercise headless voice",
            ])
            .current_dir(self.workspace.path())
            .env("PATH", &self.path)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("ARGV_OUT", self.fake_autoloop.argv_out())
            .env_remove("NO_COLOR")
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_DIAGNOSTICS")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID");
        command.output().expect("run ralph")
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "ralph failed with {}; output:\n{}",
        output.status,
        combined_output(output)
    );
}

#[test]
fn headless_run_uses_ralph_voice_and_gates_engine_noise_by_verbosity() {
    let harness = Harness::new();

    let default = harness.run(false);
    assert_success(&default);
    let default_output = combined_output(&default);
    for forbidden in [
        "autoloop engine:",
        "[autoloops]",
        "autoloop::engine",
        "ralph::autoloop_preset_gen",
    ] {
        assert!(
            !default_output.contains(forbidden),
            "raw engine/tracing marker {forbidden:?} leaked:\n{default_output}"
        );
    }
    assert!(
        !default_output
            .lines()
            .any(|line| line.trim_start().starts_with("autoloop: ")),
        "raw autoloop voice leaked:\n{default_output}"
    );
    assert!(
        default_output.contains(
            "⚠ failure-budget: event_loop.max_consecutive_failures=5 is not enforced; the loop has no equivalent consecutive-failure budget."
        ),
        "styled budget warning missing:\n{default_output}"
    );
    assert!(
        !default_output.to_ascii_lowercase().contains("unknown"),
        "unknown role leaked:\n{default_output}"
    );
    assert!(
        default_output.contains("Iteration 1/2 started | Planning Lead"),
        "planner display name missing:\n{default_output}"
    );
    assert!(
        default_output.contains(
            "Iteration 1/2 finished | Planning Lead | emitted build.task | cost $0.0100 iteration, $0.0100 total"
        ),
        "planner finish voice missing:\n{default_output}"
    );
    assert!(
        default_output.contains("Iteration 2/2 started | Build Crew"),
        "builder display name missing:\n{default_output}"
    );
    assert!(
        default_output.contains(
            "Iteration 2/2 finished | Build Crew | emitted task.complete | cost $0.0200 iteration, $0.0300 total"
        ),
        "builder finish voice missing:\n{default_output}"
    );
    assert!(
        default_output.contains("Loop terminated: Completion promise detected"),
        "ralph termination reason missing:\n{default_output}"
    );
    assert!(
        default_output.contains("Iterations:  2"),
        "termination iteration count missing:\n{default_output}"
    );
    assert!(
        default_output.contains("Est. cost:   $0.03"),
        "termination cost missing:\n{default_output}"
    );
    assert!(
        !default_output.contains("autoloops summary")
            && !default_output.contains("captured stdout"),
        "captured engine stdout leaked:\n{default_output}"
    );
    assert!(
        !default_output.as_bytes().contains(&0x1b),
        "--color never emitted ANSI escapes:\n{default_output:?}"
    );

    let piped_stdout = String::from_utf8(default.stdout.clone()).expect("UTF-8 piped stdout");
    with_settings!({
        filters => vec![
            (
                r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z",
                "[TIMESTAMP]",
            ),
            (r"primary-\d+", "primary-[TIMESTAMP]"),
            (
                r"(?x)
                  (?:/private)?/var/folders/(?:[^/\s]+/){2}T/\.tmp[A-Za-z0-9]+
                  |
                  /tmp/\.tmp[A-Za-z0-9]+",
                "[TEMP]",
            ),
            (r"\d+(?:\.\d+)?s", "[ELAPSED]"),
        ]
    }, {
        assert_snapshot!("piped_headless_output", piped_stdout);
    });

    let verbose = harness.run(true);
    assert_success(&verbose);
    let verbose_output = combined_output(&verbose);
    assert!(
        verbose_output.contains("[autoloops] [info] planner engine line")
            && verbose_output.contains("[autoloops] [info] builder engine line"),
        "verbose output omitted engine info lines:\n{verbose_output}"
    );
    assert!(
        !verbose_output.contains("autoloops summary")
            && !verbose_output.contains("captured stdout"),
        "verbose run leaked captured engine stdout:\n{verbose_output}"
    );
    assert!(
        !verbose_output.as_bytes().contains(&0x1b),
        "verbose --color never emitted ANSI escapes:\n{verbose_output:?}"
    );
}
