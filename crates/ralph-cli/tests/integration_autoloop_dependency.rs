#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ralph_core::autoloop_health::{AUTOLOOP_INSTALL_HINT, MIN_AUTOLOOP_VERSION};
use tempfile::TempDir;

struct Harness {
    workspace: TempDir,
    home: TempDir,
    bin_dir: PathBuf,
    engine_dir: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("workspace temp dir");
        let home = tempfile::tempdir().expect("home temp dir");
        let bin_dir = workspace.path().join("test-bin");
        fs::create_dir(&bin_dir).expect("create controlled PATH directory");
        let engine_dir = workspace.path().join("test-engine");
        fs::create_dir(&engine_dir).expect("create controlled engine directory");
        write_executable(&bin_dir.join("test-backend"), "#!/bin/sh\nexit 0\n");
        write_config(workspace.path(), ".ralph/agent/scratchpad.md");

        Self {
            workspace,
            home,
            bin_dir,
            engine_dir,
        }
    }

    fn init_git_repo(&self) {
        let output = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(self.workspace.path())
            .output()
            .expect("initialize temporary git repository");
        assert!(
            output.status.success(),
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let git = find_on_inherited_path("git").expect("git must be available to run tests");
        symlink(git, self.bin_dir.join("git")).expect("link git into controlled PATH");
    }

    fn install_versioned_autoloop(&self, version: &str) {
        let package = self.workspace.path().join("autoloop-package");
        let real_binary = package.join("bin/autoloop");
        fs::create_dir_all(real_binary.parent().expect("binary parent"))
            .expect("create package bin directory");
        fs::write(
            package.join("package.json"),
            format!(r#"{{"name":"@mobrienv/autoloop","version":"{version}"}}"#),
        )
        .expect("write autoloop package metadata");
        write_executable(&real_binary, "#!/bin/sh\nexit 0\n");
        symlink(real_binary, self.bin_dir.join("autoloop")).expect("create autoloop PATH shim");
    }

    fn install_unversionable_autoloop(&self) {
        write_executable(&self.bin_dir.join("autoloop"), "#!/bin/sh\nexit 0\n");
    }

    fn install_vendored_autoloop(&self, version: &str) {
        write_executable(
            &self.engine_dir.join("autoloop"),
            &format!("#!/bin/sh\nprintf 'autoloop {version}\\n'\n"),
        );
    }

    fn doctor(&self) -> Output {
        self.ralph(&["--color", "never", "--config", "ralph.yml", "doctor"])
    }

    fn run(&self) -> Output {
        self.ralph(&[
            "--color",
            "never",
            "--config",
            "ralph.yml",
            "run",
            "-p",
            "x",
            "--no-tui",
        ])
    }

    fn arrange_post_gate_failure(&self) {
        fs::write(self.workspace.path().join("blocked"), "not a directory")
            .expect("write scratchpad path blocker");
        write_config(self.workspace.path(), "blocked/scratchpad.md");
    }

    fn ralph(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ralph"))
            .args(args)
            .current_dir(self.workspace.path())
            .env("PATH", &self.bin_dir)
            .env("RALPH_ENGINE_DIR", &self.engine_dir)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID")
            .output()
            .expect("execute ralph")
    }
}

fn write_config(workspace: &Path, scratchpad: &str) {
    fs::write(
        workspace.join("ralph.yml"),
        format!(
            "core:\n  scratchpad: {scratchpad}\ncli:\n  backend: custom\n  command: test-backend\n"
        ),
    )
    .expect("write test config");
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("set executable bit");
}

fn find_on_inherited_path(command: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join(command))
        .find(|candidate| candidate.is_file())
}

fn rendered(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_run_reached_past_gate(output: &Output) {
    let text = rendered(output);
    assert!(
        !output.status.success(),
        "expected arranged failure: {text}"
    );
    assert!(
        text.contains("autoloop run failed"),
        "run did not reach the arranged engine-launch failure: {text}"
    );
    assert!(
        !text.contains("autoloop was not found in Ralph's engine directory")
            && !text.contains(&format!("but Ralph requires >= {MIN_AUTOLOOP_VERSION}")),
        "run stopped at the autoloop gate: {text}"
    );
}

#[test]
fn doctor_names_vendored_then_path_resolution_and_both_missing_remedies() {
    let harness = Harness::new();
    harness.install_versioned_autoloop("0.12.3");
    harness.install_vendored_autoloop("0.10.1");

    let vendored = harness.doctor();
    let vendored_text = rendered(&vendored);
    assert!(vendored.status.success(), "doctor output: {vendored_text}");
    assert!(
        vendored_text.contains("Autoloop 0.10.1 available (Ralph-managed engine)"),
        "doctor output: {vendored_text}"
    );
    assert!(
        !vendored_text.contains(&harness.engine_dir.join("autoloop").display().to_string()),
        "doctor exposed the physical engine path: {vendored_text}"
    );
    assert!(
        !vendored_text.contains(&harness.workspace.path().display().to_string()),
        "doctor exposed the physical workspace path: {vendored_text}"
    );

    fs::remove_file(harness.engine_dir.join("autoloop")).expect("remove vendored engine");
    let path = harness.doctor();
    let path_text = rendered(&path);
    assert!(path.status.success(), "doctor output: {path_text}");
    assert!(
        path_text.contains("Autoloop 0.12.3 available (PATH lookup)"),
        "doctor output: {path_text}"
    );
    assert!(
        !path_text.contains(&harness.bin_dir.display().to_string()),
        "doctor exposed the physical PATH engine location: {path_text}"
    );

    fs::remove_file(harness.bin_dir.join("autoloop")).expect("remove PATH shim");
    let missing = harness.doctor();
    let missing_text = rendered(&missing);
    assert!(!missing.status.success(), "doctor output: {missing_text}");
    assert!(
        missing_text.contains(AUTOLOOP_INSTALL_HINT),
        "doctor output: {missing_text}"
    );
    assert!(
        missing_text.contains("ralph doctor --install-engine")
            && missing_text.contains("no Node required"),
        "doctor output: {missing_text}"
    );
}

#[test]
fn missing_autoloop_is_reported_and_run_fails_before_lock() {
    let harness = Harness::new();
    harness.init_git_repo();

    let doctor = harness.doctor();
    let doctor_text = rendered(&doctor);
    assert!(
        !doctor.status.success(),
        "doctor unexpectedly passed: {doctor_text}"
    );
    assert!(
        doctor_text.contains("FAIL autoloop"),
        "doctor output: {doctor_text}"
    );
    assert!(
        doctor_text.contains(AUTOLOOP_INSTALL_HINT),
        "doctor output: {doctor_text}"
    );

    let run = harness.run();
    let run_text = rendered(&run);
    assert!(!run.status.success(), "run unexpectedly passed: {run_text}");
    assert!(
        run_text.contains(AUTOLOOP_INSTALL_HINT),
        "run output: {run_text}"
    );
    assert!(
        !harness.workspace.path().join(".ralph/loop.lock").exists(),
        "run acquired the loop lock before checking autoloop"
    );
    assert!(
        !harness.workspace.path().join(".worktrees").exists(),
        "run created a worktree before checking autoloop"
    );
}

#[test]
fn old_autoloop_is_rejected_by_doctor_and_run() {
    let harness = Harness::new();
    harness.install_versioned_autoloop("0.9.2");

    let doctor = harness.doctor();
    let doctor_text = rendered(&doctor);
    assert!(
        !doctor.status.success(),
        "doctor unexpectedly passed: {doctor_text}"
    );
    assert!(
        doctor_text.contains("FAIL autoloop"),
        "doctor output: {doctor_text}"
    );
    assert!(
        doctor_text.contains("0.9.2"),
        "doctor output: {doctor_text}"
    );
    assert!(
        doctor_text.contains(MIN_AUTOLOOP_VERSION),
        "doctor output: {doctor_text}"
    );

    let run = harness.run();
    let run_text = rendered(&run);
    assert!(!run.status.success(), "run unexpectedly passed: {run_text}");
    assert!(run_text.contains("0.9.2"), "run output: {run_text}");
    assert!(
        run_text.contains(MIN_AUTOLOOP_VERSION),
        "run output: {run_text}"
    );
}

#[test]
fn supported_autoloop_versions_pass_doctor_and_run_gate() {
    for version in [MIN_AUTOLOOP_VERSION, "0.12.3"] {
        let harness = Harness::new();
        harness.install_versioned_autoloop(version);

        let doctor = harness.doctor();
        let doctor_text = rendered(&doctor);
        assert!(
            doctor.status.success(),
            "doctor rejected {version}: {doctor_text}"
        );
        assert!(
            doctor_text.contains("OK   autoloop"),
            "doctor output: {doctor_text}"
        );
        assert!(
            doctor_text.contains(version),
            "doctor output: {doctor_text}"
        );

        harness.arrange_post_gate_failure();
        assert_run_reached_past_gate(&harness.run());
    }
}

#[test]
fn unversionable_autoloop_warns_and_run_gate_allows_it() {
    let harness = Harness::new();
    harness.install_unversionable_autoloop();

    let doctor = harness.doctor();
    let doctor_text = rendered(&doctor);
    assert!(
        doctor.status.success(),
        "doctor rejected unknown version: {doctor_text}"
    );
    assert!(
        doctor_text.contains("WARN autoloop"),
        "doctor output: {doctor_text}"
    );
    assert!(
        doctor_text.contains("version unknown"),
        "doctor output: {doctor_text}"
    );
    assert!(
        doctor_text.contains("existence check passed"),
        "doctor output: {doctor_text}"
    );

    harness.arrange_post_gate_failure();
    let run = harness.run();
    let run_text = rendered(&run);
    assert!(
        run_text.contains("version could not be determined; existence check passed"),
        "run output: {run_text}"
    );
    assert_run_reached_past_gate(&run);
}
