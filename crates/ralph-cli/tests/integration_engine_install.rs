#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ralph_core::autoloop_health::MIN_AUTOLOOP_VERSION;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

struct Fixture {
    root: TempDir,
    workspace: PathBuf,
    home: PathBuf,
    release_dir: PathBuf,
    engine_dir: PathBuf,
    bin_dir: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("fixture temp dir");
        let workspace = root.path().join("workspace");
        let home = root.path().join("home");
        let release_dir = root.path().join("release");
        let engine_dir = root.path().join("engine");
        let bin_dir = root.path().join("bin");
        let marker = root.path().join("version-checked");
        for directory in [&workspace, &home, &release_dir, &bin_dir] {
            fs::create_dir(directory).expect("create fixture directory");
        }

        write_executable(&bin_dir.join("test-backend"), "#!/bin/sh\nexit 0\n");
        let curl = find_on_inherited_path("curl").expect("curl must be available to run tests");
        symlink(curl, bin_dir.join("curl")).expect("link curl into controlled PATH");
        write_config(&workspace, ".ralph/agent/scratchpad.md");

        Self {
            root,
            workspace,
            home,
            release_dir,
            engine_dir,
            bin_dir,
            marker,
        }
    }

    fn write_valid_release(&self) {
        let artifact = artifact_name();
        let body = format!(
            "#!/bin/sh\n[ \"$1\" = \"--version\" ] || exit 64\nprintf 'autoloop 0.10.1\\n'\n: > {}\n",
            shell_quote(&self.marker)
        );
        fs::write(self.release_dir.join(artifact), body.as_bytes())
            .expect("write fixture artifact");
        let digest = format!("{:x}", Sha256::digest(body.as_bytes()));
        fs::write(
            self.release_dir.join("SHA256SUMS"),
            format!("{digest}  {artifact}\n"),
        )
        .expect("write fixture checksums");
    }

    fn corrupt_artifact(&self) {
        fs::write(
            self.release_dir.join(artifact_name()),
            b"corrupted artifact",
        )
        .expect("corrupt fixture artifact");
    }

    fn install(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ralph"))
            .args(["--color", "never", "doctor", "--install-engine"])
            .env("RALPH_ENGINE_DIR", &self.engine_dir)
            .env(
                "RALPH_ENGINE_RELEASE_BASE_URL",
                format!("file://{}", self.release_dir.display()),
            )
            .output()
            .expect("run engine installer")
    }

    fn install_path_autoloop(&self, version: &str) {
        write_executable(
            &self.bin_dir.join("autoloop"),
            &format!(
                "#!/bin/sh\n[ \"$1\" = \"--version\" ] || exit 64\nprintf 'autoloop {version}\\n'\n"
            ),
        );
    }

    fn arrange_post_gate_failure(&self) {
        fs::write(self.workspace.join("blocked"), "not a directory")
            .expect("write scratchpad path blocker");
        write_config(&self.workspace, "blocked/scratchpad.md");
    }

    fn run(&self, auto_install: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ralph"));
        command
            .args([
                "--color",
                "never",
                "--config",
                "ralph.yml",
                "run",
                "-p",
                "x",
                "--no-tui",
            ])
            .current_dir(&self.workspace)
            .env("PATH", &self.bin_dir)
            .env("RALPH_ENGINE_DIR", &self.engine_dir)
            .env(
                "RALPH_ENGINE_RELEASE_BASE_URL",
                format!("file://{}", self.release_dir.display()),
            )
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env_remove("RALPH_AUTO_INSTALL_ENGINE")
            .env_remove("RALPH_CONFIG")
            .env_remove("RALPH_WORKSPACE_ROOT")
            .env_remove("RALPH_MERGE_LOOP_ID");
        if auto_install {
            command.env("RALPH_AUTO_INSTALL_ENGINE", "1");
        }
        command.output().expect("run ralph")
    }

    fn installed_engine(&self) -> PathBuf {
        self.engine_dir.join("autoloop")
    }
}

fn artifact_name() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "autoloop-darwin-arm64",
        ("macos", "x86_64") => "autoloop-darwin-x64",
        ("linux", "x86_64") => "autoloop-linux-x64",
        ("linux", "aarch64") => "autoloop-linux-arm64",
        platform => panic!("installer integration test unsupported on {platform:?}"),
    }
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
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
        !text.contains("autoloop was not found on PATH")
            && !text.contains(&format!("but Ralph requires >= {MIN_AUTOLOOP_VERSION}")),
        "run stopped at the autoloop gate: {text}"
    );
}

#[test]
fn install_engine_downloads_verifies_installs_and_checks_version() {
    let fixture = Fixture::new();
    fixture.write_valid_release();

    let output = fixture.install();
    let text = rendered(&output);
    assert!(output.status.success(), "installer output: {text}");

    let installed = fixture.installed_engine();
    assert!(installed.is_file(), "installer output: {text}");
    assert_ne!(
        fs::metadata(&installed).unwrap().permissions().mode() & 0o111,
        0,
        "installed engine is not executable"
    );
    assert!(fixture.marker.is_file(), "--version was not run: {text}");
    assert!(text.contains("0.10.1"), "installer output: {text}");
    assert!(
        text.contains(&installed.display().to_string()),
        "installer output: {text}"
    );

    // Keep the temporary fixture alive until every assertion has completed.
    assert!(fixture.root.path().exists());
}

#[test]
fn install_engine_rejects_checksum_mismatch_without_installing() {
    let fixture = Fixture::new();
    fixture.write_valid_release();
    fixture.corrupt_artifact();

    let output = fixture.install();
    let text = rendered(&output);
    assert!(
        !output.status.success(),
        "installer unexpectedly passed: {text}"
    );
    assert!(
        text.to_lowercase().contains("checksum"),
        "installer output: {text}"
    );
    assert!(
        !fixture.installed_engine().exists(),
        "corrupted engine was installed: {text}"
    );
}

#[test]
fn install_engine_reports_missing_artifact_without_installing() {
    let fixture = Fixture::new();

    let output = fixture.install();
    let text = rendered(&output);
    assert!(
        !output.status.success(),
        "installer unexpectedly passed: {text}"
    );
    assert!(text.contains("curl failed"), "installer output: {text}");
    assert!(
        text.contains("missing") || text.contains("404"),
        "installer output is not actionable: {text}"
    );
    assert!(
        !fixture.installed_engine().exists(),
        "missing artifact produced an install: {text}"
    );
}

#[test]
fn run_auto_installs_missing_engine_and_proceeds_past_gate() {
    let fixture = Fixture::new();
    fixture.write_valid_release();
    fixture.arrange_post_gate_failure();

    let output = fixture.run(true);
    let text = rendered(&output);
    assert_run_reached_past_gate(&output);

    let installed = fixture.installed_engine();
    assert!(installed.is_file(), "run did not provision engine: {text}");
    assert_ne!(
        fs::metadata(&installed).unwrap().permissions().mode() & 0o111,
        0,
        "provisioned engine is not executable"
    );
    assert!(fixture.marker.is_file(), "--version was not run: {text}");
    assert!(
        text.contains("Installed autoloop 0.10.1") && text.contains("checksum verified"),
        "run did not confirm installation: {text}"
    );
}

#[test]
fn noninteractive_run_without_opt_in_refuses_engine_install() {
    let fixture = Fixture::new();
    fixture.write_valid_release();

    let output = fixture.run(false);
    let text = rendered(&output);
    assert!(!output.status.success(), "run unexpectedly passed: {text}");
    assert!(
        text.contains("RALPH_AUTO_INSTALL_ENGINE")
            && text.contains("ralph doctor --install-engine"),
        "run refusal was not actionable: {text}"
    );
    assert!(
        !fixture.installed_engine().exists(),
        "run installed engine without opt-in: {text}"
    );
    assert!(
        !fixture.workspace.join(".ralph").exists(),
        "run progressed beyond the dependency gate: {text}"
    );
}

#[test]
fn run_rejects_corrupted_auto_install_before_progressing_past_gate() {
    let fixture = Fixture::new();
    fixture.write_valid_release();
    fixture.corrupt_artifact();

    let output = fixture.run(true);
    let text = rendered(&output);
    assert!(!output.status.success(), "run unexpectedly passed: {text}");
    assert!(
        text.to_lowercase().contains("checksum"),
        "run did not report checksum failure: {text}"
    );
    assert!(
        !fixture.installed_engine().exists(),
        "run installed corrupted engine: {text}"
    );
    assert!(
        !fixture.marker.exists(),
        "run executed corrupted engine: {text}"
    );
    assert!(
        !fixture.workspace.join(".ralph").exists(),
        "run progressed beyond the dependency gate: {text}"
    );
}

#[test]
fn run_auto_installs_vendored_engine_that_outranks_old_path_engine() {
    let fixture = Fixture::new();
    fixture.write_valid_release();
    fixture.install_path_autoloop("0.9.0");
    fixture.arrange_post_gate_failure();

    let output = fixture.run(true);
    let text = rendered(&output);
    assert_run_reached_past_gate(&output);
    assert!(
        fixture.installed_engine().is_file(),
        "run did not install vendored engine: {text}"
    );
    assert!(
        fixture.marker.is_file(),
        "vendored engine was not checked: {text}"
    );
    assert!(
        text.contains("Installed autoloop 0.10.1") && text.contains("checksum verified"),
        "run did not confirm vendored installation: {text}"
    );
}
