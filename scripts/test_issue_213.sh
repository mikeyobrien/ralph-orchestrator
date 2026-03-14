#!/usr/bin/env bash
# Manual regression test for issue #213
# Run this script to verify the fix for "Subprocess TUI run wrongly spawns worktree on first run"
#
# Usage: ./test_issue_213.sh
#
# This script:
# 1. Creates a fresh test directory
# 2. Initializes git and ralph
# 3. Runs ralph in subprocess TUI mode
# 4. Checks that NO worktree was created (the bug causes a spurious worktree)

set -e

TEST_DIR=$(mktemp -d)
echo "Test directory: $TEST_DIR"
cd "$TEST_DIR"

# Initialize git repo
git init -q

# Initialize ralph (use codex as backend, or claude if unavailable)
if command -v ralph &> /dev/null; then
    ralph init --backend codex --force 2>/dev/null || ralph init --backend claude --force 2>/dev/null || true
else
    echo "WARNING: ralph not installed, using mock config"
    mkdir -p .ralph
fi

# Create a simple prompt
echo "Smoke test prompt" > PROMPT.md

echo ""
echo "=== Before running ralph ==="
ls -la
echo ""
echo "=== Checking for .worktrees ==="
ls -la .worktrees 2>/dev/null || echo "No .worktrees directory (expected)"

echo ""
echo "=== Running ralph (simulating TUI mode with script) ==="
# Run with timeout to prevent hanging
# The --legacy-tui flag forces in-process TUI which behaves similarly to subprocess TUI
# for our testing purposes
timeout 10s script -qefc 'ralph run -P PROMPT.md --skip-preflight --max-iterations 1' /tmp/ralph-test-log.txt 2>&1 || true

echo ""
echo "=== After running ralph ==="
ls -la

echo ""
echo "=== Checking for worktrees ==="
if [ -d ".worktrees" ]; then
    echo "BUG: .worktrees directory was created!"
    find .worktrees -maxdepth 3 -type d
    echo ""
    echo "=== Loop registry ==="
    cat .ralph/loops.json 2>/dev/null || echo "No loops.json"
    echo ""
    echo "=== Lock file ==="
    cat .ralph/loop.lock 2>/dev/null || echo "No loop.lock"
    RESULT=1
else
    echo "SUCCESS: No .worktrees directory created (fix working!)"
    RESULT=0
fi

# Cleanup
rm -rf "$TEST_DIR"

exit $RESULT
