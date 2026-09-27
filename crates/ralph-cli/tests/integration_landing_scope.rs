//! Landing's auto-commit covers only what the loop created.
//!
//! Regression for `landing-untracked-sweep-yxv`: an untracked operator file
//! that already sat in the workspace was swept into the loop's
//! `chore: auto-commit before merge` commit and later collided on merge.

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
name = "landing-scope-test"
completion = "task.complete"

[[role]]
id = "worker"
emits = ["task.complete"]
prompt_file = "roles/worker.md"

[handoff]
"loop.start" = ["worker"]
"#;

fn git(cwd: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn landing_commit_leaves_untracked_files_that_predate_the_run() {
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
    fs::write(root.join("README.md"), "landing scope test\n").expect("write README");
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "Ralph Test"]);
    git(root, &["config", "user.email", "ralph@example.invalid"]);
    git(root, &["add", "README.md", "ralph.yml", "preset"]);
    git(root, &["commit", "--quiet", "-m", "initial"]);

    // An operator-local file that is untracked before the run starts.
    fs::write(root.join("ralph.operator.yml"), "cli:\n  backend: local\n")
        .expect("write operator file");

    // Invocation 1 answers the `--version` probe; invocation 2 is the run,
    // which creates `feature.txt` as the loop's own work.
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/autoloop/landing_scope.jsonl");
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
            "create a feature",
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
    assert!(
        output.status.success(),
        "ralph run failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let subject = git(root, &["log", "-1", "--pretty=%s"]);
    assert!(
        subject.contains("auto-commit before merge"),
        "landing should have auto-committed the loop's work, HEAD is: {subject}"
    );
    let committed = git(root, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(
        committed.lines().any(|p| p == "feature.txt"),
        "the file the loop created must be committed:\n{committed}"
    );
    assert!(
        !committed.lines().any(|p| p == "ralph.operator.yml"),
        "an untracked file that predates the run was swept into the landing commit:\n{committed}"
    );
    let status = git(root, &["status", "--porcelain", "--", "ralph.operator.yml"]);
    assert_eq!(status.trim(), "?? ralph.operator.yml");
}
