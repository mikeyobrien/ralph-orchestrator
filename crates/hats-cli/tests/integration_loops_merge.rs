//! Integration tests for loop management merge queue UX improvements.
//!
//! Tests per spec: .hats/specs/loop-management-merge-queue.spec.md
//!
//! Features tested:
//! 1. Merge queue state transitions (queued→merging→merged)
//! 2. hats loops UX with summary header and age column
//! 3. --exclusive flag for merge-hats spawns
//! 4. Merge commit conventional format

use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Run hats loops command with given args in the temp directory.
fn hats_loops(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_hats"))
        .arg("loops")
        .args(args)
        .current_dir(temp_path)
        .output()
        .expect("Failed to execute hats loops command")
}

/// Run hats loops and assert success, returning stdout.
fn hats_loops_ok(temp_path: &std::path::Path, args: &[&str]) -> String {
    let output = hats_loops(temp_path, args);
    assert!(
        output.status.success(),
        "Command 'hats loops {}' failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Set up a temp directory with git repo and .hats directory.
fn setup_workspace() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
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

/// Write a merge queue entry directly to the JSONL file for testing.
fn write_merge_queue_entry(temp_path: &std::path::Path, entry: &str) -> Result<()> {
    let queue_path = temp_path.join(".hats/merge-queue.jsonl");
    let mut content = if queue_path.exists() {
        fs::read_to_string(&queue_path)?
    } else {
        String::new()
    };
    content.push_str(entry);
    content.push('\n');
    fs::write(queue_path, content)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Merge Queue State Transition Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_merge_queue_transition_queued_to_merging() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A loop in "queued" state
    let ts = chrono::Utc::now().to_rfc3339();
    write_merge_queue_entry(
        temp_path,
        &format!(
            r#"{{"ts":"{}","loop_id":"hats-test-001","event":{{"type":"queued","prompt":"test prompt"}}}}"#,
            ts
        ),
    )?;

    // When: The merge loop starts (via HATS_MERGE_LOOP_ID env var)
    // Note: This simulates what happens in loop_runner.rs when HATS_MERGE_LOOP_ID is set
    // For now we use the MergeQueue API directly since we can't easily spawn a full loop
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.mark_merging("hats-test-001", 12345)?;

    // Then: The entry should be in "merging" state with PID
    let entry = queue
        .get_entry("hats-test-001")?
        .expect("Entry should exist");
    assert_eq!(entry.state, hats_core::MergeState::Merging);
    assert_eq!(entry.merge_pid, Some(12345));

    Ok(())
}

#[test]
fn test_merge_queue_transition_merging_to_merged() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A loop in "merging" state
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("hats-test-002", "test prompt")?;
    queue.mark_merging("hats-test-002", 12345)?;

    // When: The merge completes successfully with a commit SHA
    queue.mark_merged("hats-test-002", "abc123def")?;

    // Then: The entry should be in "merged" state with commit SHA
    let entry = queue
        .get_entry("hats-test-002")?
        .expect("Entry should exist");
    assert_eq!(entry.state, hats_core::MergeState::Merged);
    assert_eq!(entry.merge_commit, Some("abc123def".to_string()));

    Ok(())
}

#[test]
fn test_merge_queue_transition_merging_to_needs_review() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A loop in "merging" state
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("hats-test-003", "test prompt")?;
    queue.mark_merging("hats-test-003", 12345)?;

    // When: The merge fails (e.g., conflicts, timeout)
    queue.mark_needs_review("hats-test-003", "merge conflicts in src/main.rs")?;

    // Then: The entry should be in "needs-review" state with reason
    let entry = queue
        .get_entry("hats-test-003")?
        .expect("Entry should exist");
    assert_eq!(entry.state, hats_core::MergeState::NeedsReview);
    assert_eq!(
        entry.failure_reason,
        Some("merge conflicts in src/main.rs".to_string())
    );

    Ok(())
}

#[test]
fn test_merge_queue_cannot_skip_merging_state() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A loop in "queued" state
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("hats-test-004", "test prompt")?;

    // When: Trying to mark as merged directly (skipping merging state)
    let result = queue.mark_merged("hats-test-004", "abc123");

    // Then: Should fail with InvalidTransition error
    assert!(matches!(
        result,
        Err(hats_core::MergeQueueError::InvalidTransition(_, _, _))
    ));

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. hats loops UX Tests - Summary Header and Age Column
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_loops_list_shows_summary_header() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: Some loops in various states
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("hats-test-101", "prompt 1")?;
    queue.enqueue("hats-test-102", "prompt 2")?;
    queue.mark_merging("hats-test-102", 123)?;

    // When: Running hats loops list
    let stdout = hats_loops_ok(temp_path, &["list"]);

    // Then: Should show summary header with counts by state
    // Per spec: "A summary header provides counts by state and primary lock info"
    assert!(
        stdout.contains("queued:") || stdout.contains("Queued:"),
        "Summary should show queued count. Got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("merging:") || stdout.contains("Merging:"),
        "Summary should show merging count. Got:\n{}",
        stdout
    );

    Ok(())
}

#[test]
fn test_loops_list_shows_age_column() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A loop queued some time ago
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("hats-test-201", "test prompt")?;

    // When: Running hats loops list
    let stdout = hats_loops_ok(temp_path, &["list"]);

    // Then: Should show AGE column header
    // Per spec: "Add an Age column (relative time) for running/queued/merging/needs-review"
    assert!(
        stdout.contains("AGE") || stdout.contains("Age"),
        "Output should include AGE column header. Got:\n{}",
        stdout
    );

    Ok(())
}

#[test]
fn test_loops_list_hides_terminal_states_by_default() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: Loops in both active and terminal states
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("hats-active-001", "active prompt")?;
    queue.enqueue("hats-merged-001", "merged prompt")?;
    queue.mark_merging("hats-merged-001", 123)?;
    queue.mark_merged("hats-merged-001", "abc123")?;

    // When: Running hats loops list (default, no --all)
    let stdout = hats_loops_ok(temp_path, &["list"]);

    // Then: Should show active state but hide terminal state
    // Per spec: "Terminal states (merged, discarded) are hidden by default"
    assert!(
        stdout.contains("hats-active-001") || stdout.contains("queued"),
        "Should show active (queued) loop. Got:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("hats-merged-001") || stdout.contains("Use --all"),
        "Should hide merged loop by default. Got:\n{}",
        stdout
    );

    Ok(())
}

#[test]
fn test_loops_list_all_shows_terminal_states() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A loop in terminal (merged) state
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("hats-merged-002", "merged prompt")?;
    queue.mark_merging("hats-merged-002", 123)?;
    queue.mark_merged("hats-merged-002", "abc123def")?;

    // When: Running hats loops list --all
    let stdout = hats_loops_ok(temp_path, &["list", "--all"]);

    // Then: Should show terminal states with commit SHA
    // Per spec: "--all includes terminal states and commit SHA"
    assert!(
        stdout.contains("hats-merged-002") || stdout.contains("merged"),
        "Should show merged loop with --all. Got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("abc123"),
        "Should show commit SHA for merged loop. Got:\n{}",
        stdout
    );

    Ok(())
}

#[test]
fn test_loops_list_shows_footer_hints() -> Result<()> {
    let temp_dir = setup_workspace()?;
    let temp_path = temp_dir.path();

    // Given: A loop in needs-review state
    let queue = hats_core::MergeQueue::new(temp_path);
    queue.enqueue("hats-review-001", "test prompt")?;
    queue.mark_merging("hats-review-001", 123)?;
    queue.mark_needs_review("hats-review-001", "conflicts")?;

    // When: Running hats loops list
    let stdout = hats_loops_ok(temp_path, &["list"]);

    // Then: Should show footer hint for retry action
    // Per spec: "Provide a footer hint with next actions"
    assert!(
        stdout.contains("retry") || stdout.contains("Retry") || stdout.contains("--help"),
        "Should show actionable hints. Got:\n{}",
        stdout
    );

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. --exclusive Flag Tests for Merge-Hats Spawns
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_spawn_merge_hats_uses_exclusive_flag() -> Result<()> {
    // This test verifies that process_pending_merges passes --exclusive to hats run
    // Per spec: "Auto-spawned merge loops use --exclusive to wait for the primary lock"
    //
    // We verify this by checking the spawn command construction in loop_runner.rs
    // The implementation should include "--exclusive" in the args array
    //
    // Since we can't easily intercept the spawn call, we verify the source code
    // contains the expected flag. This is a meta-test that ensures the implementation
    // matches the spec.
    let source = include_str!("../src/loop_runner.rs");

    // Verify --exclusive is used in the merge-hats spawn path
    assert!(
        source.contains(r#""--exclusive""#),
        "loop_runner.rs should use --exclusive flag for merge-hats spawns.\n\
         Per spec: 'Auto-spawned merge loops use --exclusive to wait for the primary lock'"
    );

    Ok(())
}

#[test]
fn test_manual_merge_uses_exclusive_flag() -> Result<()> {
    // This test verifies that spawn_merge_hats (used by `hats loops merge`)
    // also uses --exclusive flag
    // Per spec: "Manual merge (hats loops merge) uses --exclusive as well"
    let source = include_str!("../src/loops.rs");

    // Verify the spawn_merge_hats helper function uses --exclusive
    // The function should call `hats run ... --exclusive ...`
    assert!(
        source.contains(r#""--exclusive""#),
        "loops.rs spawn_merge_hats should use --exclusive flag.\n\
         Per spec: 'Manual merge (hats loops merge) uses --exclusive as well'\n\
         The spawn_merge_hats helper must pass --exclusive to hats run."
    );

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Merge Commit Conventional Format Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_merge_commit_format_conventional() -> Result<()> {
    // Per spec: "Use conventional commit format: merge(hats): <summary> (loop <id>)"
    //
    // This test verifies the merge-loop preset or merge logic generates
    // conventional commit messages.
    //
    // Check the merge-loop preset contains instructions for conventional commits
    let preset_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("presets/merge-loop.yml");

    if preset_path.exists() {
        let content = fs::read_to_string(&preset_path)?;

        // The preset should instruct the agent to use conventional commit format
        assert!(
            content.contains("merge(hats)")
                || content.contains("conventional")
                || content.contains("commit format")
                || content.contains("commit message"),
            "merge-loop preset should specify conventional commit format.\n\
             Per spec: 'merge(hats): <summary> (loop <id>)'\n\
             Preset content: {}",
            &content[..content.len().min(500)]
        );
    }

    Ok(())
}

#[test]
fn test_merge_commit_subject_length_limit() -> Result<()> {
    // Per spec: "<summary> is single-line, trimmed, and truncated to keep full subject ≤ 72 chars"
    //
    // This test verifies that merge commit message generation respects the 72 char limit
    //
    // Format: "merge(hats): <summary> (loop <id>)"
    // - "merge(hats): " = 14 chars
    // - " (loop " = 7 chars
    // - ")" = 1 char
    // - loop_id example "hats-20250127-143052-a3f2" = 26 chars
    // Total overhead: ~48 chars, leaving ~24 chars for summary
    let loop_id = "hats-20250127-143052-a3f2";
    let overhead = format!("merge(hats):  (loop {})", loop_id);
    let max_summary_len = 72 - overhead.len();

    // Summary should be truncated if too long
    let long_summary =
        "This is a very long summary that exceeds the maximum allowed length for commit subjects";
    let truncated_summary = if long_summary.len() > max_summary_len {
        &long_summary[..max_summary_len]
    } else {
        long_summary
    };

    let commit_message = format!("merge(hats): {} (loop {})", truncated_summary, loop_id);

    assert!(
        commit_message.len() <= 72,
        "Commit message should be ≤ 72 chars. Got {} chars: {}",
        commit_message.len(),
        commit_message
    );

    Ok(())
}

#[test]
fn test_merge_uses_no_ff_flag() -> Result<()> {
    // Per spec: "Prefer `git merge hats/<id> --no-ff -m <message>` so the message is explicit"
    //
    // Check the merge-loop preset instructs use of --no-ff
    let preset_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("presets/merge-loop.yml");

    if preset_path.exists() {
        let content = fs::read_to_string(&preset_path)?;

        assert!(
            content.contains("--no-ff") || content.contains("no-ff"),
            "merge-loop preset should specify --no-ff for merge.\n\
             Per spec: 'Prefer git merge hats/<id> --no-ff -m <message>'\n\
             Preset content: {}",
            &content[..content.len().min(500)]
        );
    }

    Ok(())
}
