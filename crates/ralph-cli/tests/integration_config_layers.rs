//! A user-scope `cli.args` pairs with the user-scope `cli.backend`; it must not
//! leak onto a different backend a project selects (mem-1790103346-2b0f).

use std::fs;
use std::process::Command;

#[test]
fn project_backend_does_not_inherit_user_backend_args() {
    let home = tempfile::tempdir().expect("home");
    let workspace = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(home.path().join(".ralph")).unwrap();
    fs::write(
        home.path().join(".ralph/config.yml"),
        "cli:\n  backend: pi\n  args: [\"--provider\", \"spark\", \"--model\", \"GLM-5.3-Flash-EXL3\"]\n",
    )
    .unwrap();
    fs::write(
        workspace.path().join("ralph.yml"),
        "cli:\n  backend: claude\n",
    )
    .unwrap();
    let git = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(workspace.path())
        .status()
        .unwrap();
    assert!(git.success());

    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "--color",
            "never",
            "--config",
            "ralph.yml",
            "run",
            "--dry-run",
            "--skip-preflight",
            "--no-tui",
            "--prompt",
            "x",
        ])
        .current_dir(workspace.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env_remove("RALPH_CONFIG")
        .env_remove("RALPH_WORKSPACE_ROOT")
        .output()
        .expect("run ralph");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{text}");
    assert!(!text.contains("cannot receive CLI args"), "{text}");
}
