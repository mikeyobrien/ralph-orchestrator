#!/usr/bin/env bash
# Enforce hooks mutation quality gate for CI.
#
# Gate semantics (Step 12.5):
# - Run cargo-mutants on hooks-critical modules only.
# - Require operational score >= HOOKS_MUTATION_THRESHOLD (caught / (caught + missed)).
# - Hard-fail on any MISS mutant in loop_runner critical ranges:
#   - crates/ralph-cli/src/loop_runner.rs:3467-3560
#   - crates/ralph-cli/src/loop_runner.rs:3623-3635
# - Report TIMEOUT and unviable mutants separately.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
JUSTFILE_PATH="$REPO_ROOT/Justfile"
DEFAULT_THRESHOLD="55"

ARTIFACT_DIR="${HOOKS_MUTATION_ARTIFACT_DIR:-$REPO_ROOT/.artifacts/hooks-mutation}"
MUTANTS_OUT_DIR="$ARTIFACT_DIR/mutants.out"
REPORT_PATH="$ARTIFACT_DIR/hooks-mutation-report.md"
SUMMARY_PATH="$ARTIFACT_DIR/hooks-mutation-summary.json"
SURVIVORS_PATH="$ARTIFACT_DIR/hooks-mutation-survivors.txt"
CRITICAL_MISS_PATH="$ARTIFACT_DIR/critical-miss.txt"
CRITICAL_TIMEOUT_PATH="$ARTIFACT_DIR/critical-timeout.txt"
CRITICAL_UNVIABLE_PATH="$ARTIFACT_DIR/critical-unviable.txt"

read_threshold_from_justfile() {
    if [[ ! -f "$JUSTFILE_PATH" ]]; then
        echo "$DEFAULT_THRESHOLD"
        return
    fi

    local parsed
    parsed="$(awk -F':=' '/^HOOKS_MUTATION_THRESHOLD[[:space:]]*:=[[:space:]]*[0-9]+(\.[0-9]+)?[[:space:]]*$/ {gsub(/[[:space:]]/, "", $2); print $2; exit}' "$JUSTFILE_PATH")"

    if [[ -n "$parsed" ]]; then
        echo "$parsed"
    else
        echo "$DEFAULT_THRESHOLD"
    fi
}

count_lines() {
    local file="$1"
    if [[ -f "$file" ]]; then
        wc -l < "$file" | tr -d '[:space:]'
    else
        echo "0"
    fi
}

filter_critical_lines() {
    local input_file="$1"
    local output_file="$2"

    if [[ ! -f "$input_file" ]]; then
        : > "$output_file"
        return
    fi

    awk -F: '
        $1 == "crates/ralph-cli/src/loop_runner.rs" {
            line = $2 + 0
            if ((line >= 3467 && line <= 3560) || (line >= 3623 && line <= 3635)) {
                print $0
            }
        }
    ' "$input_file" > "$output_file"
}

append_prefixed_lines() {
    local prefix="$1"
    local input_file="$2"
    local output_file="$3"

    if [[ -f "$input_file" ]]; then
        awk -v prefix="$prefix" '{print prefix " " $0}' "$input_file" >> "$output_file"
    fi
}

append_report_section() {
    local title="$1"
    local file_path="$2"
    local max_lines="$3"
    local total

    total="$(count_lines "$file_path")"

    echo "### $title"
    echo "- Count: $total"

    if [[ "$total" -gt 0 ]]; then
        echo '
```text'
        head -n "$max_lines" "$file_path"
        if [[ "$total" -gt "$max_lines" ]]; then
            echo "... ($((total - max_lines)) more lines; see $(basename "$file_path"))"
        fi
        echo '```'
    fi

    echo
}

threshold="${HOOKS_MUTATION_THRESHOLD:-$(read_threshold_from_justfile)}"

if ! [[ "$threshold" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    echo "Invalid HOOKS_MUTATION_THRESHOLD: '$threshold'" >&2
    exit 2
fi

rm -rf "$ARTIFACT_DIR"
mkdir -p "$ARTIFACT_DIR"

mutation_cmd=(
    cargo mutants
    --baseline skip
    --file crates/ralph-core/src/hooks/executor.rs
    --file crates/ralph-core/src/hooks/engine.rs
    --file crates/ralph-core/src/preflight.rs
    --file crates/ralph-cli/src/loop_runner.rs
    -o "$ARTIFACT_DIR"
    --no-times
    --colors never
    --caught
    --unviable
)

set +e
(
    cd "$REPO_ROOT"
    "${mutation_cmd[@]}"
)
mutants_exit=$?
set -e

caught_file="$MUTANTS_OUT_DIR/caught.txt"
missed_file="$MUTANTS_OUT_DIR/missed.txt"
timeout_file="$MUTANTS_OUT_DIR/timeout.txt"
unviable_file="$MUTANTS_OUT_DIR/unviable.txt"

caught_count="$(count_lines "$caught_file")"
missed_count="$(count_lines "$missed_file")"
timeout_count="$(count_lines "$timeout_file")"
unviable_count="$(count_lines "$unviable_file")"

operational_denominator=$((caught_count + missed_count))

operational_score="$(awk -v caught="$caught_count" -v missed="$missed_count" 'BEGIN { if (caught + missed == 0) printf "0.00"; else printf "%.2f", (100 * caught) / (caught + missed) }')"
strict_score="$(awk -v caught="$caught_count" -v missed="$missed_count" -v timeout="$timeout_count" 'BEGIN { if (caught + missed + timeout == 0) printf "0.00"; else printf "%.2f", (100 * caught) / (caught + missed + timeout) }')"

filter_critical_lines "$missed_file" "$CRITICAL_MISS_PATH"
filter_critical_lines "$timeout_file" "$CRITICAL_TIMEOUT_PATH"
filter_critical_lines "$unviable_file" "$CRITICAL_UNVIABLE_PATH"

critical_miss_count="$(count_lines "$CRITICAL_MISS_PATH")"
critical_timeout_count="$(count_lines "$CRITICAL_TIMEOUT_PATH")"
critical_unviable_count="$(count_lines "$CRITICAL_UNVIABLE_PATH")"

: > "$SURVIVORS_PATH"
append_prefixed_lines "MISS" "$missed_file" "$SURVIVORS_PATH"
append_prefixed_lines "TIMEOUT" "$timeout_file" "$SURVIVORS_PATH"

threshold_met="false"
if [[ "$operational_denominator" -gt 0 ]]; then
    if awk -v score="$operational_score" -v minimum="$threshold" 'BEGIN { exit !(score + 0 >= minimum + 0) }'; then
        threshold_met="true"
    fi
fi

status="pass"
fail_reasons=()

if [[ "$mutants_exit" -ne 0 && "$mutants_exit" -ne 3 ]]; then
    fail_reasons+=("cargo mutants exited with code $mutants_exit (expected 0 or 3)")
fi

if [[ "$operational_denominator" -eq 0 ]]; then
    fail_reasons+=("caught + missed == 0; mutation run did not produce usable operational score")
fi

if [[ "$threshold_met" != "true" ]]; then
    fail_reasons+=("operational score ${operational_score}% is below required threshold ${threshold}%")
fi

if [[ "$critical_miss_count" -gt 0 ]]; then
    fail_reasons+=("critical-path MISS survivors detected in loop_runner.rs:3467-3560,3623-3635")
fi

if [[ "${#fail_reasons[@]}" -gt 0 ]]; then
    status="fail"
fi

timestamp_utc="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
mutation_cmd_string="$(printf '%q ' "${mutation_cmd[@]}")"

cat > "$SUMMARY_PATH" <<EOF
{
  "status": "$status",
  "timestamp_utc": "$timestamp_utc",
  "threshold_percent": $threshold,
  "mutants_exit_code": $mutants_exit,
  "scores": {
    "operational_percent": $operational_score,
    "strict_percent": $strict_score
  },
  "counts": {
    "caught": $caught_count,
    "missed": $missed_count,
    "timeout": $timeout_count,
    "unviable": $unviable_count,
    "critical_miss": $critical_miss_count,
    "critical_timeout": $critical_timeout_count,
    "critical_unviable": $critical_unviable_count
  },
  "threshold_met": $threshold_met
}
EOF

{
    echo "# Hooks mutation CI gate report"
    echo
    echo "- Status: **${status^^}**"
    echo "- Timestamp (UTC): ${timestamp_utc}"
    echo "- Mutation command exit code: ${mutants_exit}"
    echo "- Mutation threshold (operational): **>= ${threshold}%**"
    echo "- Operational score: **${operational_score}%** (caught=${caught_count}, missed=${missed_count})"
    echo "- Strict score: **${strict_score}%** (caught=${caught_count}, missed=${missed_count}, timeout=${timeout_count})"
    echo "- Critical-path MISS count: **${critical_miss_count}**"
    echo "- Critical-path TIMEOUT count: **${critical_timeout_count}**"
    echo "- Critical-path unviable count: **${critical_unviable_count}**"
    echo
    echo "## Outcome counts"
    echo
    echo "| Outcome | Count |"
    echo "|---|---:|"
    echo "| caught | ${caught_count} |"
    echo "| missed | ${missed_count} |"
    echo "| timeout | ${timeout_count} |"
    echo "| unviable | ${unviable_count} |"
    echo
    echo "## Gate checks"
    echo
    echo "- Operational threshold met (>= ${threshold}%): **$([[ "$threshold_met" == "true" ]] && echo "yes" || echo "no")**"
    echo "- Critical-path MISS invariant met: **$([[ "$critical_miss_count" -eq 0 ]] && echo "yes" || echo "no")**"
    echo "- TIMEOUT/unviable are reported separately (non-blocking for this gate)."
    echo
    if [[ "${#fail_reasons[@]}" -gt 0 ]]; then
        echo "## Failure reasons"
        echo
        for reason in "${fail_reasons[@]}"; do
            echo "- $reason"
        done
        echo
    fi

    echo "## Command"
    echo
    echo '```bash'
    echo "$mutation_cmd_string"
    echo '```'
    echo

    append_report_section "Critical MISS survivors" "$CRITICAL_MISS_PATH" 100
    append_report_section "Critical TIMEOUT survivors" "$CRITICAL_TIMEOUT_PATH" 100
    append_report_section "Critical unviable mutants" "$CRITICAL_UNVIABLE_PATH" 100
    append_report_section "All MISS survivors" "$missed_file" 80
    append_report_section "All TIMEOUT survivors" "$timeout_file" 80
    append_report_section "All unviable mutants" "$unviable_file" 80

    echo "## Artifact paths"
    echo
    echo "- Report: $REPORT_PATH"
    echo "- Summary JSON: $SUMMARY_PATH"
    echo "- Survivors (MISS + TIMEOUT): $SURVIVORS_PATH"
    echo "- Mutants output: $MUTANTS_OUT_DIR"
} > "$REPORT_PATH"

if [[ -n "${GITHUB_STEP_SUMMARY:-}" && -f "$REPORT_PATH" ]]; then
    cat "$REPORT_PATH" >> "$GITHUB_STEP_SUMMARY"
fi

echo "Hooks mutation gate status: ${status^^}"
echo "Operational score: ${operational_score}% (threshold ${threshold}%)"
echo "Counts: caught=${caught_count} missed=${missed_count} timeout=${timeout_count} unviable=${unviable_count}"
echo "Critical-path MISS/TIMEOUT/unviable: ${critical_miss_count}/${critical_timeout_count}/${critical_unviable_count}"
echo "Report: $REPORT_PATH"

if [[ "$status" == "fail" ]]; then
    exit 1
fi
