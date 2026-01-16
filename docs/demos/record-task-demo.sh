#!/bin/bash
# Demo script for ralph task
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
type_text "ralph task \"Add user authentication with JWT tokens\""
sleep 0.3
echo ""

# Run the command with timeout (captures startup, then exits)
timeout 15 ./target/release/ralph task "Add user authentication with JWT tokens" 2>&1 || true

echo ""
echo "# Demo: Task generation started! Creates structured .code-task.md files"
echo "# with acceptance criteria and implementation guidance."
