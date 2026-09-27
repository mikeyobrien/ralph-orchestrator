//! Jev completion judge at the engine's acceptance seam (Step 10, 2c.2).

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use ralph_core::testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop};

fn git(cwd: &Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .args(args)
            .current_dir(cwd)
            .status()
            .unwrap()
            .success()
    );
}

struct Workspace {
    dir: tempfile::TempDir,
    home: tempfile::TempDir,
    fake: FakeAutoloop,
}

impl Workspace {
    fn new(ralph_yml: &str, fixture: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("ralph.yml"), ralph_yml).unwrap();
        git(dir.path(), &["init", "--quiet"]);
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/autoloop")
            .join(fixture);
        let fake = build_fake_autoloop(&dir.path().join("fake-autoloop"), &fixture).unwrap();
        Self { dir, home, fake }
    }

    fn ralph(&self, args: &[&str]) -> Output {
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.fake.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited));
        Command::new(env!("CARGO_BIN_EXE_ralph"))
            .args(args)
            .current_dir(self.dir.path())
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID")
            .env_remove("RALPH_ENGINE_DIR")
            .env_remove("TYPESAFE_API_KEY")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap()
    }

    fn run(&self) -> Output {
        self.ralph(&[
            "--color",
            "never",
            "--config",
            "ralph.yml",
            "run",
            "--no-tui",
            "--skip-preflight",
            "-p",
            "judge me",
        ])
    }

    fn path(&self, relative: &str) -> std::path::PathBuf {
        self.dir.path().join(relative)
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

const JUDGE_ON: &str = "cli:\n  backend: claude\ncore:\n  completion:\n    jev:\n      enabled: true\n      threshold: 0.85\n";

#[test]
fn an_enabled_judge_becomes_an_engine_acceptance_command() {
    let workspace = Workspace::new(JUDGE_ON, "jev_new_engine.jsonl");
    let output = workspace.run();
    assert!(output.status.success(), "{}", text(&output));
    let autoloops =
        fs::read_to_string(workspace.path(".ralph/autoloop-preset/autoloops.toml")).unwrap();
    let line = autoloops
        .lines()
        .find(|line| line.starts_with("acceptance.verify_cmds = ["))
        .unwrap_or_else(|| panic!("no acceptance command: {autoloops}"));
    assert!(line.contains(" gate jev-judge --snapshot "), "{line}");
    assert!(
        autoloops.contains("review.enabled = false\n"),
        "the metareview's EXIT would bypass the gate: {autoloops}"
    );
    let snapshot = fs::read_to_string(workspace.path(".ralph/autoloop/jev-judge.json")).unwrap();
    assert!(snapshot.contains("\"threshold\": 0.85"), "{snapshot}");
}

#[test]
fn a_disabled_judge_writes_nothing_and_needs_no_credential() {
    let workspace = Workspace::new(
        "cli:\n  backend: claude\ncore:\n  completion:\n    jev:\n      enabled: false\n",
        // No gate, so no second version probe: one probe line, then the run.
        "engine_hooks.jsonl",
    );
    let output = workspace.run();
    assert!(output.status.success(), "{}", text(&output));
    let autoloops =
        fs::read_to_string(workspace.path(".ralph/autoloop-preset/autoloops.toml")).unwrap();
    assert!(!autoloops.contains("acceptance."), "{autoloops}");
    assert!(!workspace.path(".ralph/autoloop/jev-judge.json").exists());
}

#[test]
fn an_explicit_preset_with_the_judge_refuses() {
    let workspace = Workspace::new(
        &format!("{JUDGE_ON}  autoloop_preset: preset\n"),
        "jev_new_engine.jsonl",
    );
    let output = workspace.run();
    let text = text(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("core.completion.jev is set, but core.autoloop_preset (preset)"),
        "{text}"
    );
}

#[test]
fn an_engine_without_the_acceptance_gate_refuses() {
    let workspace = Workspace::new(JUDGE_ON, "judge_old_engine.jsonl");
    let output = workspace.run();
    let text = text(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("the Jev completion judge needs autoloop >= 0.11.0, but autoloop 0.10.0"),
        "{text}"
    );
}

#[test]
fn the_judge_holds_without_a_credential_and_records_no_objective() {
    let workspace = Workspace::new("cli:\n  backend: claude\n", "jev_new_engine.jsonl");
    let state = workspace.path(".ralph/autoloop");
    fs::create_dir_all(&state).unwrap();
    fs::write(
        state.join("journal.jsonl"),
        concat!(
            r#"{"run":"r1","topic":"loop.start","fields":{"objective":"SECRET-OBJECTIVE build it"}}"#,
            "\n",
            r#"{"run":"r1","iteration":"1","topic":"iteration.finish","fields":{"output":"done"}}"#,
            "\n",
        ),
    )
    .unwrap();
    let snapshot = serde_json::json!({
        "settings": {"model": "jev-1.13.0", "threshold": 0.8, "timeout_ms": 1000},
        "workspace": workspace.dir.path(),
        "journal_file": state.join("journal.jsonl"),
        "record_file": state.join("jev-judge.jsonl"),
    });
    fs::write(state.join("jev-judge.json"), snapshot.to_string()).unwrap();

    let output = workspace.ralph(&[
        "gate",
        "jev-judge",
        "--snapshot",
        state.join("jev-judge.json").to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "jev judge held: TYPESAFE_API_KEY is not set; decided via marker fallback, not a Jev approval"
    );
    let record = fs::read_to_string(state.join("jev-judge.jsonl")).unwrap();
    assert!(record.contains("\"kind\":\"marker_fallback\""), "{record}");
    assert!(record.contains("\"approved\":false"), "{record}");
    assert!(record.contains("\"run_id\":\"r1\""), "{record}");
    assert!(!record.contains("SECRET-OBJECTIVE"), "{record}");
}
