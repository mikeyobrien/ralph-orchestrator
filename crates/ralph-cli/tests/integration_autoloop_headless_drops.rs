//! Headless output names skipped engine events instead of dropping them
//! silently (Phase 2b: surface dropped progress).
//!
//! Completion still fails closed on a malformed stream (`c1fb58b`); the live
//! warning is what tells the operator why before that verdict prints.

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use ralph_core::testing::fake_autoloop::build_fake_autoloop;

const AUTOLOOPS_TOML: &str = r#"
[event_loop]
max_iterations = 1
completion_event = "task.complete"
"#;

const TOPOLOGY_TOML: &str = r#"
name = "headless-drops-test"
completion = "task.complete"

[[role]]
id = "worker"
emits = ["task.complete"]
prompt_file = "roles/worker.md"

[handoff]
"loop.start" = ["worker"]
"#;

fn git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn headless_run_reports_an_unreadable_engine_event_before_failing_closed() {
    let workspace = tempfile::tempdir().expect("workspace temp dir");
    let home = tempfile::tempdir().expect("home temp dir");
    let root = workspace.path();
    let preset = root.join("preset");
    fs::create_dir_all(preset.join("roles")).expect("create preset roles dir");
    fs::write(preset.join("autoloops.toml"), AUTOLOOPS_TOML).expect("write autoloops.toml");
    fs::write(preset.join("topology.toml"), TOPOLOGY_TOML).expect("write topology.toml");
    fs::write(preset.join("roles/worker.md"), "Complete the test.").expect("write role prompt");
    fs::write(
        root.join("ralph.yml"),
        "core:\n  engine: autoloop\n  autoloop_preset: preset\ncli:\n  backend: claude\nfeatures:\n  auto_merge: false\n",
    )
    .expect("write ralph.yml");
    fs::write(root.join("README.md"), "headless drops test\n").expect("write README");
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "Ralph Test"]);
    git(root, &["config", "user.email", "ralph@example.invalid"]);
    git(root, &["add", "README.md", "ralph.yml", "preset"]);
    git(root, &["commit", "--quiet", "-m", "initial"]);

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/autoloop/headless_dropped.jsonl");
    let fake_autoloop = build_fake_autoloop(&root.join("fake-autoloop"), &fixture)
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
            "--no-tui",
            "--skip-preflight",
            "-p",
            "drop an event",
        ])
        .current_dir(root)
        .env("PATH", path)
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env_remove("RALPH_CONFIG")
        .env_remove("RALPH_WORKSPACE_ROOT")
        .env_remove("RALPH_MERGE_LOOP_ID")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run ralph");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a malformed stream must still fail closed:\n{stdout}"
    );

    let lines: Vec<&str> = stdout.lines().collect();
    let warning = lines
        .iter()
        .position(|line| {
            *line
                == "⚠ 1 unreadable engine event skipped (8 B): progress above may be incomplete, journal intact"
        })
        .unwrap_or_else(|| panic!("the skipped event was not reported:\n{stdout}"));
    let finished = lines
        .iter()
        .position(|line| line.starts_with("Iteration 1/1 finished"))
        .unwrap_or_else(|| panic!("the readable events still render:\n{stdout}"));
    assert!(
        warning > finished,
        "the drop is reported after the poll's readable events:\n{stdout}"
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("unreadable"))
            .count(),
        1,
        "one drop is reported once:\n{stdout}"
    );
    let verdict = lines
        .iter()
        .position(|line| line.contains("Loop terminated: Too many malformed JSONL events"))
        .unwrap_or_else(|| panic!("fail-closed verdict missing:\n{stdout}"));
    assert!(
        warning < verdict,
        "the live warning precedes the verdict:\n{stdout}"
    );
}
