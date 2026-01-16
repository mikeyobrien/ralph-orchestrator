#!/bin/bash
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

echo ""
echo "🎉 Git hooks installed successfully!"
echo ""
echo "The pre-commit hook will now run before each commit to check:"
echo "  • cargo fmt --check (formatting)"
echo "  • cargo clippy (linting)"
