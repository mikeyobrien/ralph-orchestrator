//! Git worktree management for parallel Ralph loops.
//!
//! Provides filesystem isolation for concurrent loops using git worktrees.
//! Each parallel loop gets its own working directory with full filesystem
//! isolation, sharing only `.git` history. Conflicts are resolved at merge time.
//!
//! # Example
//!
//! ```no_run
//! use ralph_core::worktree::{Worktree, WorktreeConfig, create_worktree, remove_worktree, list_worktrees};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = WorktreeConfig::default();
//!
//!     // Create worktree for a parallel loop
//!     let worktree = create_worktree(".", "ralph-20250124-a3f2", &config)?;
//!     println!("Created worktree at: {}", worktree.path.display());
//!
//!     // List all worktrees
//!     let worktrees = list_worktrees(".")?;
//!     for wt in worktrees {
//!         println!("  {}: {}", wt.branch, wt.path.display());
//!     }
//!
//!     // Clean up when done
//!     remove_worktree(".", &worktree.path)?;
//!     Ok(())
//! }
//! ```

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Configuration for worktree operations.
#[derive(Debug, Clone)]
pub struct WorktreeConfig {
    /// Directory where worktrees are created (default: `.worktrees`).
    pub worktree_dir: PathBuf,
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            worktree_dir: PathBuf::from(".worktrees"),
        }
    }
}

impl WorktreeConfig {
    /// Create config with custom worktree directory.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            worktree_dir: dir.into(),
        }
    }

    /// Get the absolute path to worktree directory relative to repo root.
    pub fn worktree_path(&self, repo_root: &Path) -> PathBuf {
        if self.worktree_dir.is_absolute() {
            self.worktree_dir.clone()
        } else {
            repo_root.join(&self.worktree_dir)
        }
    }
}

/// Information about a git worktree.
#[derive(Debug, Clone)]
pub struct Worktree {
    /// Absolute path to the worktree directory.
    pub path: PathBuf,

    /// The branch checked out in this worktree.
    pub branch: String,

    /// Whether this is the main worktree.
    pub is_main: bool,

    /// HEAD commit (if available).
    pub head: Option<String>,
}

/// Errors that can occur during worktree operations.
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Git command failed.
    #[error("Git command failed: {0}")]
    Git(String),

    /// Worktree already exists.
    #[error("Worktree already exists: {0}")]
    AlreadyExists(String),

    /// Worktree not found.
    #[error("Worktree not found: {0}")]
    NotFound(String),

    /// Not a git repository.
    #[error("Not a git repository: {0}")]
    NotARepo(String),

    /// Branch already exists.
    #[error("Branch already exists: {0}")]
    BranchExists(String),
}

/// Create a new worktree for a parallel Ralph loop.
///
/// Creates a new branch and worktree at `{config.worktree_dir}/{loop_id}`.
/// The branch is created from HEAD of the current branch.
///
/// # Arguments
///
/// * `repo_root` - Root of the git repository
/// * `loop_id` - Unique identifier for the loop (e.g., "ralph-20250124-a3f2")
/// * `config` - Worktree configuration
///
/// # Returns
///
/// Information about the created worktree.
pub fn create_worktree(
    repo_root: impl AsRef<Path>,
    loop_id: &str,
    config: &WorktreeConfig,
) -> Result<Worktree, WorktreeError> {
    let repo_root = repo_root.as_ref();

    // Verify this is a git repository
    if !repo_root.join(".git").exists() && !repo_root.join(".git").is_file() {
        return Err(WorktreeError::NotARepo(
            repo_root.to_string_lossy().to_string(),
        ));
    }

    let worktree_base = config.worktree_path(repo_root);
    let worktree_path = worktree_base.join(loop_id);
    let branch_name = format!("ralph/{loop_id}");

    // Check if worktree already exists
    if worktree_path.exists() {
        return Err(WorktreeError::AlreadyExists(
            worktree_path.to_string_lossy().to_string(),
        ));
    }

    // Ensure worktree directory exists
    fs::create_dir_all(&worktree_base)?;

    // Create worktree with new branch
    // git worktree add -b <branch> <path>
    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(&worktree_path)
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check for specific error cases
        if stderr.contains("already exists") {
            if stderr.contains("branch") {
                return Err(WorktreeError::BranchExists(branch_name));
            }
            return Err(WorktreeError::AlreadyExists(
                worktree_path.to_string_lossy().to_string(),
            ));
        }

        return Err(WorktreeError::Git(stderr.to_string()));
    }

    // Get the HEAD commit
    let head = get_head_commit(&worktree_path).ok();

    tracing::debug!(
        "Created worktree at {} on branch {}",
        worktree_path.display(),
        branch_name
    );

    Ok(Worktree {
        path: worktree_path,
        branch: branch_name,
        is_main: false,
        head,
    })
}

/// Remove a worktree and optionally its branch.
///
/// # Arguments
///
/// * `repo_root` - Root of the git repository
/// * `worktree_path` - Path to the worktree to remove
///
/// # Note
///
/// This also deletes the associated branch if it exists.
pub fn remove_worktree(
    repo_root: impl AsRef<Path>,
    worktree_path: impl AsRef<Path>,
) -> Result<(), WorktreeError> {
    let repo_root = repo_root.as_ref();
    let worktree_path = worktree_path.as_ref();

    if !worktree_path.exists() {
        return Err(WorktreeError::NotFound(
            worktree_path.to_string_lossy().to_string(),
        ));
    }

    // Get the branch name before removing
    let branch = get_worktree_branch(worktree_path);

    // Remove the worktree (--force handles uncommitted changes)
    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::Git(stderr.to_string()));
    }

    // Delete the branch if it was a ralph/* branch
    if let Some(branch) = branch
        && branch.starts_with("ralph/")
    {
        let output = Command::new("git")
            .args(["branch", "-D", &branch])
            .current_dir(repo_root)
            .output()?;

        if !output.status.success() {
            // Non-fatal: branch might already be deleted
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::debug!("Failed to delete branch {}: {}", branch, stderr);
        }
    }

    // Prune worktree refs
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo_root)
        .output();

    tracing::debug!("Removed worktree at {}", worktree_path.display());

    Ok(())
}

/// List all git worktrees in the repository.
///
/// # Arguments
///
/// * `repo_root` - Root of the git repository (can be any worktree)
///
/// # Returns
///
/// List of all worktrees, including the main worktree.
pub fn list_worktrees(repo_root: impl AsRef<Path>) -> Result<Vec<Worktree>, WorktreeError> {
    let repo_root = repo_root.as_ref();

    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::Git(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_worktree_list(&stdout)
}

/// Parse the porcelain output of `git worktree list`.
fn parse_worktree_list(output: &str) -> Result<Vec<Worktree>, WorktreeError> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_head: Option<String> = None;
    let mut current_branch: Option<String> = None;
    let mut is_bare = false;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            // Save previous worktree if any
            if let Some(path) = current_path.take()
                && !is_bare
            {
                worktrees.push(Worktree {
                    path,
                    branch: current_branch
                        .take()
                        .unwrap_or_else(|| "(detached)".to_string()),
                    is_main: worktrees.is_empty(), // First one is main
                    head: current_head.take(),
                });
            }

            current_path = Some(PathBuf::from(line.strip_prefix("worktree ").unwrap()));
            current_head = None;
            current_branch = None;
            is_bare = false;
        } else if line.starts_with("HEAD ") {
            current_head = Some(line.strip_prefix("HEAD ").unwrap().to_string());
        } else if line.starts_with("branch ") {
            // Branch is in format "refs/heads/branch-name"
            let branch_ref = line.strip_prefix("branch ").unwrap();
            current_branch = Some(
                branch_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch_ref)
                    .to_string(),
            );
        } else if line == "bare" {
            is_bare = true;
        }
    }

    // Don't forget the last one
    if let Some(path) = current_path
        && !is_bare
    {
        worktrees.push(Worktree {
            path,
            branch: current_branch.unwrap_or_else(|| "(detached)".to_string()),
            is_main: worktrees.is_empty(),
            head: current_head,
        });
    }

    Ok(worktrees)
}

/// Ensure the worktree directory is in `.gitignore`.
///
/// Appends the pattern to `.gitignore` if not already present.
///
/// # Arguments
///
/// * `repo_root` - Root of the git repository
/// * `worktree_dir` - The worktree directory pattern to ignore (e.g., ".worktrees")
pub fn ensure_gitignore(
    repo_root: impl AsRef<Path>,
    worktree_dir: &str,
) -> Result<(), WorktreeError> {
    let repo_root = repo_root.as_ref();
    let gitignore_path = repo_root.join(".gitignore");

    // Pattern to add (with trailing slash for directory)
    let pattern = if worktree_dir.ends_with('/') {
        worktree_dir.to_string()
    } else {
        format!("{}/", worktree_dir)
    };

    // Check if pattern already exists
    if gitignore_path.exists() {
        let file = File::open(&gitignore_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();

            // Check if this line matches our pattern (with or without trailing slash)
            if trimmed == pattern || trimmed == pattern.trim_end_matches('/') {
                tracing::debug!("Pattern {} already in .gitignore", pattern);
                return Ok(());
            }
        }
    }

    // Append the pattern
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore_path)?;

    // Add newline before if file exists and doesn't end with newline
    if gitignore_path.exists() {
        let contents = fs::read_to_string(&gitignore_path)?;
        if !contents.is_empty() && !contents.ends_with('\n') {
            writeln!(file)?;
        }
    }

    writeln!(file, "{}", pattern)?;

    tracing::debug!("Added {} to .gitignore", pattern);

    Ok(())
}

/// Get the branch name for a worktree.
fn get_worktree_branch(worktree_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch != "HEAD" {
            return Some(branch);
        }
    }
    None
}

/// Get the HEAD commit SHA for a worktree.
fn get_head_commit(worktree_path: &Path) -> Result<String, WorktreeError> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(worktree_path)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(WorktreeError::Git(stderr.to_string()))
    }
}

/// Get the list of Ralph-specific worktrees (those with `ralph/` branches).
pub fn list_ralph_worktrees(repo_root: impl AsRef<Path>) -> Result<Vec<Worktree>, WorktreeError> {
    let all = list_worktrees(repo_root)?;
    Ok(all
        .into_iter()
        .filter(|wt| wt.branch.starts_with("ralph/"))
        .collect())
}

/// Check if a worktree exists for the given loop ID.
pub fn worktree_exists(
    repo_root: impl AsRef<Path>,
    loop_id: &str,
    config: &WorktreeConfig,
) -> bool {
    let worktree_path = config.worktree_path(repo_root.as_ref()).join(loop_id);
    worktree_path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
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

        // Create initial commit (required for worktrees)
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
    fn test_worktree_config_default() {
        let config = WorktreeConfig::default();
        assert_eq!(config.worktree_dir, PathBuf::from(".worktrees"));
    }

    #[test]
    fn test_worktree_config_path() {
        let config = WorktreeConfig::default();
        let repo = Path::new("/repo");
        assert_eq!(
            config.worktree_path(repo),
            PathBuf::from("/repo/.worktrees")
        );

        let absolute_config = WorktreeConfig::with_dir("/tmp/worktrees");
        assert_eq!(
            absolute_config.worktree_path(repo),
            PathBuf::from("/tmp/worktrees")
        );
    }

    #[test]
    fn test_create_and_remove_worktree() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        let config = WorktreeConfig::default();
        let loop_id = "test-loop-123";

        // Create worktree
        let worktree = create_worktree(temp_dir.path(), loop_id, &config).unwrap();

        assert!(worktree.path.exists());
        assert_eq!(worktree.branch, "ralph/test-loop-123");
        assert!(!worktree.is_main);
        assert!(worktree.head.is_some());

        // Verify README was copied
        assert!(worktree.path.join("README.md").exists());

        // Remove worktree
        remove_worktree(temp_dir.path(), &worktree.path).unwrap();
        assert!(!worktree.path.exists());
    }

    #[test]
    fn test_create_worktree_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        let config = WorktreeConfig::default();
        let loop_id = "duplicate";

        // Create first worktree
        let _wt = create_worktree(temp_dir.path(), loop_id, &config).unwrap();

        // Try to create duplicate
        let result = create_worktree(temp_dir.path(), loop_id, &config);
        assert!(matches!(result, Err(WorktreeError::AlreadyExists(_))));
    }

    #[test]
    fn test_list_worktrees() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        // Initially just the main worktree
        let worktrees = list_worktrees(temp_dir.path()).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].is_main);

        // Add a worktree
        let config = WorktreeConfig::default();
        let _wt = create_worktree(temp_dir.path(), "loop-1", &config).unwrap();

        let worktrees = list_worktrees(temp_dir.path()).unwrap();
        assert_eq!(worktrees.len(), 2);
    }

    #[test]
    fn test_list_ralph_worktrees() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        let config = WorktreeConfig::default();
        let _wt1 = create_worktree(temp_dir.path(), "loop-1", &config).unwrap();
        let _wt2 = create_worktree(temp_dir.path(), "loop-2", &config).unwrap();

        let ralph_worktrees = list_ralph_worktrees(temp_dir.path()).unwrap();
        assert_eq!(ralph_worktrees.len(), 2);
        assert!(
            ralph_worktrees
                .iter()
                .all(|wt| wt.branch.starts_with("ralph/"))
        );
    }

    #[test]
    fn test_ensure_gitignore_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore = temp_dir.path().join(".gitignore");

        assert!(!gitignore.exists());

        ensure_gitignore(temp_dir.path(), ".worktrees").unwrap();

        assert!(gitignore.exists());
        let contents = fs::read_to_string(&gitignore).unwrap();
        assert!(contents.contains(".worktrees/"));
    }

    #[test]
    fn test_ensure_gitignore_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore = temp_dir.path().join(".gitignore");

        fs::write(&gitignore, "node_modules/\n").unwrap();

        ensure_gitignore(temp_dir.path(), ".worktrees").unwrap();

        let contents = fs::read_to_string(&gitignore).unwrap();
        assert!(contents.contains("node_modules/"));
        assert!(contents.contains(".worktrees/"));
    }

    #[test]
    fn test_ensure_gitignore_already_present() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore = temp_dir.path().join(".gitignore");

        fs::write(&gitignore, ".worktrees/\n").unwrap();

        ensure_gitignore(temp_dir.path(), ".worktrees").unwrap();

        let contents = fs::read_to_string(&gitignore).unwrap();
        // Should only appear once
        assert_eq!(contents.matches(".worktrees/").count(), 1);
    }

    #[test]
    fn test_ensure_gitignore_without_trailing_slash() {
        let temp_dir = TempDir::new().unwrap();
        let gitignore = temp_dir.path().join(".gitignore");

        // Existing pattern without trailing slash
        fs::write(&gitignore, ".worktrees\n").unwrap();

        ensure_gitignore(temp_dir.path(), ".worktrees").unwrap();

        let contents = fs::read_to_string(&gitignore).unwrap();
        // Should not add duplicate
        assert!(!contents.contains(".worktrees/\n.worktrees/"));
    }

    #[test]
    fn test_worktree_exists() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        let config = WorktreeConfig::default();
        let loop_id = "check-exists";

        assert!(!worktree_exists(temp_dir.path(), loop_id, &config));

        let _wt = create_worktree(temp_dir.path(), loop_id, &config).unwrap();

        assert!(worktree_exists(temp_dir.path(), loop_id, &config));
    }

    #[test]
    fn test_not_a_repo() {
        let temp_dir = TempDir::new().unwrap();
        // Don't init git

        let config = WorktreeConfig::default();
        let result = create_worktree(temp_dir.path(), "loop-1", &config);

        assert!(matches!(result, Err(WorktreeError::NotARepo(_))));
    }

    #[test]
    fn test_remove_nonexistent_worktree() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path());

        let result = remove_worktree(temp_dir.path(), temp_dir.path().join("nonexistent"));

        assert!(matches!(result, Err(WorktreeError::NotFound(_))));
    }

    #[test]
    fn test_parse_worktree_list() {
        let output = r"worktree /path/to/main
HEAD abc123def
branch refs/heads/main

worktree /path/to/.worktrees/loop-1
HEAD def456ghi
branch refs/heads/ralph/loop-1

";

        let worktrees = parse_worktree_list(output).unwrap();
        assert_eq!(worktrees.len(), 2);

        assert_eq!(worktrees[0].path, PathBuf::from("/path/to/main"));
        assert_eq!(worktrees[0].branch, "main");
        assert!(worktrees[0].is_main);
        assert_eq!(worktrees[0].head, Some("abc123def".to_string()));

        assert_eq!(
            worktrees[1].path,
            PathBuf::from("/path/to/.worktrees/loop-1")
        );
        assert_eq!(worktrees[1].branch, "ralph/loop-1");
        assert!(!worktrees[1].is_main);
    }
}
