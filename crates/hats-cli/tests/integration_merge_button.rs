//! Integration tests for merge button UX improvements.
//!
//! These tests verify the enhanced merge button behavior per spec requirements:
//! 1. Merge button on worktrees shows active/blocked states
//! 2. Merge only available when no work on primary repo/loop
//! 3. Smart merge reads latest commits for execution summary
//! 4. User steering request for unclear merges

use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Set up a temp directory with git repo and .hats directory.
fn setup_workspace() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(temp_path)
        .output()?;

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp_path)
        .output()?;

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_path)
        .output()?;

    // Create initial commit
    fs::write(temp_path.join("README.md"), "# Test Repo")?;
    Command::new("git")
        .args(["add", "."])
        .current_dir(temp_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(temp_path)
        .output()?;

    // Create .hats directory
    fs::create_dir_all(temp_path.join(".hats"))?;

    Ok(temp_dir)
}

/// Create a worktree with some commits for testing.
fn create_worktree_with_commits(
    temp_path: &std::path::Path,
    loop_id: &str,
    num_commits: usize,
) -> Result<std::path::PathBuf> {
    let worktree_path = temp_path.join(".worktrees").join(loop_id);
    let branch_name = format!("hats/{}", loop_id);

    // Create worktree
    Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(&worktree_path)
        .current_dir(temp_path)
        .output()?;

    // Create commits in worktree
    for i in 0..num_commits {
        let filename = format!("file_{}.txt", i);
        fs::write(worktree_path.join(&filename), format!("Content {}", i))?;
        Command::new("git")
            .args(["add", &filename])
            .current_dir(&worktree_path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", &format!("Add file {}", i)])
            .current_dir(&worktree_path)
            .output()?;
    }

    Ok(worktree_path)
}

/// Write a loop lock file (simulates primary loop running).
fn write_loop_lock(temp_path: &std::path::Path, pid: u32, prompt: &str) -> Result<()> {
    let lock_path = temp_path.join(".hats/loop.lock");
    fs::write(
        lock_path,
        format!(
            "{{\"pid\":{},\"prompt\":\"{}\",\"started\":\"2024-01-21T12:00:00Z\"}}",
            pid, prompt
        ),
    )?;
    Ok(())
}

/// Run hats loops command with given args in the temp directory.
fn hats_loops(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_hats"))
        .arg("loops")
        .args(args)
        .current_dir(temp_path)
        .output()
        .expect("Failed to execute hats loops command")
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Merge Button State Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_merge_button_state_active_when_primary_idle() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree loop that has completed and queued for merge
    let _worktree_path = create_worktree_with_commits(temp_path, "test-loop-001", 2)?;

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-001", "Add new feature")?;

    // And: No primary loop is running (no lock file)
    // (implicitly no lock file exists)

    // When: Checking merge button state via API
    let state = hats_core::merge_button_state(temp_path, "test-loop-001")?;

    // Then: Button should be active (can merge now)
    assert_eq!(
        state,
        hats_core::MergeButtonState::Active,
        "Merge button should be Active when primary loop is idle"
    );

    Ok(())
}

#[test]
fn test_merge_button_state_blocked_when_primary_running() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree loop that has completed and queued for merge
    let _worktree_path = create_worktree_with_commits(temp_path, "test-loop-002", 2)?;

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-002", "Add another feature")?;

    // And: Primary loop IS running (lock file exists with active PID)
    write_loop_lock(temp_path, std::process::id(), "Working on something")?;

    // When: Checking merge button state via API
    let state = hats_core::merge_button_state(temp_path, "test-loop-002")?;

    // Then: Button should be blocked with reason
    assert!(
        matches!(state, hats_core::MergeButtonState::Blocked { ref reason } if reason.contains("primary")),
        "Merge button should be Blocked when primary loop is running. Got: {:?}",
        state
    );

    Ok(())
}

#[test]
fn test_merge_button_state_blocked_reason_shows_primary_prompt() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree loop queued for merge
    let _worktree_path = create_worktree_with_commits(temp_path, "test-loop-003", 1)?;

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-003", "Feature X")?;

    // And: Primary loop is working on something specific
    write_loop_lock(temp_path, std::process::id(), "Implementing authentication")?;

    // When: Getting merge button state
    let state = hats_core::merge_button_state(temp_path, "test-loop-003")?;

    // Then: Blocked reason should explain what primary is doing (for tooltip)
    match state {
        hats_core::MergeButtonState::Blocked { reason } => {
            assert!(
                reason.contains("authentication") || reason.contains("Implementing"),
                "Blocked reason should show primary loop's prompt for tooltip. Got: {}",
                reason
            );
        }
        hats_core::MergeButtonState::Active => panic!("Expected Blocked state, got: {:?}", state),
    }

    Ok(())
}

#[test]
fn test_merge_button_state_blocked_when_merge_already_running() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree loop queued for merge
    let _worktree_path = create_worktree_with_commits(temp_path, "test-loop-004", 1)?;

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-004", "Feature Y")?;

    // And: This loop is already being merged (in merging state)
    queue.mark_merging("test-loop-004", 99999)?;

    // When: Checking merge button state
    let state = hats_core::merge_button_state(temp_path, "test-loop-004")?;

    // Then: Button should be blocked (merge in progress)
    assert!(
        matches!(state, hats_core::MergeButtonState::Blocked { ref reason } if reason.contains("progress") || reason.contains("merging")),
        "Merge button should be Blocked when merge is in progress. Got: {:?}",
        state
    );

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Smart Merge Commit Summary Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_smart_merge_reads_commits_for_summary() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree with specific commits
    let worktree_path = create_worktree_with_commits(temp_path, "test-loop-005", 3)?;

    // And: The worktree has meaningful commit messages
    fs::write(worktree_path.join("auth.rs"), "fn login() {}")?;
    Command::new("git")
        .args(["add", "auth.rs"])
        .current_dir(&worktree_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "feat(auth): implement login endpoint"])
        .current_dir(&worktree_path)
        .output()?;

    // When: Generating merge summary
    let summary = hats_core::smart_merge_summary(temp_path, "test-loop-005")?;

    // Then: Summary should reflect the commit content
    assert!(
        summary.contains("auth") || summary.contains("login"),
        "Summary should mention key terms from commits. Got: {}",
        summary
    );

    Ok(())
}

#[test]
fn test_smart_merge_summary_respects_72_char_limit() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree with many commits (long summary)
    let worktree_path = create_worktree_with_commits(temp_path, "test-loop-006", 5)?;

    for i in 0..5 {
        let filename = format!("feature_{}.rs", i);
        fs::write(worktree_path.join(&filename), format!("// feature {}", i))?;
        Command::new("git")
            .args(["add", &filename])
            .current_dir(&worktree_path)
            .output()?;
        Command::new("git")
            .args([
                "commit",
                "-m",
                &format!("feat: add very important feature number {}", i),
            ])
            .current_dir(&worktree_path)
            .output()?;
    }

    // When: Generating merge summary
    let summary = hats_core::smart_merge_summary(temp_path, "test-loop-006")?;

    // Then: Full merge commit subject should be ≤ 72 chars
    let loop_id = "test-loop-006";
    let full_subject = format!("merge(hats): {} (loop {})", summary, loop_id);

    assert!(
        full_subject.len() <= 72,
        "Full commit subject should be ≤ 72 chars. Got {} chars: {}",
        full_subject.len(),
        full_subject
    );

    Ok(())
}

#[test]
fn test_smart_merge_summary_single_line() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree with commits
    let _worktree_path = create_worktree_with_commits(temp_path, "test-loop-007", 2)?;

    // When: Generating merge summary
    let summary = hats_core::smart_merge_summary(temp_path, "test-loop-007")?;

    // Then: Summary should be single line (no newlines)
    assert!(
        !summary.contains('\n'),
        "Summary should be single line. Got: {}",
        summary
    );

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. User Steering for Unclear Merges Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_merge_needs_steering_when_conflicting_files() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree that modified files also modified on main
    let worktree_path = create_worktree_with_commits(temp_path, "test-loop-008", 1)?;

    // Modify same file on main branch
    fs::write(temp_path.join("README.md"), "# Updated on main")?;
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(temp_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Update README on main"])
        .current_dir(temp_path)
        .output()?;

    // Also modify README in worktree
    fs::write(worktree_path.join("README.md"), "# Updated in worktree")?;
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&worktree_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Update README in worktree"])
        .current_dir(&worktree_path)
        .output()?;

    // When: Checking if merge needs user steering
    let needs_steering = hats_core::merge_needs_steering(temp_path, "test-loop-008")?;

    // Then: Should indicate steering is needed due to potential conflicts
    assert!(
        needs_steering.needs_input,
        "Should need user steering when files conflict"
    );
    assert!(
        needs_steering.reason.contains("conflict")
            || needs_steering.reason.contains("README")
            || needs_steering.reason.contains("modified"),
        "Reason should explain the conflict. Got: {}",
        needs_steering.reason
    );

    Ok(())
}

#[test]
fn test_merge_no_steering_for_clean_additions() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree that only adds new files (no conflicts)
    let worktree_path = temp_path.join(".worktrees").join("test-loop-009");
    let branch_name = "hats/test-loop-009";

    Command::new("git")
        .args(["worktree", "add", "-b", branch_name])
        .arg(&worktree_path)
        .current_dir(temp_path)
        .output()?;

    // Add completely new files in worktree
    fs::write(worktree_path.join("new_feature.rs"), "fn new() {}")?;
    Command::new("git")
        .args(["add", "new_feature.rs"])
        .current_dir(&worktree_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Add new feature"])
        .current_dir(&worktree_path)
        .output()?;

    // When: Checking if merge needs user steering
    let needs_steering = hats_core::merge_needs_steering(temp_path, "test-loop-009")?;

    // Then: Should NOT need steering (clean addition)
    assert!(
        !needs_steering.needs_input,
        "Should not need steering for clean file additions"
    );

    Ok(())
}

#[test]
fn test_merge_steering_provides_helpful_options() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree that might need steering
    let worktree_path = create_worktree_with_commits(temp_path, "test-loop-010", 1)?;

    // Create potential conflict
    fs::write(temp_path.join("config.yml"), "version: 1")?;
    Command::new("git")
        .args(["add", "config.yml"])
        .current_dir(temp_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Add config"])
        .current_dir(temp_path)
        .output()?;

    fs::write(worktree_path.join("config.yml"), "version: 2")?;
    Command::new("git")
        .args(["add", "config.yml"])
        .current_dir(&worktree_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "Update config"])
        .current_dir(&worktree_path)
        .output()?;

    // When: Getting steering decision info
    let needs_steering = hats_core::merge_needs_steering(temp_path, "test-loop-010")?;

    // Then: Should provide actionable options
    if needs_steering.needs_input {
        assert!(
            !needs_steering.options.is_empty(),
            "Should provide options when steering needed"
        );

        // Should have common merge options
        let option_labels: Vec<&str> = needs_steering
            .options
            .iter()
            .map(|o| o.label.as_str())
            .collect();
        assert!(
            option_labels
                .iter()
                .any(|l| l.contains("ours") || l.contains("theirs") || l.contains("manual")),
            "Should provide merge strategy options. Got: {:?}",
            option_labels
        );
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. hats loops list UX Improvements
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_loops_list_shows_merge_button_state_column() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree loop queued for merge
    let _worktree_path = create_worktree_with_commits(temp_path, "test-loop-011", 1)?;

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-011", "Feature Z")?;

    // When: Running hats loops list
    let output = hats_loops(temp_path, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Then: Should show merge button state (or MERGE column)
    assert!(
        stdout.contains("MERGE") || stdout.contains("merge"),
        "Should show merge column in loops list. Got:\n{}",
        stdout
    );

    Ok(())
}

#[test]
fn test_loops_list_shows_active_merge_button_indicator() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree loop queued for merge
    let _worktree_path = create_worktree_with_commits(temp_path, "test-loop-012", 1)?;

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-012", "Feature A")?;

    // And: No primary loop running (merge is possible)
    // (implicitly no lock)

    // When: Running hats loops list
    let output = hats_loops(temp_path, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Then: Should show active/ready indicator for merge
    assert!(
        stdout.contains("ready")
            || stdout.contains("Ready")
            || stdout.contains("✓")
            || stdout.contains("active"),
        "Should show active merge indicator when merge is possible. Got:\n{}",
        stdout
    );

    Ok(())
}

#[test]
fn test_loops_list_shows_blocked_merge_indicator_with_reason() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree loop queued for merge
    let _worktree_path = create_worktree_with_commits(temp_path, "test-loop-013", 1)?;

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-013", "Feature B")?;

    // And: Primary loop is running
    write_loop_lock(temp_path, std::process::id(), "Busy with other work")?;

    // When: Running hats loops list
    let output = hats_loops(temp_path, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Then: Should show blocked indicator
    assert!(
        stdout.contains("blocked")
            || stdout.contains("Blocked")
            || stdout.contains("✗")
            || stdout.contains("waiting"),
        "Should show blocked merge indicator when primary is busy. Got:\n{}",
        stdout
    );

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Execution Summary Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_merge_execution_summary_describes_merged_content() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A completed merge
    let worktree_path = create_worktree_with_commits(temp_path, "test-loop-014", 2)?;

    // Add specific feature
    fs::write(worktree_path.join("user_auth.rs"), "fn authenticate() {}")?;
    Command::new("git")
        .args(["add", "user_auth.rs"])
        .current_dir(&worktree_path)
        .output()?;
    Command::new("git")
        .args(["commit", "-m", "feat(auth): add user authentication"])
        .current_dir(&worktree_path)
        .output()?;

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-014", "Implement user authentication")?;
    queue.mark_merging("test-loop-014", 12345)?;

    // When: Generating execution summary for the merge
    let summary = hats_core::merge_execution_summary(temp_path, "test-loop-014")?;

    // Then: Summary should describe what was merged
    assert!(
        summary.contains("auth") || summary.contains("user") || summary.contains("authentication"),
        "Execution summary should describe merged content. Got: {}",
        summary
    );

    // And: Should include file count or commit count
    assert!(
        summary.contains("file") || summary.contains("commit") || summary.contains("change"),
        "Execution summary should quantify changes. Got: {}",
        summary
    );

    Ok(())
}

#[test]
fn test_merge_execution_summary_includes_stats() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A worktree with multiple changes
    let worktree_path = create_worktree_with_commits(temp_path, "test-loop-015", 3)?;

    // Add multiple files
    for i in 0..3 {
        let filename = format!("module_{}.rs", i);
        fs::write(
            worktree_path.join(&filename),
            format!("// Module {}\npub fn func{}() {{}}", i, i),
        )?;
        Command::new("git")
            .args(["add", &filename])
            .current_dir(&worktree_path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", &format!("Add module {}", i)])
            .current_dir(&worktree_path)
            .output()?;
    }

    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("test-loop-015", "Add multiple modules")?;
    queue.mark_merging("test-loop-015", 12345)?;

    // When: Generating execution summary
    let summary = hats_core::merge_execution_summary(temp_path, "test-loop-015")?;

    // Then: Should include meaningful stats
    // Either file count, line count, or commit count
    let has_stats = summary.contains('3')
        || summary.contains("three")
        || summary.contains("files")
        || summary.contains("commits")
        || summary.contains("modules");

    assert!(
        has_stats,
        "Execution summary should include quantitative stats. Got: {}",
        summary
    );

    Ok(())
}
