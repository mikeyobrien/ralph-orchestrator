#!/usr/bin/env bash
# evaluate-preset.sh - Evaluate a single hat collection preset
#
# Usage: ./tools/evaluate-preset.sh <preset-name> [backend]
#
# Example:
#   ./tools/evaluate-preset.sh tdd-red-green claude
#   ./tools/evaluate-preset.sh spec-driven kiro
#
# Optional:
#   RALPH_EVAL_BINARY=/abs/path/to/ralph ./tools/evaluate-preset.sh code-assist claude smoke

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
MODE=${3:-${RALPH_PRESET_TASK_VARIANT:-full}}
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
TASK_FILE="tools/preset-test-tasks.yml"

load_yaml_block() {
    local section=$1
    local key=$2
    awk -v section="$section" -v key="$key" '
        $0 == section ":" { in_section = 1; next }
        in_section && $0 ~ "^[^ ]" { in_section = 0 }
        in_section && $0 == "  " key ": |" { in_block = 1; next }
        in_block {
            if ($0 ~ "^  [^ ]") { exit }
            if ($0 ~ /^    /) {
                sub(/^    /, "", $0)
                print
                next
            }
            if ($0 == "") {
                print ""
                next
            }
            exit
        }
    ' "$TASK_FILE"
}

load_yaml_scalar() {
    local section=$1
    local key=$2
    awk -v section="$section" -v key="$key" '
        function trim(s) {
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
            gsub(/^"/, "", s)
            gsub(/"$/, "", s)
            return s
        }
        $0 == section ":" { in_section = 1; next }
        in_section && $0 ~ "^[^ ]" { in_section = 0 }
        in_section && match($0, /^  ([^:]+):[[:space:]]*(.*)$/, m) {
            if (m[1] == key) {
                print trim(m[2])
                exit
            }
        }
    ' "$TASK_FILE"
}

count_iterations_from_output() {
    local output_log=$1
    if [[ ! -f "$output_log" ]]; then
        echo "0"
        return
    fi

    local iterations
    iterations=$(grep -Ec '^ ITERATION [0-9]+ [|│]' "$output_log" 2>/dev/null || true)
    if [[ -z "$iterations" ]]; then
        iterations="0"
    fi
    echo "$iterations"
}

extract_hats_from_output() {
    local output_log=$1
    if [[ ! -f "$output_log" ]]; then
        echo "unknown"
        return
    fi

    local hats
    hats=$(grep -E '^ ITERATION [0-9]+ [|│]' "$output_log" 2>/dev/null | \
        sed -E 's/^.*[|│][[:space:]]*[^[:alnum:]_-]*[[:space:]]*([[:alnum:]_-]+)[[:space:]]*[|│].*$/\1/' | \
        sort -u | tr '\n' ',' | sed 's/,$//' || true)
    if [[ -z "$hats" ]]; then
        hats="unknown"
    fi
    echo "$hats"
}

session_recording_empty() {
    local session_file=$1
    if [[ ! -s "$session_file" ]]; then
        echo "true"
    else
        echo "false"
    fi
}

termination_source() {
    local session_file=$1
    local raw_exit_code=$2

    if [[ -s "$session_file" ]] && grep -q '"topic":"loop.terminate"' "$session_file" 2>/dev/null; then
        echo "session_jsonl"
    elif [[ "$raw_exit_code" -eq 0 ]]; then
        echo "process_exit"
    else
        echo "none"
    fi
}

resolve_ralph_command() {
    if [[ -n "${RALPH_EVAL_BINARY:-}" ]]; then
        if [[ "$RALPH_EVAL_BINARY" == */* ]]; then
            if [[ ! -x "$RALPH_EVAL_BINARY" ]]; then
                echo -e "${RED}Error: RALPH_EVAL_BINARY is not executable: ${RALPH_EVAL_BINARY}${NC}" >&2
                exit 1
            fi
            export PATH="$(dirname "$RALPH_EVAL_BINARY"):$PATH"
            RALPH_CMD=("$RALPH_EVAL_BINARY")
        else
            if ! command -v "$RALPH_EVAL_BINARY" >/dev/null 2>&1; then
                echo -e "${RED}Error: RALPH_EVAL_BINARY not found on PATH: ${RALPH_EVAL_BINARY}${NC}" >&2
                exit 1
            fi
            RALPH_CMD=("$RALPH_EVAL_BINARY")
        fi
    else
        RALPH_CMD=(cargo run --release --bin ralph --)
    fi
}

run_ralph() {
    "${RALPH_CMD[@]}" "$@"
}

run_ralph_with_timeout() {
    local timeout_seconds=$1
    shift
    timeout --foreground "$timeout_seconds" "${RALPH_CMD[@]}" "$@"
}

completion_promise_reached() {
    local session_file=$1
    local output_log=$2
    local completion_promise=$3

    if [[ -z "$completion_promise" ]]; then
        echo "false"
    elif { [[ -s "$session_file" ]] && grep -Fq "\"topic\":\"${completion_promise}\"" "$session_file" 2>/dev/null; } || \
         grep -Fq "Event emitted: ${completion_promise}" "$output_log" 2>/dev/null || \
         grep -Fq "All done! ${completion_promise} detected." "$output_log" 2>/dev/null; then
        echo "true"
    else
        echo "false"
    fi
}

if [[ "${EVALUATE_PRESET_LIB_ONLY:-0}" == "1" ]]; then
    return 0 2>/dev/null || exit 0
fi

resolve_ralph_command

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
echo -e "  Mode:      ${GREEN}${MODE}${NC}"
echo -e "  Timestamp: ${TIMESTAMP}"
echo -e "  Log dir:   ${LOG_DIR}"
echo -e "  Sandbox:   ${SANDBOX_DIR}"
echo ""

# Load test task from YAML (requires yq)
if command -v yq &> /dev/null; then
    if [[ "$MODE" == "smoke" ]]; then
        TEST_TASK=$(yq -r ".smoke_test_tasks[\"${PRESET}\"] // \"\"" "$TASK_FILE")
        TIMEOUT=$(yq -r ".smoke_timeouts[\"${PRESET}\"] // 300" "$TASK_FILE")
    else
        TEST_TASK=$(yq -r ".test_tasks[\"${PRESET}\"] // \"\"" "$TASK_FILE")
        COMPLEXITY=$(yq -r ".complexity[\"${PRESET}\"]" "$TASK_FILE")
        TIMEOUT=$(yq -r ".timeouts[\"${COMPLEXITY}\"]" "$TASK_FILE")
    fi
else
    echo -e "${YELLOW}Warning: yq not found, using shell YAML fallback${NC}"
    if [[ "$MODE" == "smoke" ]]; then
        TEST_TASK=$(load_yaml_block "smoke_test_tasks" "$PRESET")
        TIMEOUT=$(load_yaml_scalar "smoke_timeouts" "$PRESET")
    else
        TEST_TASK=$(load_yaml_block "test_tasks" "$PRESET")
        COMPLEXITY=$(load_yaml_scalar "complexity" "$PRESET")
        TIMEOUT=$(load_yaml_scalar "timeouts" "$COMPLEXITY")
    fi
fi

if [[ -z "$TEST_TASK" ]]; then
    TEST_TASK="Smoke-test the ${PRESET} workflow with a simple task."
fi

if [[ -z "${TIMEOUT:-}" ]]; then
    TIMEOUT=300
fi

if command -v yq &> /dev/null; then
    COMPLETION_PROMISE=$(yq -r '.event_loop.completion_promise // ""' "$PRESET_FILE")
else
    COMPLETION_PROMISE=$(sed -n 's/^[[:space:]]*completion_promise:[[:space:]]*"\{0,1\}\([^"]*\)"\{0,1\}[[:space:]]*$/\1/p' "$PRESET_FILE" | head -n1)
fi

echo -e "${BLUE}Test Task:${NC}"
echo "$TEST_TASK" | sed 's/^/  /'
echo ""
echo -e "${BLUE}Timeout:${NC} ${TIMEOUT}s"
echo -e "${BLUE}Completion Promise:${NC} ${COMPLETION_PROMISE:-unknown}"
echo ""

# Record environment
cat > "$LOG_DIR/environment.json" << EOF
{
  "preset": "$PRESET",
  "backend": "$BACKEND",
  "mode": "$MODE",
  "timestamp": "$TIMESTAMP",
  "ralph_version": "$(run_ralph --version 2>/dev/null || echo 'unknown')",
  "backend_version": "$(${BACKEND}-cli --version 2>/dev/null || ${BACKEND} --version 2>/dev/null || echo 'unknown')",
  "os": "$(uname -s)",
  "hostname": "$(hostname 2>/dev/null || uname -n 2>/dev/null || echo unknown)",
  "completion_promise": "${COMPLETION_PROMISE}"
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
run_ralph_with_timeout "$TIMEOUT" run \
    -c "$TEMP_CONFIG" \
    -p "$TEST_TASK" \
    --record-session "$LOG_DIR/session.jsonl" \
    2>&1 | tee "$LOG_DIR/output.log"

RAW_EXIT_CODE=$?
set -e

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Record exit status
echo "$RAW_EXIT_CODE" > "$LOG_DIR/raw_exit_code"
echo "$DURATION" > "$LOG_DIR/duration_seconds"

SESSION_FILE="$LOG_DIR/session.jsonl"
OUTPUT_LOG="$LOG_DIR/output.log"
ITERATIONS=$(count_iterations_from_output "$OUTPUT_LOG")
HATS=$(extract_hats_from_output "$OUTPUT_LOG")
if [[ -f "$OUTPUT_LOG" ]]; then
    ITERATION_SOURCE="output.log"
else
    ITERATION_SOURCE="none"
fi

SESSION_RECORDING_EMPTY="true"
if [[ -f "$SESSION_FILE" ]]; then
    SESSION_RECORDING_EMPTY=$(session_recording_empty "$SESSION_FILE")
fi

EVENTS="0"
if [[ -s "$SESSION_FILE" ]]; then
    EVENTS=$(grep -c '"event":"bus.publish"' "$SESSION_FILE" 2>/dev/null || true)
    if [[ -z "$EVENTS" ]]; then
        EVENTS="0"
    fi
fi

TERMINATION_SOURCE=$(termination_source "$SESSION_FILE" "$RAW_EXIT_CODE")
COMPLETION_PROMISE_REACHED=$(completion_promise_reached "$SESSION_FILE" "$OUTPUT_LOG" "$COMPLETION_PROMISE")

if [[ "$TERMINATION_SOURCE" != "none" ]]; then
    COMPLETED="true"
else
    COMPLETED="false"
fi

# Escape any quotes in HATS for JSON validity
HATS_ESCAPED=$(echo "$HATS" | sed 's/"/\\"/g')

echo -e "${BLUE}Extracting metrics...${NC}"
echo ""
echo -e "${BLUE}Metrics:${NC}"
echo -e "  Iterations: ${ITERATIONS}"
echo -e "  Hats:       ${HATS}"
echo -e "  Events:     ${EVENTS}"
echo -e "  Promise:    ${COMPLETION_PROMISE:-unknown} (reached: ${COMPLETION_PROMISE_REACHED})"
echo -e "  Completed:  ${COMPLETED}"
echo -e "  Session:    empty=${SESSION_RECORDING_EMPTY}, termination_source=${TERMINATION_SOURCE}"

echo "$RAW_EXIT_CODE" > "$LOG_DIR/exit_code"

cat > "$LOG_DIR/metrics.json" << EOF
{
  "preset": "$PRESET",
  "backend": "$BACKEND",
  "mode": "$MODE",
  "duration_seconds": $DURATION,
  "raw_exit_code": $RAW_EXIT_CODE,
  "iterations": $ITERATIONS,
  "events_published": $EVENTS,
  "hats_activated": "$HATS_ESCAPED",
  "iteration_source": "$ITERATION_SOURCE",
  "session_recording_empty": $SESSION_RECORDING_EMPTY,
  "termination_source": "$TERMINATION_SOURCE",
  "completion_promise": "$COMPLETION_PROMISE",
  "completion_promise_reached": $COMPLETION_PROMISE_REACHED,
  "completed": $COMPLETED,
  "timestamp": "$TIMESTAMP"
}
EOF

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"

# Summary
if [[ $RAW_EXIT_CODE -eq 0 ]]; then
    echo -e "${GREEN}✅ Evaluation completed successfully${NC}"
elif [[ $RAW_EXIT_CODE -eq 124 ]]; then
    echo -e "${RED}❌ Evaluation timed out after ${TIMEOUT}s${NC}"
else
    echo -e "${YELLOW}⚠️  Evaluation completed with exit code: ${RAW_EXIT_CODE}${NC}"
fi

echo ""
echo -e "  Duration:   ${DURATION}s"
echo -e "  Exit code:  ${RAW_EXIT_CODE}"
echo -e "  Logs:       ${LOG_DIR}/"
echo ""

# Restore original agent state if backup exists
if [[ -d "$AGENT_BACKUP_DIR" ]]; then
    echo -e "${BLUE}Restoring original .agent/ directory...${NC}"
    rm -rf .agent
    cp -r "$AGENT_BACKUP_DIR" .agent
    echo -e "${GREEN}Original .agent/ state restored (backup preserved in $AGENT_BACKUP_DIR)${NC}"
fi

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"

exit $RAW_EXIT_CODE
