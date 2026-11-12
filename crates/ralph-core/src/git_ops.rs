//! Git operations for Ralph orchestration.
//!
//! Provides utilities for git operations like auto-committing uncommitted changes
//! before merge queue operations, and git state cleanup during landing.

use std::io;
use std::path::Path;
use std::process::Command;

/// Result of an auto-commit operation.
#[derive(Debug, Clone)]
pub struct AutoCommitResult {
    /// Whether a commit was made.
    pub committed: bool,

    /// The commit SHA if a commit was made.
    pub commit_sha: Option<String>,

    /// Number of files that were staged.
    pub files_staged: usize,
}

impl AutoCommitResult {
    /// Create a result indicating no commit was made.
    pub fn no_commit() -> Self {
        Self {
            committed: false,
            commit_sha: None,
            files_staged: 0,
        }
    }
}

/// Errors that can occur during git operations.
#[derive(Debug, thiserror::Error)]
pub enum GitOpsError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Git command failed.
    #[error("Git command failed: {0}")]
    Git(String),

    /// Git config is missing (user.name or user.email not set).
    #[error("Git config missing: {0}")]
    ConfigMissing(String),
}

/// Check if the working directory has uncommitted changes.
///
/// Returns true if there are:
/// - Untracked files (not in .gitignore)
/// - Staged changes
/// - Unstaged modifications
///
/// # Arguments
///
/// * `path` - Path to the git repository (or worktree)
pub fn has_uncommitted_changes(path: impl AsRef<Path>) -> Result<bool, GitOpsError> {
    let path = path.as_ref();

    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(!stdout.trim().is_empty())
}

/// Whether an untracked path (relative to the workspace) is a Ralph run artifact
/// that the auto-commit must leave out.
///
/// Artifacts are:
/// - anything under a nested `.ralph/` directory (e.g. `crates/foo/.ralph/diagnostics/`),
///   which only appears when something ran Ralph code with a subdirectory as cwd;
/// - anything under the workspace's own `.ralph/` except the durable text artifacts
///   (specs, `*.code-task.md` files, `agent/memories.md`, `agent/decisions.md`), and
///   never a `logs/` directory inside them;
/// - anything under the workspace-root `logs/`, which Ralph's prompts hand to agents
///   as scratch space for temporary artifacts.
///
/// Ignore rules alone can't express this: the auto-commit runs in user repos whose
/// `.gitignore` knows nothing about Ralph's layout.
fn is_run_artifact(path: &str) -> bool {
    let components: Vec<&str> = path.split('/').collect();
    match components.as_slice() {
        ["logs", ..] => true,
        [".ralph", rest @ ..] => !is_durable_ralph_artifact(rest),
        [_, rest @ ..] => rest.contains(&".ralph"),
        [] => false,
    }
}

/// Durable files under the workspace `.ralph/` that a loop commits on purpose.
/// `rest` is the path below `.ralph/`.
fn is_durable_ralph_artifact(rest: &[&str]) -> bool {
    match rest {
        ["agent", "memories.md" | "decisions.md"] => true,
        ["specs", inner @ .., _file] => !inner.contains(&"logs"),
        ["tasks", .., file] => file.ends_with(".code-task.md"),
        _ => false,
    }
}

/// Stage the loop's work: every change to tracked files, plus untracked files
/// that are not run artifacts (see [`is_run_artifact`]).
fn stage_loop_work(path: &Path) -> Result<(), GitOpsError> {
    let run_git = |args: &[&str], stdin: Option<&[u8]>| -> Result<Vec<u8>, GitOpsError> {
        let mut child = Command::new("git")
            .args(args)
            .current_dir(path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        if let Some(input) = stdin {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin is piped")
                .write_all(input)?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitOpsError::Git(format!(
                "Failed to stage changes: {}",
                stderr
            )));
        }
        Ok(output.stdout)
    };

    // Tracked files are always the loop's business: modifications and deletions.
    run_git(&["add", "--update"], None)?;

    let untracked = run_git(&["ls-files", "--others", "--exclude-standard", "-z"], None)?;
    let mut pathspecs = Vec::new();
    for file in untracked.split(|b| *b == 0).filter(|f| !f.is_empty()) {
        if !is_run_artifact(&String::from_utf8_lossy(file)) {
            pathspecs.extend_from_slice(file);
            pathspecs.push(0);
        }
    }
    if !pathspecs.is_empty() {
        run_git(
            &[
                "--literal-pathspecs",
                "add",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            Some(&pathspecs),
        )?;
    }
    Ok(())
}

/// Auto-commit the loop's uncommitted work in the repository.
///
/// This stages changes to tracked files and untracked files that are not Ralph
/// run artifacts (diagnostics, logs, scratchpads, runtime state — see
/// [`is_run_artifact`]), then creates a commit with a standardized message. If
/// nothing is left to stage, returns without creating a commit.
///
/// # Arguments
///
/// * `path` - Path to the git repository (or worktree)
/// * `loop_id` - The loop ID to include in the commit message
///
/// # Returns
///
/// Information about what was committed, or an error if the operation failed.
///
/// # Commit Message Format
///
/// The commit message follows the format:
/// `chore: auto-commit before merge (loop {loop_id})`
pub fn auto_commit_changes(
    path: impl AsRef<Path>,
    loop_id: &str,
) -> Result<AutoCommitResult, GitOpsError> {
    let path = path.as_ref();

    // Check if there are any uncommitted changes
    if !has_uncommitted_changes(path)? {
        return Ok(AutoCommitResult::no_commit());
    }

    stage_loop_work(path)?;

    // Count staged files
    let files_staged = count_staged_files(path)?;

    // If only run artifacts were present, nothing is staged: no commit
    if files_staged == 0 {
        return Ok(AutoCommitResult::no_commit());
    }

    // Create the commit
    let commit_message = format!("chore: auto-commit before merge (loop {})", loop_id);

    let output = Command::new("git")
        .args(["commit", "-m", &commit_message])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check if the failure is due to missing git config
        if stderr.contains("user.email") || stderr.contains("user.name") {
            return Err(GitOpsError::ConfigMissing(
                "user.name or user.email not configured".to_string(),
            ));
        }

        return Err(GitOpsError::Git(format!("Failed to commit: {}", stderr)));
    }

    // Get the commit SHA
    let commit_sha = get_head_sha(path)?;

    Ok(AutoCommitResult {
        committed: true,
        commit_sha: Some(commit_sha),
        files_staged,
    })
}

/// Count the number of files staged for commit.
fn count_staged_files(path: &Path) -> Result<usize, GitOpsError> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().filter(|line| !line.is_empty()).count())
}

/// Get the HEAD commit SHA.
pub fn get_head_sha(path: impl AsRef<Path>) -> Result<String, GitOpsError> {
    let path = path.as_ref();
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(stderr.to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get the current branch name.
///
/// Returns the name of the currently checked out branch, or an error if
/// in detached HEAD state or if git command fails.
///
/// # Arguments
///
/// * `path` - Path to the git repository (or worktree)
pub fn get_current_branch(path: impl AsRef<Path>) -> Result<String, GitOpsError> {
    let path = path.as_ref();
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(stderr.to_string()));
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // "HEAD" indicates detached HEAD state
    if branch == "HEAD" {
        return Err(GitOpsError::Git("Detached HEAD state".to_string()));
    }

    Ok(branch)
}

/// Clear all git stashes in the repository.
///
/// Runs `git stash clear` to remove all stash entries. This is useful for
/// cleaning up temporary state during landing.
///
/// # Arguments
///
/// * `path` - Path to the git repository (or worktree)
///
/// # Returns
///
/// The number of stashes that were cleared (0 if none existed).
pub fn clean_stashes(path: impl AsRef<Path>) -> Result<usize, GitOpsError> {
    let path = path.as_ref();

    // First, count existing stashes
    let output = Command::new("git")
        .args(["stash", "list"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(stderr.to_string()));
    }

    let stash_count = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .count();

    if stash_count == 0 {
        return Ok(0);
    }

    // Clear all stashes
    let output = Command::new("git")
        .args(["stash", "clear"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(format!(
            "Failed to clear stashes: {}",
            stderr
        )));
    }

    Ok(stash_count)
}

/// Prune stale remote-tracking references.
///
/// Runs `git remote prune origin` to remove local references to remote
/// branches that no longer exist. This is useful for cleaning up git state.
///
/// # Arguments
///
/// * `path` - Path to the git repository (or worktree)
pub fn prune_remote_refs(path: impl AsRef<Path>) -> Result<(), GitOpsError> {
    let path = path.as_ref();

    // Check if 'origin' remote exists before pruning
    let output = Command::new("git")
        .args(["remote"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(stderr.to_string()));
    }

    let remotes = String::from_utf8_lossy(&output.stdout);
    if !remotes.lines().any(|r| r.trim() == "origin") {
        // No origin remote, nothing to prune
        return Ok(());
    }

    let output = Command::new("git")
        .args(["remote", "prune", "origin"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(format!(
            "Failed to prune remote refs: {}",
            stderr
        )));
    }

    Ok(())
}

/// Check if the working tree is clean (no uncommitted changes).
///
/// This is the inverse of `has_uncommitted_changes`, provided for semantic clarity.
///
/// # Arguments
///
/// * `path` - Path to the git repository (or worktree)
///
/// # Returns
///
/// `true` if the working tree is clean (no changes), `false` if there are uncommitted changes.
pub fn is_working_tree_clean(path: impl AsRef<Path>) -> Result<bool, GitOpsError> {
    has_uncommitted_changes(path).map(|has_changes| !has_changes)
}

/// Get a short summary of the HEAD commit.
///
/// Returns a string like "abc1234: commit message subject"
///
/// # Arguments
///
/// * `path` - Path to the git repository (or worktree)
pub fn get_commit_summary(path: impl AsRef<Path>) -> Result<String, GitOpsError> {
    let path = path.as_ref();
    let output = Command::new("git")
        .args(["log", "-1", "--format=%h: %s"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitOpsError::Git(stderr.to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get a list of files that were modified in the most recent commits.
///
/// Returns up to `limit` most recently modified files.
///
/// # Arguments
///
/// * `path` - Path to the git repository (or worktree)
/// * `limit` - Maximum number of files to return
pub fn get_recent_files(path: impl AsRef<Path>, limit: usize) -> Result<Vec<String>, GitOpsError> {
    let path = path.as_ref();

    // Try different ranges based on available commits
    // Start with HEAD~5..HEAD, fall back to smaller ranges
    let ranges = ["HEAD~5..HEAD", "HEAD~2..HEAD", "HEAD~1..HEAD"];

    for range in ranges {
        let output = Command::new("git")
            .args(["diff", "--name-only", range, "--"])
            .current_dir(path)
            .output()?;

        if output.status.success() {
            let files = String::from_utf8_lossy(&output.stdout);
            let file_list: Vec<String> = files
                .lines()
                .filter(|line| !line.is_empty())
                .take(limit)
                .map(String::from)
                .collect();

            if !file_list.is_empty() {
                return Ok(file_list);
            }
        }
    }

    // Fall back to listing all tracked files (for new repos with one commit)
    let output = Command::new("git")
        .args(["ls-files", "--"])
        .current_dir(path)
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .take(limit)
        .map(String::from)
        .collect();

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_git_repo(dir: &Path) {
        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@test.local"])
            .current_dir(dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(dir.join("README.md"), "# Test").unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(dir)
            .output()
            .unwrap();
    }

    #[test]
    fn test_has_uncommitted_changes_clean() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        assert!(!has_uncommitted_changes(temp.path()).unwrap());
    }

    #[test]
    fn test_has_uncommitted_changes_untracked() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        fs::write(temp.path().join("new_file.txt"), "content").unwrap();

        assert!(has_uncommitted_changes(temp.path()).unwrap());
    }

    #[test]
    fn test_has_uncommitted_changes_staged() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        fs::write(temp.path().join("staged.txt"), "content").unwrap();
        Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        assert!(has_uncommitted_changes(temp.path()).unwrap());
    }

    #[test]
    fn test_has_uncommitted_changes_modified() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        fs::write(temp.path().join("README.md"), "# Modified").unwrap();

        assert!(has_uncommitted_changes(temp.path()).unwrap());
    }

    #[test]
    fn test_auto_commit_no_changes() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        let result = auto_commit_changes(temp.path(), "test-loop").unwrap();

        assert!(!result.committed);
        assert!(result.commit_sha.is_none());
        assert_eq!(result.files_staged, 0);
    }

    #[test]
    fn test_auto_commit_untracked_files() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        fs::write(temp.path().join("feature.txt"), "new feature").unwrap();

        let result = auto_commit_changes(temp.path(), "loop-123").unwrap();

        assert!(result.committed);
        assert!(result.commit_sha.is_some());
        assert_eq!(result.files_staged, 1);

        // Verify the commit message
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=%s"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        let message = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            message.trim(),
            "chore: auto-commit before merge (loop loop-123)"
        );
    }

    #[test]
    fn test_auto_commit_staged_changes() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        fs::write(temp.path().join("staged.txt"), "staged content").unwrap();
        Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let result = auto_commit_changes(temp.path(), "loop-456").unwrap();

        assert!(result.committed);
        assert!(result.commit_sha.is_some());
        assert_eq!(result.files_staged, 1);
    }

    #[test]
    fn test_auto_commit_unstaged_modifications() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        fs::write(temp.path().join("README.md"), "# Modified content").unwrap();

        let result = auto_commit_changes(temp.path(), "loop-789").unwrap();

        assert!(result.committed);
        assert!(result.commit_sha.is_some());
        assert_eq!(result.files_staged, 1);
    }

    #[test]
    fn test_auto_commit_mixed_changes() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // Untracked file
        fs::write(temp.path().join("new.txt"), "new").unwrap();

        // Staged file
        fs::write(temp.path().join("staged.txt"), "staged").unwrap();
        Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Modified tracked file
        fs::write(temp.path().join("README.md"), "# Modified").unwrap();

        let result = auto_commit_changes(temp.path(), "loop-mixed").unwrap();

        assert!(result.committed);
        assert!(result.commit_sha.is_some());
        assert_eq!(result.files_staged, 3);
    }

    #[test]
    fn test_auto_commit_working_tree_clean_after() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        fs::write(temp.path().join("feature.txt"), "feature").unwrap();

        let result = auto_commit_changes(temp.path(), "loop-clean").unwrap();
        assert!(result.committed);

        // Working tree should be clean after commit
        assert!(!has_uncommitted_changes(temp.path()).unwrap());
    }

    #[test]
    fn test_auto_commit_returns_correct_sha() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        fs::write(temp.path().join("file.txt"), "content").unwrap();

        let result = auto_commit_changes(temp.path(), "loop-sha").unwrap();

        // Verify the returned SHA matches HEAD
        let head_sha = get_head_sha(temp.path()).unwrap();
        assert_eq!(result.commit_sha.unwrap(), head_sha);
    }

    #[test]
    fn test_auto_commit_only_gitignored_files() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // Add gitignore
        fs::write(temp.path().join(".gitignore"), "*.log\n").unwrap();
        Command::new("git")
            .args(["add", ".gitignore"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Add gitignore"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Create only ignored files
        fs::write(temp.path().join("debug.log"), "log content").unwrap();

        // Should report no uncommitted changes (ignored files don't count)
        assert!(!has_uncommitted_changes(temp.path()).unwrap());

        let result = auto_commit_changes(temp.path(), "loop-ignored").unwrap();
        assert!(!result.committed);
    }

    fn write_file(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn files_in_head_commit(dir: &Path) -> Vec<String> {
        let output = Command::new("git")
            .args(["show", "--name-only", "--pretty=format:", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        let mut files: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        files.sort();
        files
    }

    /// The artifacts from the observed 893-file auto-commit, in a repo with no
    /// .gitignore help: nested `.ralph/` state under crate dirs, root `logs/`,
    /// and the loop's own runtime state and scratchpad.
    const RUN_ARTIFACTS: &[&str] = &[
        "crates/ralph-core/.ralph/diagnostics/2026-11-12T21-00-00/agent-output.jsonl",
        "crates/ralph-cli/.ralph/diagnostics/2026-11-12T21-00-00/orchestration.jsonl",
        "crates/ralph-core/.ralph/agent/scratchpad.md",
        "logs/report.txt",
        "logs/nested/run-1.txt",
        ".ralph/agent/scratchpad.md",
        ".ralph/agent/tasks.jsonl",
        ".ralph/events-20261112-210407.jsonl",
        ".ralph/diagnostics/2026-11-12T21-00-00/errors.jsonl",
        ".ralph/loop.lock",
        ".ralph/specs/feature/logs/step01-red.txt",
    ];

    #[test]
    fn test_auto_commit_leaves_run_artifacts_out() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        write_file(temp.path(), "src/feature.rs", "pub fn feature() {}\n");
        for artifact in RUN_ARTIFACTS {
            write_file(temp.path(), artifact, "artifact\n");
        }

        let result = auto_commit_changes(temp.path(), "loop-artifacts").unwrap();

        assert!(result.committed);
        assert_eq!(files_in_head_commit(temp.path()), vec!["src/feature.rs"]);
        assert_eq!(result.files_staged, 1);
    }

    #[test]
    fn test_auto_commit_only_artifacts_makes_no_commit() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());
        let head_before = get_head_sha(temp.path()).unwrap();

        for artifact in RUN_ARTIFACTS {
            write_file(temp.path(), artifact, "artifact\n");
        }

        let result = auto_commit_changes(temp.path(), "loop-artifacts-only").unwrap();

        assert!(!result.committed);
        assert_eq!(get_head_sha(temp.path()).unwrap(), head_before);
    }

    #[test]
    fn test_auto_commit_still_commits_intended_new_files_alongside_artifacts() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // A tracked file under logs/ was committed on purpose; edits to it stay in.
        write_file(temp.path(), "logs/README.md", "# Logs\n");
        Command::new("git")
            .args(["add", "logs/README.md"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "track logs readme"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        write_file(temp.path(), "logs/README.md", "# Logs, edited\n");

        let intended = [
            "src/feature.rs",
            "src/logs/mod.rs",
            "crates/new-crate/src/lib.rs",
            "docs/file with spaces.md",
            // Must be staged literally: as a glob it would also match `logs/`.
            "logs*",
            ".ralph/agent/memories.md",
            ".ralph/agent/decisions.md",
            ".ralph/specs/feature/design.md",
            ".ralph/tasks/feature/task-01.code-task.md",
        ];
        for file in intended {
            write_file(temp.path(), file, "intended\n");
        }
        for artifact in RUN_ARTIFACTS {
            write_file(temp.path(), artifact, "artifact\n");
        }

        let result = auto_commit_changes(temp.path(), "loop-intended").unwrap();

        let mut expected: Vec<String> = intended
            .iter()
            .chain(["logs/README.md"].iter())
            .map(|s| s.to_string())
            .collect();
        expected.sort();
        assert!(result.committed);
        assert_eq!(files_in_head_commit(temp.path()), expected);
        assert_eq!(result.files_staged, expected.len());
    }

    #[test]
    fn test_get_current_branch() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        let branch = get_current_branch(temp.path()).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn test_get_current_branch_custom() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // Create and checkout a new branch
        Command::new("git")
            .args(["checkout", "-b", "feature-branch"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let branch = get_current_branch(temp.path()).unwrap();
        assert_eq!(branch, "feature-branch");
    }

    #[test]
    fn test_clean_stashes_empty() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // No stashes initially
        let cleared = clean_stashes(temp.path()).unwrap();
        assert_eq!(cleared, 0);
    }

    #[test]
    fn test_clean_stashes_with_stash() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // Create a change and stash it
        fs::write(temp.path().join("README.md"), "# Modified").unwrap();
        Command::new("git")
            .args(["stash", "push", "-m", "test stash"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Create another change and stash it
        fs::write(temp.path().join("README.md"), "# Modified again").unwrap();
        Command::new("git")
            .args(["stash", "push", "-m", "test stash 2"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        // Clear stashes
        let cleared = clean_stashes(temp.path()).unwrap();
        assert_eq!(cleared, 2);

        // Verify stashes are gone
        let cleared_again = clean_stashes(temp.path()).unwrap();
        assert_eq!(cleared_again, 0);
    }

    #[test]
    fn test_prune_remote_refs_no_origin() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // Should succeed even without origin remote
        prune_remote_refs(temp.path()).unwrap();
    }

    #[test]
    fn test_is_working_tree_clean() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // Clean working tree
        assert!(is_working_tree_clean(temp.path()).unwrap());

        // Make a change
        fs::write(temp.path().join("new_file.txt"), "content").unwrap();

        // Now it's dirty
        assert!(!is_working_tree_clean(temp.path()).unwrap());
    }

    #[test]
    fn test_get_commit_summary() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        let summary = get_commit_summary(temp.path()).unwrap();
        assert!(summary.contains("Initial commit"), "Got: {}", summary);
    }

    #[test]
    fn test_get_recent_files() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());

        // Create and commit a new file
        fs::write(temp.path().join("feature.txt"), "content").unwrap();
        Command::new("git")
            .args(["add", "feature.txt"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Add feature"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let files = get_recent_files(temp.path(), 10).unwrap();
        assert!(
            files.contains(&"feature.txt".to_string()),
            "Got: {:?}",
            files
        );
    }
}
