//! Lifecycle hooks under the autoloop engine (Step 8b, hook parity).
//!
//! `post.loop.*` hooks become the engine's finish notification in the
//! generated preset; every other event refuses the run and names itself.

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use ralph_core::testing::fake_autoloop::{FakeAutoloop, build_fake_autoloop};

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

struct Workspace {
    dir: tempfile::TempDir,
    home: tempfile::TempDir,
    fake: FakeAutoloop,
}

impl Workspace {
    fn new(hooks_yaml: &str) -> Self {
        let dir = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");
        let root = dir.path();
        fs::write(
            root.join("ralph.yml"),
            format!("cli:\n  backend: claude\n{hooks_yaml}"),
        )
        .expect("write ralph.yml");
        fs::write(root.join("README.md"), "hooks test\n").expect("write README");
        git(root, &["init", "--quiet"]);
        git(root, &["config", "user.name", "Ralph Test"]);
        git(root, &["config", "user.email", "ralph@example.invalid"]);
        git(root, &["add", "README.md", "ralph.yml"]);
        git(root, &["commit", "--quiet", "-m", "initial"]);
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/autoloop/engine_hooks.jsonl");
        let fake = build_fake_autoloop(&root.join("fake-autoloop"), &fixture)
            .expect("build fixture-driven fake autoloop");
        Self { dir, home, fake }
    }

    fn run(&self) -> Output {
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.fake.bin_dir().to_path_buf()];
        paths.extend(std::env::split_paths(&inherited_path));
        Command::new(env!("CARGO_BIN_EXE_ralph"))
            .args([
                "--color",
                "never",
                "--config",
                "ralph.yml",
                "run",
                "--no-tui",
                "--skip-preflight",
                "-p",
                "hooks",
            ])
            .current_dir(self.dir.path())
            .env("PATH", std::env::join_paths(paths).expect("construct PATH"))
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run ralph")
    }

    fn generated_autoloops(&self) -> String {
        fs::read_to_string(
            self.dir
                .path()
                .join(".ralph/autoloop-preset/autoloops.toml"),
        )
        .expect("generated preset")
    }
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

const HOOK: &str = "      - name: notify\n        command: [\"true\"]\n        on_error: warn\n";

#[test]
fn post_loop_hooks_become_the_engines_finish_notification() {
    let workspace = Workspace::new(&format!(
        "hooks:\n  enabled: true\n  events:\n    post.loop.complete:\n{HOOK}    post.loop.error:\n{HOOK}"
    ));
    let output = workspace.run();
    assert!(output.status.success(), "{}", combined(&output));

    let autoloops = workspace.generated_autoloops();
    assert!(
        autoloops.contains("hooks notify --snapshot"),
        "notify.command must dispatch through ralph: {autoloops}"
    );
    assert!(
        autoloops.contains("notify.on = \"completed,failed,stopped\""),
        "{autoloops}"
    );
    let snapshot = workspace
        .dir
        .path()
        .join(".ralph/autoloop/notify-hooks.json");
    let snapshot = fs::read_to_string(snapshot).expect("hook snapshot written");
    assert!(snapshot.contains("post.loop.complete"), "{snapshot}");
}

#[test]
fn a_complete_only_hook_notifies_on_completion_only() {
    let workspace = Workspace::new(&format!(
        "hooks:\n  enabled: true\n  events:\n    post.loop.complete:\n{HOOK}"
    ));
    let output = workspace.run();
    assert!(output.status.success(), "{}", combined(&output));
    assert!(
        workspace
            .generated_autoloops()
            .contains("notify.on = \"completed\"\n")
    );
}

#[test]
fn no_hooks_means_no_notify_block() {
    let workspace = Workspace::new("");
    let output = workspace.run();
    assert!(output.status.success(), "{}", combined(&output));
    assert!(!workspace.generated_autoloops().contains("notify."));
}

#[test]
fn an_unsupported_hook_event_refuses_to_start_and_names_it() {
    let workspace = Workspace::new(&format!(
        "hooks:\n  enabled: true\n  events:\n    pre.loop.start:\n{HOOK}"
    ));
    let output = workspace.run();
    let text = combined(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("hook event `pre.loop.start` has no equivalent under the v3 autoloop engine"),
        "{text}"
    );
    assert!(
        !workspace.dir.path().join(".ralph/autoloop-preset").exists(),
        "the run must refuse before generating a preset"
    );
}
