#!/usr/bin/env bash
# Setup git hooks for development
# Run this once after cloning the repository

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
HOOKS_DIR="$REPO_ROOT/.hooks"
GIT_HOOKS_DIR="$REPO_ROOT/.git/hooks"

echo "Setting up git hooks..."

# Ensure .git/hooks directory exists
mkdir -p "$GIT_HOOKS_DIR"

# Install pre-commit hook
if [ -f "$HOOKS_DIR/pre-commit" ]; then
    cp "$HOOKS_DIR/pre-commit" "$GIT_HOOKS_DIR/pre-commit"
    chmod +x "$GIT_HOOKS_DIR/pre-commit"
    echo "✅ Installed pre-commit hook"
else
    echo "❌ No pre-commit hook found in .hooks/"
    exit 1
fi

# Install pre-push hook
if [ -f "$HOOKS_DIR/pre-push" ]; then
    cp "$HOOKS_DIR/pre-push" "$GIT_HOOKS_DIR/pre-push"
    chmod +x "$GIT_HOOKS_DIR/pre-push"
    echo "✅ Installed pre-push hook"
else
    echo "⚠️  No pre-push hook found in .hooks/ (optional)"
fi

echo ""
echo "🎉 Git hooks installed successfully!"
echo ""
echo "The pre-commit hook will run before each commit to check:"
echo "  • ./scripts/ci-rust-gate.sh --skip-hooks-bdd --skip-mock-e2e"
echo ""
echo "The pre-push hook will run before each push to verify:"
echo "  • ./scripts/ci-rust-gate.sh"
echo "    Includes: embedded sync, fmt, clippy, CI test skip list, hooks BDD, mock E2E smoke"
echo ""
echo "Skip either hook with --no-verify when needed."
