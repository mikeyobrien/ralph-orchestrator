//! Integration tests for remote review and rebase loop workflows.

use anyhow::{Context, Result, bail};
use ralph_core::MergeQueue;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .with_context(|| format!("git {}", args.join(" ")))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
}

fn git_succeeds(path: &Path, args: &[&str]) -> Result<bool> {
    let status = Command::new("git")
        .args(args)
        .current_dir(path)
        .status()
        .with_context(|| format!("git {}", args.join(" ")))?;

    Ok(status.success())
}

fn ralph_loops(path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("loops")
        .args(args)
        .current_dir(path)
        .output()
        .with_context(|| format!("ralph loops {}", args.join(" ")))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        bail!(
            "ralph loops {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
}

fn setup_repo_with_remote() -> Result<(TempDir, TempDir)> {
    let remote = TempDir::new()?;
    git(remote.path(), &["init", "--bare", "--initial-branch=main"])?;

    let repo = TempDir::new()?;
    git(repo.path(), &["init", "--initial-branch=main"])?;
    git(repo.path(), &["config", "user.email", "test@example.com"])?;
    git(repo.path(), &["config", "user.name", "Test User"])?;

    fs::write(repo.path().join("README.md"), "# Test\n")?;
    fs::write(repo.path().join(".gitignore"), ".worktrees/\n.ralph/\n")?;
    git(repo.path(), &["add", "README.md", ".gitignore"])?;
    git(repo.path(), &["commit", "-m", "Initial commit"])?;
    git(
        repo.path(),
        &[
            "remote",
            "add",
            "origin",
            &remote.path().display().to_string(),
        ],
    )?;
    git(repo.path(), &["push", "-u", "origin", "main"])?;

    Ok((repo, remote))
}

fn create_loop_worktree(repo: &Path, loop_id: &str) -> Result<std::path::PathBuf> {
    let worktree = repo.join(".worktrees").join(loop_id);
    fs::create_dir_all(repo.join(".worktrees"))?;
    let branch = format!("ralph/{loop_id}");
    git(
        repo,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            &worktree.display().to_string(),
        ],
    )?;
    git(&worktree, &["config", "user.email", "test@example.com"])?;
    git(&worktree, &["config", "user.name", "Test User"])?;
    Ok(worktree)
}

#[test]
fn publish_review_pushes_loop_branch_and_writes_summary() -> Result<()> {
    let (repo, remote) = setup_repo_with_remote()?;
    let loop_id = "review-loop-001";
    let worktree = create_loop_worktree(repo.path(), loop_id)?;

    fs::write(worktree.join("feature.txt"), "ready for review\n")?;
    fs::create_dir_all(worktree.join(".ralph/agent"))?;
    fs::write(
        worktree.join(".ralph/agent/handoff.md"),
        "# Session Handoff\n",
    )?;
    git(&worktree, &["add", "feature.txt"])?;
    git(&worktree, &["commit", "-m", "Add review feature"])?;

    let summary_path = repo.path().join(".ralph/reviews/review-loop-001.md");
    let summary_arg = summary_path.display().to_string();

    let stdout = ralph_loops(
        repo.path(),
        &[
            "publish-review",
            loop_id,
            "--remote",
            "origin",
            "--base",
            "origin/main",
            "--summary",
            &summary_arg,
        ],
    )?;

    assert!(
        stdout.contains("Published loop 'review-loop-001'"),
        "unexpected stdout: {stdout}"
    );

    let remote_refs = git(
        remote.path(),
        &["show-ref", "--verify", "refs/heads/ralph/review-loop-001"],
    )?;
    assert!(
        remote_refs.contains("refs/heads/ralph/review-loop-001"),
        "remote branch was not pushed: {remote_refs}"
    );

    let summary = fs::read_to_string(summary_path)?;
    assert!(summary.contains("# Remote Review: review-loop-001"));
    assert!(summary.contains("origin/ralph/review-loop-001"));
    assert!(summary.contains("Add review feature"));
    assert!(summary.contains("feature.txt"));
    assert!(summary.contains("handoff.md"));

    Ok(())
}

#[test]
fn rebase_loop_updates_branch_without_merging_to_main() -> Result<()> {
    let (repo, _remote) = setup_repo_with_remote()?;
    let loop_id = "rebase-loop-001";
    let worktree = create_loop_worktree(repo.path(), loop_id)?;

    fs::write(worktree.join("loop.txt"), "loop change\n")?;
    git(&worktree, &["add", "loop.txt"])?;
    git(&worktree, &["commit", "-m", "Add loop change"])?;

    fs::write(repo.path().join("base.txt"), "base change\n")?;
    git(repo.path(), &["add", "base.txt"])?;
    git(repo.path(), &["commit", "-m", "Advance base"])?;

    let stdout = ralph_loops(
        repo.path(),
        &["rebase", loop_id, "--base", "main", "--no-fetch"],
    )?;

    assert!(
        stdout.contains("Rebased 1 loop branch(es) onto main."),
        "unexpected stdout: {stdout}"
    );

    git(
        repo.path(),
        &[
            "merge-base",
            "--is-ancestor",
            "main",
            "ralph/rebase-loop-001",
        ],
    )?;
    assert!(
        !repo.path().join("loop.txt").exists(),
        "rebase must not merge loop changes into main"
    );

    Ok(())
}

#[test]
fn rebase_without_loop_id_rebases_queued_worktree_branch() -> Result<()> {
    let (repo, _remote) = setup_repo_with_remote()?;
    let loop_id = "queued-rebase-loop-001";
    let worktree = create_loop_worktree(repo.path(), loop_id)?;

    fs::write(worktree.join("queued.txt"), "queued loop change\n")?;
    git(&worktree, &["add", "queued.txt"])?;
    git(&worktree, &["commit", "-m", "Add queued loop change"])?;

    let queue = MergeQueue::new(repo.path());
    queue.enqueue(loop_id, "queued prompt")?;

    fs::write(repo.path().join("base-queued.txt"), "base change\n")?;
    git(repo.path(), &["add", "base-queued.txt"])?;
    git(repo.path(), &["commit", "-m", "Advance queued base"])?;

    let stdout = ralph_loops(repo.path(), &["rebase", "--base", "main", "--no-fetch"])?;

    assert!(
        stdout.contains("Rebased 1 loop branch(es) onto main."),
        "unexpected stdout: {stdout}"
    );

    git(
        repo.path(),
        &[
            "merge-base",
            "--is-ancestor",
            "main",
            "ralph/queued-rebase-loop-001",
        ],
    )?;
    assert!(
        !repo.path().join("queued.txt").exists(),
        "bulk rebase must not merge queued loop changes into main"
    );

    Ok(())
}

#[test]
fn rebase_push_updates_custom_remote_review_branch() -> Result<()> {
    let (repo, remote) = setup_repo_with_remote()?;
    let loop_id = "custom-review-loop-001";
    let branch = format!("ralph/{loop_id}");
    let remote_branch = "reviews/custom-review-loop-001";
    let worktree = create_loop_worktree(repo.path(), loop_id)?;

    fs::write(worktree.join("custom-review.txt"), "custom review\n")?;
    git(&worktree, &["add", "custom-review.txt"])?;
    git(&worktree, &["commit", "-m", "Add custom review"])?;

    let summary_path = repo.path().join(".ralph/reviews/custom-review-loop-001.md");
    let summary_arg = summary_path.display().to_string();
    ralph_loops(
        repo.path(),
        &[
            "publish-review",
            loop_id,
            "--remote",
            "origin",
            "--remote-branch",
            remote_branch,
            "--base",
            "origin/main",
            "--summary",
            &summary_arg,
        ],
    )?;
    let initial_remote_head = git(
        remote.path(),
        &["rev-parse", "refs/heads/reviews/custom-review-loop-001"],
    )?;

    fs::write(repo.path().join("base-custom-review.txt"), "base change\n")?;
    git(repo.path(), &["add", "base-custom-review.txt"])?;
    git(repo.path(), &["commit", "-m", "Advance custom-review base"])?;

    let stdout = ralph_loops(
        repo.path(),
        &[
            "rebase",
            loop_id,
            "--base",
            "main",
            "--no-fetch",
            "--push",
            "--remote",
            "origin",
        ],
    )?;

    assert!(
        stdout.contains("Rebased 1 loop branch(es) onto main."),
        "unexpected stdout: {stdout}"
    );
    let local_head = git(repo.path(), &["rev-parse", &branch])?;
    let custom_remote_head = git(
        remote.path(),
        &["rev-parse", "refs/heads/reviews/custom-review-loop-001"],
    )?;
    assert_eq!(custom_remote_head.trim(), local_head.trim());
    assert_ne!(custom_remote_head.trim(), initial_remote_head.trim());
    assert!(
        !git_succeeds(
            remote.path(),
            &[
                "show-ref",
                "--verify",
                "refs/heads/ralph/custom-review-loop-001",
            ],
        )?,
        "rebase --push must not create the default remote branch when a custom review branch is configured"
    );

    Ok(())
}

#[test]
fn rebase_branch_without_worktree_preserves_current_checkout() -> Result<()> {
    let (repo, _remote) = setup_repo_with_remote()?;
    let loop_id = "branch-only-rebase-loop-001";
    let branch = format!("ralph/{loop_id}");

    git(repo.path(), &["checkout", "-b", &branch])?;
    fs::write(
        repo.path().join("branch-only.txt"),
        "branch-only loop change\n",
    )?;
    git(repo.path(), &["add", "branch-only.txt"])?;
    git(
        repo.path(),
        &["commit", "-m", "Add branch-only loop change"],
    )?;
    git(repo.path(), &["checkout", "main"])?;

    let queue = MergeQueue::new(repo.path());
    queue.enqueue(loop_id, "branch-only prompt")?;

    fs::write(repo.path().join("base-branch-only.txt"), "base change\n")?;
    git(repo.path(), &["add", "base-branch-only.txt"])?;
    git(repo.path(), &["commit", "-m", "Advance branch-only base"])?;

    let stdout = ralph_loops(
        repo.path(),
        &["rebase", loop_id, "--base", "main", "--no-fetch"],
    )?;

    assert!(
        stdout.contains("Rebased 1 loop branch(es) onto main."),
        "unexpected stdout: {stdout}"
    );
    git(
        repo.path(),
        &["merge-base", "--is-ancestor", "main", &branch],
    )?;
    assert_eq!(
        git(repo.path(), &["branch", "--show-current"])?.trim(),
        "main"
    );
    assert!(
        !repo.path().join("branch-only.txt").exists(),
        "branch-only rebase must not switch or merge into main"
    );

    Ok(())
}

#[test]
fn rebase_branch_without_worktree_does_not_suffix_match_another_worktree() -> Result<()> {
    let (repo, _remote) = setup_repo_with_remote()?;

    let branch_only_loop = "loop";
    let branch_only_branch = format!("ralph/{branch_only_loop}");
    git(repo.path(), &["checkout", "-b", &branch_only_branch])?;
    fs::write(repo.path().join("branch-only-loop.txt"), "branch loop\n")?;
    git(repo.path(), &["add", "branch-only-loop.txt"])?;
    git(repo.path(), &["commit", "-m", "Add branch-only loop"])?;
    git(repo.path(), &["checkout", "main"])?;

    let other_loop = "other-loop";
    let other_worktree = create_loop_worktree(repo.path(), other_loop)?;
    fs::write(other_worktree.join("other-loop.txt"), "other loop\n")?;
    git(&other_worktree, &["add", "other-loop.txt"])?;
    git(&other_worktree, &["commit", "-m", "Add other loop"])?;

    let queue = MergeQueue::new(repo.path());
    queue.enqueue(branch_only_loop, "branch-only prompt")?;

    fs::write(repo.path().join("base-suffix.txt"), "base change\n")?;
    git(repo.path(), &["add", "base-suffix.txt"])?;
    git(repo.path(), &["commit", "-m", "Advance suffix base"])?;

    let stdout = ralph_loops(
        repo.path(),
        &["rebase", branch_only_loop, "--base", "main", "--no-fetch"],
    )?;

    assert!(
        stdout.contains("Rebased 1 loop branch(es) onto main."),
        "unexpected stdout: {stdout}"
    );
    assert!(
        git_succeeds(
            repo.path(),
            &["merge-base", "--is-ancestor", "main", &branch_only_branch],
        )?,
        "requested branch-only loop should be rebased"
    );
    assert!(
        !git_succeeds(
            repo.path(),
            &["merge-base", "--is-ancestor", "main", "ralph/other-loop"],
        )?,
        "suffix-matching worktree branch must not be rebased"
    );
    assert_eq!(
        git(repo.path(), &["branch", "--show-current"])?.trim(),
        "main"
    );

    Ok(())
}

#[test]
fn rebase_explicit_branch_without_queue_does_not_suffix_match_another_worktree() -> Result<()> {
    let (repo, _remote) = setup_repo_with_remote()?;

    let branch_only_loop = "loop";
    let branch_only_branch = format!("ralph/{branch_only_loop}");
    git(repo.path(), &["checkout", "-b", &branch_only_branch])?;
    fs::write(repo.path().join("branch-only-loop.txt"), "branch loop\n")?;
    git(repo.path(), &["add", "branch-only-loop.txt"])?;
    git(repo.path(), &["commit", "-m", "Add branch-only loop"])?;
    git(repo.path(), &["checkout", "main"])?;

    let other_worktree = create_loop_worktree(repo.path(), "other-loop")?;
    fs::write(other_worktree.join("other-loop.txt"), "other loop\n")?;
    git(&other_worktree, &["add", "other-loop.txt"])?;
    git(&other_worktree, &["commit", "-m", "Add other loop"])?;

    fs::write(repo.path().join("base-no-queue.txt"), "base change\n")?;
    git(repo.path(), &["add", "base-no-queue.txt"])?;
    git(repo.path(), &["commit", "-m", "Advance base without queue"])?;

    let stdout = ralph_loops(
        repo.path(),
        &["rebase", branch_only_loop, "--base", "main", "--no-fetch"],
    )?;

    assert!(
        stdout.contains("Rebased 1 loop branch(es) onto main."),
        "unexpected stdout: {stdout}"
    );
    assert!(
        git_succeeds(
            repo.path(),
            &["merge-base", "--is-ancestor", "main", &branch_only_branch],
        )?,
        "requested branch-only loop should be rebased"
    );
    assert!(
        !git_succeeds(
            repo.path(),
            &["merge-base", "--is-ancestor", "main", "ralph/other-loop"],
        )?,
        "suffix-matching worktree branch must not be rebased"
    );

    Ok(())
}

#[test]
fn publish_review_explicit_branch_without_queue_does_not_suffix_match_another_worktree()
-> Result<()> {
    let (repo, remote) = setup_repo_with_remote()?;

    let branch_only_loop = "loop";
    let branch_only_branch = format!("ralph/{branch_only_loop}");
    git(repo.path(), &["checkout", "-b", &branch_only_branch])?;
    fs::write(
        repo.path().join("branch-only-review.txt"),
        "branch review\n",
    )?;
    git(repo.path(), &["add", "branch-only-review.txt"])?;
    git(repo.path(), &["commit", "-m", "Add branch-only review"])?;
    git(repo.path(), &["checkout", "main"])?;

    let other_worktree = create_loop_worktree(repo.path(), "other-loop")?;
    fs::write(other_worktree.join("other-review.txt"), "other review\n")?;
    git(&other_worktree, &["add", "other-review.txt"])?;
    git(&other_worktree, &["commit", "-m", "Add other review"])?;

    let summary_path = repo.path().join(".ralph/reviews/loop.md");
    let summary_arg = summary_path.display().to_string();

    let stdout = ralph_loops(
        repo.path(),
        &[
            "publish-review",
            branch_only_loop,
            "--remote",
            "origin",
            "--base",
            "origin/main",
            "--summary",
            &summary_arg,
        ],
    )?;

    assert!(
        stdout.contains("Published loop 'loop'"),
        "unexpected stdout: {stdout}"
    );
    assert!(
        !stdout.contains("other-loop"),
        "publish-review must not select suffix-matching worktree: {stdout}"
    );

    let remote_refs = git(
        remote.path(),
        &["show-ref", "--verify", "refs/heads/ralph/loop"],
    )?;
    assert!(
        remote_refs.contains("refs/heads/ralph/loop"),
        "requested branch was not pushed: {remote_refs}"
    );
    assert!(
        git(remote.path(), &["show-ref"]).map_or(true, |refs| !refs.contains("other-loop")),
        "suffix-matching branch must not be pushed"
    );

    let summary = fs::read_to_string(summary_path)?;
    assert!(summary.contains("# Remote Review: loop"));
    assert!(summary.contains("branch-only-review.txt"));
    assert!(!summary.contains("other-review.txt"));

    Ok(())
}
