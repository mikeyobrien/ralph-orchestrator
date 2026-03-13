#!/usr/bin/env bash
# evaluate-all-presets.sh - Evaluate all hat collection presets
#
# Usage: ./tools/evaluate-all-presets.sh [backend]
#
# Example:
#   ./tools/evaluate-all-presets.sh claude
#   ./tools/evaluate-all-presets.sh kiro

set -euo pipefail

# Resolve project root from script location (works regardless of cwd)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors (defined early for use in trap)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Handle Ctrl+C gracefully - kill child processes and exit
cleanup() {
    echo -e "\n${YELLOW}Interrupted - cleaning up...${NC}"
    # Kill entire process group
    kill 0 2>/dev/null || true
    exit 130
}
trap cleanup SIGINT SIGTERM

BACKEND=${1:-claude}
MODE=${2:-${RALPH_PRESET_TASK_VARIANT:-full}}
SUITE_ID="$(date +%Y%m%d_%H%M%S)_${MODE}"
RESULTS_DIR=".eval/results/${SUITE_ID}"
mkdir -p "$RESULTS_DIR"

# Core presets to evaluate (hatless-baseline runs first as control)
PRESETS="hatless-baseline code-assist debug research review pdd-to-code-assist"

TOTAL=6
PASSED=0
FAILED=0
PARTIAL=0

# Results file (portable alternative to associative array)
RESULTS_FILE="$RESULTS_DIR/.results.tmp"
> "$RESULTS_FILE"

echo -e "${CYAN}"
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║                                                               ║"
echo "║        🎩  Core Hat Collection Evaluation Suite  🎩         ║"
echo "║                                                               ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo ""
echo -e "  Backend:     ${GREEN}${BACKEND}${NC}"
echo -e "  Mode:        ${GREEN}${MODE}${NC}"
echo -e "  Suite ID:    ${SUITE_ID}"
echo -e "  Presets:     ${TOTAL}"
echo -e "  Results:     ${RESULTS_DIR}/"
echo ""
echo -e "${BLUE}Starting evaluation...${NC}"
echo ""

START_TIME=$(date +%s)

num=0
for preset in $PRESETS; do
    num=$((num + 1))

    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  [${num}/${TOTAL}] Evaluating: ${YELLOW}${preset}${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""

    # Run evaluation
    set +e
    ./tools/evaluate-preset.sh "$preset" "$BACKEND" "$MODE"
    EXIT_CODE=$?
    set -e

    # Copy metrics to results
    if [[ -f ".eval/logs/${preset}/latest/metrics.json" ]]; then
        cp ".eval/logs/${preset}/latest/metrics.json" "$RESULTS_DIR/${preset}.json"
    fi

    PROMISE_REACHED="false"
    SESSION_RECORDING_EMPTY="false"
    if [[ -f "$RESULTS_DIR/${preset}.json" ]]; then
        PROMISE_REACHED=$(sed -n 's/.*"completion_promise_reached": \(true\|false\).*/\1/p' "$RESULTS_DIR/${preset}.json" | head -n1)
        if [[ -z "$PROMISE_REACHED" ]]; then
            PROMISE_REACHED="false"
        fi
        SESSION_RECORDING_EMPTY=$(sed -n 's/.*"session_recording_empty": \(true\|false\).*/\1/p' "$RESULTS_DIR/${preset}.json" | head -n1)
        if [[ -z "$SESSION_RECORDING_EMPTY" ]]; then
            SESSION_RECORDING_EMPTY="false"
        fi
    fi

    # Track result
    if [[ $EXIT_CODE -eq 0 && "$SESSION_RECORDING_EMPTY" != "true" ]]; then
        echo "${preset}|✅ PASS" >> "$RESULTS_FILE"
        PASSED=$((PASSED + 1))
    elif [[ $EXIT_CODE -eq 0 ]]; then
        echo "${preset}|⚠️ PARTIAL" >> "$RESULTS_FILE"
        PARTIAL=$((PARTIAL + 1))
    elif [[ $EXIT_CODE -eq 124 ]]; then
        echo "${preset}|⏱️ TIMEOUT" >> "$RESULTS_FILE"
        FAILED=$((FAILED + 1))
    else
        # Check if the preset emitted its completion promise but failed to exit cleanly
        if [[ "$PROMISE_REACHED" == "true" ]]; then
            echo "${preset}|⚠️ PARTIAL" >> "$RESULTS_FILE"
            PARTIAL=$((PARTIAL + 1))
        else
            echo "${preset}|❌ FAIL" >> "$RESULTS_FILE"
            FAILED=$((FAILED + 1))
        fi
    fi

    echo ""
done

END_TIME=$(date +%s)
TOTAL_DURATION=$((END_TIME - START_TIME))

echo ""
echo -e "${CYAN}"
echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║                     EVALUATION SUMMARY                        ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo ""

# Print results table
echo -e "${BLUE}Results by Preset:${NC}"
echo ""
printf "  %-30s %s\n" "PRESET" "STATUS"
printf "  %-30s %s\n" "──────────────────────────────" "────────"
while IFS='|' read -r preset status; do
    printf "  %-30s %s\n" "$preset" "$status"
done < "$RESULTS_FILE"

echo ""
echo -e "${BLUE}Summary:${NC}"
echo ""
echo -e "  ${GREEN}✅ Passed:${NC}   ${PASSED}/${TOTAL}"
echo -e "  ${YELLOW}⚠️  Partial:${NC} ${PARTIAL}/${TOTAL}"
echo -e "  ${RED}❌ Failed:${NC}   ${FAILED}/${TOTAL}"
echo ""
echo -e "  Total Duration: ${TOTAL_DURATION}s"
echo ""

# Generate summary report
SUMMARY_FILE="$RESULTS_DIR/SUMMARY.md"
cat > "$SUMMARY_FILE" << EOF
# Preset Evaluation Summary

**Date**: $(date -Iseconds 2>/dev/null || date)
**Backend**: ${BACKEND}
**Mode**: ${MODE}
**Suite ID**: ${SUITE_ID}

## Results

| Status | Count | Percentage |
|--------|-------|------------|
| ✅ Pass | ${PASSED} | $((PASSED * 100 / TOTAL))% |
| ⚠️ Partial | ${PARTIAL} | $((PARTIAL * 100 / TOTAL))% |
| ❌ Fail | ${FAILED} | $((FAILED * 100 / TOTAL))% |

## By Preset

| Preset | Status | Duration | Iterations |
|--------|--------|----------|------------|
EOF

while IFS='|' read -r preset status; do
    metrics_file="$RESULTS_DIR/${preset}.json"
    if [[ -f "$metrics_file" ]]; then
        duration=$(cat "$metrics_file" | grep duration_seconds | sed 's/.*: *\([0-9]*\).*/\1/' || echo "N/A")
        iterations=$(cat "$metrics_file" | grep '"iterations"' | sed 's/.*: *\([0-9]*\).*/\1/' || echo "N/A")
    else
        duration="N/A"
        iterations="N/A"
    fi
    echo "| ${preset} | ${status} | ${duration}s | ${iterations} |" >> "$SUMMARY_FILE"
done < "$RESULTS_FILE"

cat >> "$SUMMARY_FILE" << EOF

## Total Duration

${TOTAL_DURATION}s ($((TOTAL_DURATION / 60)) minutes)

## Next Steps

1. Review failed presets in \`.eval/logs/<preset>/latest/output.log\`
2. Update findings in \`tools/preset-evaluation-findings.md\`
3. Create issues for bugs found
EOF

# Cleanup temp file
rm -f "$RESULTS_FILE"

echo -e "${GREEN}Summary written to: ${SUMMARY_FILE}${NC}"
echo ""

# Create latest symlink
ln -sfn "$SUITE_ID" ".eval/results/latest"

# Exit with failure if any presets failed
if [[ $FAILED -gt 0 ]]; then
    exit 1
fi
