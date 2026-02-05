#!/bin/bash
# TUI Manual Testing Script
# Run this in a real terminal (not from Claude Code)

set -e

cd "$(dirname "$0")/.."

echo "=== Hats TUI Manual Test ==="
echo ""

# Check Claude authentication
echo "1. Checking Claude authentication..."
if ! claude -p "say hi" --output-format json 2>/dev/null | head -1 | grep -q '"type"'; then
    echo "   ❌ Claude not authenticated. Run: claude /login"
    exit 1
fi
echo "   ✅ Claude authenticated"
echo ""

# Build the project
echo "2. Building Hats..."
cargo build --bin hats --quiet
echo "   ✅ Build complete"
echo ""

# Run TUI test options
echo "3. Choose a test:"
echo "   [1] PTY Demo (no Claude needed) - tests TUI rendering"
echo "   [2] Full Hats Loop - tests complete TUI with Claude"
echo "   [3] Validation Example - renders widgets to files"
echo ""
read -p "Enter choice (1-3): " choice

case $choice in
    1)
        echo ""
        echo "Starting PTY demo..."
        echo "Controls: Ctrl+A then Q to quit"
        echo ""
        cargo run -p hats-tui --example pty_output_demo
        ;;
    2)
        echo ""
        echo "Starting Hats with TUI..."
        echo "Controls: Ctrl+A then ? for help"
        echo ""
        cargo run --bin hats -- run -i -c hats.code-assist.yml \
            -p "Run cargo test -p hats-tui, verify all tests pass, then emit build.done"
        ;;
    3)
        echo ""
        echo "Running validation example..."
        cargo run -p hats-tui --example validate_widgets
        echo ""
        echo "Output saved to tui-validation/"
        ls -la tui-validation/
        ;;
    *)
        echo "Invalid choice"
        exit 1
        ;;
esac

echo ""
echo "=== Test Complete ==="
