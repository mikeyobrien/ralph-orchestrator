use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("resolve workspace root")
}

fn git_check_ignore(repo: &Path, path: &str) -> Result<bool> {
    let output = Command::new("git")
        .arg("check-ignore")
        .arg("--quiet")
        .arg("--no-index")
        .arg(path)
        .current_dir(repo)
        .output()
        .with_context(|| format!("run git check-ignore for {path}"))?;

    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!(
            "git check-ignore failed for {path}: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

#[test]
fn ralph_gitignore_tracks_committed_artifacts_not_runtime_state() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let repo = temp_dir.path();
    let workspace_root = workspace_root()?;

    let init = Command::new("git").arg("init").current_dir(repo).output()?;
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    fs::copy(workspace_root.join(".gitignore"), repo.join(".gitignore"))?;

    fs::create_dir_all(repo.join(".ralph/specs/example"))?;
    fs::create_dir_all(repo.join(".ralph/tasks"))?;
    fs::create_dir_all(repo.join(".ralph/tasks/data-pipeline/step02"))?;
    fs::create_dir_all(repo.join(".ralph/agent"))?;
    fs::write(repo.join(".ralph/specs/example/spec.md"), "# Spec\n")?;
    fs::write(repo.join(".ralph/tasks/example.code-task.md"), "# Task\n")?;
    fs::write(
        repo.join(".ralph/tasks/data-pipeline/step02/task-01-create-model.code-task.md"),
        "# Task\n",
    )?;
    fs::write(repo.join(".ralph/tasks/debug.log"), "debug\n")?;
    fs::write(repo.join(".ralph/tasks/note.txt"), "note\n")?;
    fs::write(
        repo.join(".ralph/tasks/data-pipeline/step02/tmp.json"),
        "{}\n",
    )?;
    fs::write(repo.join(".ralph/agent/decisions.md"), "# Decisions\n")?;
    fs::write(repo.join(".ralph/agent/memories.md"), "# Memories\n")?;
    fs::write(repo.join(".ralph/agent/tasks.jsonl"), "{}\n")?;
    fs::write(repo.join(".ralph/agent/scratchpad.md"), "draft\n")?;
    fs::write(repo.join(".ralph/loops.json"), "{}\n")?;

    for tracked in [
        ".ralph/specs/example/spec.md",
        ".ralph/tasks/example.code-task.md",
        ".ralph/tasks/data-pipeline/step02/task-01-create-model.code-task.md",
        ".ralph/agent/decisions.md",
        ".ralph/agent/memories.md",
    ] {
        assert!(
            !git_check_ignore(repo, tracked)?,
            "{tracked} should be visible to git"
        );
    }

    for ignored in [
        ".ralph/tasks/debug.log",
        ".ralph/tasks/note.txt",
        ".ralph/tasks/data-pipeline/step02/tmp.json",
        ".ralph/agent/tasks.jsonl",
        ".ralph/agent/scratchpad.md",
        ".ralph/loops.json",
    ] {
        assert!(
            git_check_ignore(repo, ignored)?,
            "{ignored} should stay ignored"
        );
    }

    Ok(())
}
