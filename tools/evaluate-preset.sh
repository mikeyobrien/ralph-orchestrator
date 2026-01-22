#!/bin/bash
# evaluate-preset.sh - Evaluate a single hat collection preset
#
# Usage: ./tools/evaluate-preset.sh <preset-name> [backend]
#
# Example:
#   ./tools/evaluate-preset.sh tdd-red-green claude
#   ./tools/evaluate-preset.sh spec-driven kiro

set -euo pipefail

# Resolve project root from script location (works regardless of cwd)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors for output (defined early for use in trap)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Handle Ctrl+C gracefully - kill child processes and exit
cleanup() {
    echo -e "\n${YELLOW}Interrupted - cleaning up...${NC}"
    # Restore original agent state if backup exists
    if [[ -n "${AGENT_BACKUP_DIR:-}" && -d "$AGENT_BACKUP_DIR" ]]; then
        echo -e "${BLUE}Restoring original .agent/ directory...${NC}"
        rm -rf .agent
        cp -r "$AGENT_BACKUP_DIR" .agent
        echo -e "${GREEN}Original .agent/ state restored (backup preserved in $AGENT_BACKUP_DIR)${NC}"
    fi
    # Kill entire process group
    kill 0 2>/dev/null || true
    exit 130
}
trap cleanup SIGINT SIGTERM

PRESET=${1:-}
BACKEND=${2:-claude}
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

if [[ -z "$PRESET" ]]; then
    echo -e "${RED}Error: Preset name required${NC}"
    echo "Usage: $0 <preset-name> [backend]"
    echo ""
    echo "Available presets:"
    ls -1 presets/*.yml | xargs -n1 basename | sed 's/.yml$//'
    exit 1
fi

PRESET_FILE="presets/${PRESET}.yml"
if [[ ! -f "$PRESET_FILE" ]]; then
    echo -e "${RED}Error: Preset file not found: $PRESET_FILE${NC}"
    exit 1
fi

# Setup directories
LOG_DIR=".eval/logs/${PRESET}/${TIMESTAMP}"
SANDBOX_DIR=".eval-sandbox/${PRESET}"
mkdir -p "$LOG_DIR"

# Clean sandbox for fresh evaluation (prevents stale state from previous runs)
rm -rf "$SANDBOX_DIR"
mkdir -p "$SANDBOX_DIR"

# Create 'latest' symlink
ln -sfn "$TIMESTAMP" ".eval/logs/${PRESET}/latest"

echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Preset Evaluation: ${YELLOW}${PRESET}${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "  Backend:   ${GREEN}${BACKEND}${NC}"
echo -e "  Timestamp: ${TIMESTAMP}"
echo -e "  Log dir:   ${LOG_DIR}"
echo -e "  Sandbox:   ${SANDBOX_DIR}"
echo ""

# Load test task from YAML (requires yq)
if command -v yq &> /dev/null; then
    TEST_TASK=$(yq -r ".test_tasks[\"${PRESET}\"]" tools/preset-test-tasks.yml)
    COMPLEXITY=$(yq -r ".complexity[\"${PRESET}\"]" tools/preset-test-tasks.yml)
    TIMEOUT=$(yq -r ".timeouts[\"${COMPLEXITY}\"]" tools/preset-test-tasks.yml)
else
    echo -e "${YELLOW}Warning: yq not found, using default test task${NC}"
    TEST_TASK="Test the ${PRESET} workflow with a simple task."
    TIMEOUT=300
fi

echo -e "${BLUE}Test Task:${NC}"
echo "$TEST_TASK" | sed 's/^/  /'
echo ""
echo -e "${BLUE}Timeout:${NC} ${TIMEOUT}s"
echo ""

# Record environment
cat > "$LOG_DIR/environment.json" << EOF
{
  "preset": "$PRESET",
  "backend": "$BACKEND",
  "timestamp": "$TIMESTAMP",
  "ralph_version": "$(cargo run --bin ralph -- --version 2>/dev/null || echo 'unknown')",
  "backend_version": "$(${BACKEND}-cli --version 2>/dev/null || ${BACKEND} --version 2>/dev/null || echo 'unknown')",
  "os": "$(uname -s)",
  "hostname": "$(hostname)"
}
EOF

# Backup and reset agent state for clean evaluation
AGENT_BACKUP_DIR="$LOG_DIR/agent-backup"
if [[ -d ".agent" ]]; then
    echo -e "${BLUE}Backing up existing .agent/ directory...${NC}"
    cp -r .agent "$AGENT_BACKUP_DIR"
fi

# Create fresh agent state for evaluation
rm -rf .agent
mkdir -p .agent
cat > .agent/scratchpad.md << 'SCRATCHPAD_EOF'
# Scratchpad — Preset Evaluation

## Current Status
**Mode**: Preset Evaluation
**Task**: See prompt below

## Active Task
Follow the instructions in the prompt. This is a fresh evaluation context.
SCRATCHPAD_EOF

# Create .ralph directory for events isolation
mkdir -p .ralph
echo -e "${GREEN}Created fresh .agent/ and .ralph/ state for evaluation${NC}"
echo ""

# Run evaluation
echo -e "${BLUE}Starting evaluation...${NC}"
echo ""

START_TIME=$(date +%s)

# Create temporary merged config with backend settings
TEMP_CONFIG="$LOG_DIR/merged-config.yml"

# Use yq to merge if available, otherwise simple override
if command -v yq &> /dev/null; then
    yq eval-all 'select(fileIndex == 0) * select(fileIndex == 1)' \
        "$PRESET_FILE" - > "$TEMP_CONFIG" << YAML_EOF
cli:
  backend: "$BACKEND"
  prompt_mode: "arg"
  pty_mode: false
  pty_interactive: false
  idle_timeout_secs: 120

adapters:
  kiro:
    timeout: 900
  claude:
    timeout: 900

verbose: false
YAML_EOF
else
    # Fallback: strip cli section from preset and add our own
    grep -v '^\(cli:\|  backend:\|  prompt_mode:\|  pty_mode:\|  pty_interactive:\|  idle_timeout_secs:\)' "$PRESET_FILE" > "$TEMP_CONFIG"
    cat >> "$TEMP_CONFIG" << YAML_EOF

# Evaluation settings (added by evaluate-preset.sh)
cli:
  backend: "$BACKEND"
  prompt_mode: "arg"
  pty_mode: false
  pty_interactive: false
  idle_timeout_secs: 120

adapters:
  kiro:
    timeout: 900
  claude:
    timeout: 900

verbose: false
YAML_EOF
fi

# Run ralph with the merged config
set +e  # Don't exit on error - we want to capture failures
# Use --foreground to allow Ctrl+C to propagate to child processes
timeout --foreground "$TIMEOUT" \
    cargo run --release --bin ralph -- run \
        -c "$TEMP_CONFIG" \
        -p "$TEST_TASK" \
        --record-session "$LOG_DIR/session.jsonl" \
        2>&1 | tee "$LOG_DIR/output.log"

EXIT_CODE=$?
set -e

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Record exit status
echo "$EXIT_CODE" > "$LOG_DIR/exit_code"
echo "$DURATION" > "$LOG_DIR/duration_seconds"

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"

# Summary
if [[ $EXIT_CODE -eq 0 ]]; then
    echo -e "${GREEN}✅ Evaluation completed successfully${NC}"
elif [[ $EXIT_CODE -eq 124 ]]; then
    echo -e "${RED}❌ Evaluation timed out after ${TIMEOUT}s${NC}"
else
    echo -e "${YELLOW}⚠️  Evaluation completed with exit code: ${EXIT_CODE}${NC}"
fi

echo ""
echo -e "  Duration:   ${DURATION}s"
echo -e "  Exit code:  ${EXIT_CODE}"
echo -e "  Logs:       ${LOG_DIR}/"
echo ""

# Generate metrics if session exists
if [[ -f "$LOG_DIR/session.jsonl" ]]; then
    echo -e "${BLUE}Extracting metrics...${NC}"

    # Count iterations from output.log (ITERATION markers are more reliable than session.jsonl)
    if [[ -f "$LOG_DIR/output.log" ]]; then
        ITERATIONS=$(grep -c '^ ITERATION [0-9]\+ │' "$LOG_DIR/output.log" 2>/dev/null || true)
        if [[ -z "$ITERATIONS" ]]; then
            ITERATIONS="0"
        fi
    else
        ITERATIONS="0"
    fi

    # Extract unique hats from output.log iteration markers
    # Pattern: " ITERATION N │ 🎭 hat_name │ ..."
    # We extract the part between the emoji and the next │
    if [[ -f "$LOG_DIR/output.log" ]]; then
        HATS=$(grep '^ ITERATION [0-9]\+ │' "$LOG_DIR/output.log" 2>/dev/null | \
               sed -E 's/.* │ [^[:alnum:]]+ ([a-z_]+) │.*/\1/' | \
               sort -u | tr '\n' ',' | sed 's/,$//' || true)
        # Handle empty result
        if [[ -z "$HATS" ]]; then
            HATS="unknown"
        fi
    else
        HATS="unknown"
    fi

    # Count events published via bus.publish
    EVENTS=$(grep -c '"event":"bus.publish"' "$LOG_DIR/session.jsonl" 2>/dev/null || true)
    if [[ -z "$EVENTS" ]]; then
        EVENTS="0"
    fi

    # Check completion by looking for loop.terminate event
    if grep -q '"topic":"loop.terminate"' "$LOG_DIR/session.jsonl" 2>/dev/null; then
        COMPLETED="true"
    else
        COMPLETED="false"
    fi

    # Escape any quotes in HATS for JSON validity
    HATS_ESCAPED=$(echo "$HATS" | sed 's/"/\\"/g')

    cat > "$LOG_DIR/metrics.json" << EOF
{
  "preset": "$PRESET",
  "backend": "$BACKEND",
  "duration_seconds": $DURATION,
  "exit_code": $EXIT_CODE,
  "iterations": $ITERATIONS,
  "events_published": $EVENTS,
  "hats_activated": "$HATS_ESCAPED",
  "completed": $COMPLETED,
  "timestamp": "$TIMESTAMP"
}
EOF

    echo ""
    echo -e "${BLUE}Metrics:${NC}"
    echo -e "  Iterations: ${ITERATIONS}"
    echo -e "  Hats:       ${HATS}"
    echo -e "  Events:     ${EVENTS}"
    echo -e "  Completed:  ${COMPLETED}"
fi

# Restore original agent state if backup exists
if [[ -d "$AGENT_BACKUP_DIR" ]]; then
    echo -e "${BLUE}Restoring original .agent/ directory...${NC}"
    rm -rf .agent
    cp -r "$AGENT_BACKUP_DIR" .agent
    echo -e "${GREEN}Original .agent/ state restored (backup preserved in $AGENT_BACKUP_DIR)${NC}"
fi

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"

exit $EXIT_CODE
