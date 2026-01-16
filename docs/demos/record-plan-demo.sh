#!/bin/bash
# Demo script for ralph plan
# This shows the command and captures the startup sequence

set -e
cd "$(dirname "$0")/../.."

# Clean environment
clear
export PS1='$ '
export TERM=xterm-256color

# Simulate typing effect
type_text() {
    for (( i=0; i<${#1}; i++ )); do
        printf '%s' "${1:$i:1}"
        sleep 0.05
    done
}

# Show the prompt
printf '$ '
sleep 0.5

# "Type" the command
type_text "ralph plan \"Build a CLI tool for managing dotfiles\""
sleep 0.3
echo ""

# Run the command with timeout (captures startup, then exits)
# Use 'script' to ensure proper TTY handling
timeout 15 ./target/release/ralph plan "Build a CLI tool for managing dotfiles" 2>&1 || true

echo ""
echo "# Demo: Planning session started! The PDD SOP guides you through"
echo "# requirements gathering, research, and design phases."
