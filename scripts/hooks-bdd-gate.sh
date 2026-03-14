#!/usr/bin/env bash
# Enforce hooks BDD acceptance quality gate for CI.
#
# Gate semantics (Step 13.5):
# - Run hooks BDD in deterministic CI-safe mode.
# - Require full AC coverage (AC-01..AC-18) with zero failures.
# - Always emit actionable artifacts (raw output + concise report + JSON summary).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="${HOOKS_BDD_ARTIFACT_DIR:-$REPO_ROOT/.artifacts/hooks-bdd}"
RAW_OUTPUT_PATH="$ARTIFACT_DIR/hooks-bdd.out"
REPORT_PATH="$ARTIFACT_DIR/hooks-bdd-report.md"
SUMMARY_PATH="$ARTIFACT_DIR/hooks-bdd-summary.json"
FAILURES_PATH="$ARTIFACT_DIR/hooks-bdd-failures.txt"

EXPECTED_AC_IDS=(
    "AC-01" "AC-02" "AC-03" "AC-04" "AC-05" "AC-06"
    "AC-07" "AC-08" "AC-09" "AC-10" "AC-11" "AC-12"
    "AC-13" "AC-14" "AC-15" "AC-16" "AC-17" "AC-18"
)

json_array_from_args() {
    if [[ "$#" -eq 0 ]]; then
        printf '[]'
        return
    fi

    printf '['
    local first=1
    local value
    for value in "$@"; do
        if [[ "$first" -eq 0 ]]; then
            printf ', '
        fi
        printf '"%s"' "$value"
        first=0
    done
    printf ']'
}

rm -rf "$ARTIFACT_DIR"
mkdir -p "$ARTIFACT_DIR"

hooks_bdd_cmd=(
    cargo run -p ralph-e2e -- --hooks-bdd --mock --quiet
)

set +e
(
    cd "$REPO_ROOT"
    set -o pipefail
    "${hooks_bdd_cmd[@]}" 2>&1 | tee "$RAW_OUTPUT_PATH"
)
hooks_bdd_exit=$?
set -e

summary_line="$(grep -E 'Summary: [0-9]+ passed, [0-9]+ failed, [0-9]+ total' "$RAW_OUTPUT_PATH" | tail -n 1 || true)"
passed_count=0
failed_count=0
total_count=0

if [[ -n "$summary_line" ]]; then
    read -r passed_count failed_count total_count < <(
        printf '%s\n' "$summary_line" | sed -E 's/^.*Summary: ([0-9]+) passed, ([0-9]+) failed, ([0-9]+) total.*$/\1 \2 \3/'
    )
fi

mapfile -t pass_ids < <(
    grep -oE 'PASS AC-[0-9]{2}' "$RAW_OUTPUT_PATH" | awk '{print $2}' | sort -u || true
)
mapfile -t fail_ids < <(
    grep -oE 'FAIL AC-[0-9]{2}' "$RAW_OUTPUT_PATH" | awk '{print $2}' | sort -u || true
)

declare -A pass_map=()
declare -A fail_map=()

for ac_id in "${pass_ids[@]}"; do
    pass_map["$ac_id"]=1
done
for ac_id in "${fail_ids[@]}"; do
    fail_map["$ac_id"]=1
done

if ! grep -E 'FAIL AC-[0-9]{2}' "$RAW_OUTPUT_PATH" > "$FAILURES_PATH"; then
    : > "$FAILURES_PATH"
fi

missing_ids=()
for ac_id in "${EXPECTED_AC_IDS[@]}"; do
    if [[ -z "${pass_map[$ac_id]:-}" && -z "${fail_map[$ac_id]:-}" ]]; then
        missing_ids+=("$ac_id")
    fi
done

status="pass"
fail_reasons=()

if [[ "$hooks_bdd_exit" -ne 0 ]]; then
    fail_reasons+=("hooks BDD command exited with code $hooks_bdd_exit")
fi

if [[ -z "$summary_line" ]]; then
    fail_reasons+=("missing summary line in hooks BDD output")
fi

if [[ "$failed_count" -ne 0 ]]; then
    fail_reasons+=("summary reports $failed_count failed scenario(s)")
fi

if [[ "$total_count" -ne "${#EXPECTED_AC_IDS[@]}" ]]; then
    fail_reasons+=("expected ${#EXPECTED_AC_IDS[@]} total scenarios, observed $total_count")
fi

if [[ "${#missing_ids[@]}" -gt 0 ]]; then
    fail_reasons+=("missing AC IDs from output: ${missing_ids[*]}")
fi

if [[ "${#fail_reasons[@]}" -gt 0 ]]; then
    status="fail"
fi

timestamp_utc="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
command_string="cargo run -p ralph-e2e -- --hooks-bdd --mock --quiet"

pass_ids_json="$(json_array_from_args "${pass_ids[@]}")"
fail_ids_json="$(json_array_from_args "${fail_ids[@]}")"
missing_ids_json="$(json_array_from_args "${missing_ids[@]}")"

cat > "$SUMMARY_PATH" <<EOF
{
  "status": "$status",
  "timestamp_utc": "$timestamp_utc",
  "command": "$command_string",
  "exit_code": $hooks_bdd_exit,
  "summary_line": "$summary_line",
  "counts": {
    "passed": $passed_count,
    "failed": $failed_count,
    "total": $total_count,
    "expected_total": ${#EXPECTED_AC_IDS[@]}
  },
  "ac": {
    "passed_ids": $pass_ids_json,
    "failed_ids": $fail_ids_json,
    "missing_ids": $missing_ids_json
  }
}
EOF

{
    echo "# Hooks BDD CI gate report"
    echo
    echo "- Status: **${status^^}**"
    echo "- Timestamp (UTC): ${timestamp_utc}"
    echo "- Command: \`${command_string}\`"
    echo "- Exit code: ${hooks_bdd_exit}"
    echo "- Summary: ${summary_line:-<missing>}"
    echo "- Traceability matrix: \`crates/ralph-e2e/features/hooks/TRACEABILITY.md\`"
    echo
    echo "## Scenario counts"
    echo
    echo "| Metric | Value |"
    echo "|---|---:|"
    echo "| passed | ${passed_count} |"
    echo "| failed | ${failed_count} |"
    echo "| total | ${total_count} |"
    echo "| expected | ${#EXPECTED_AC_IDS[@]} |"
    echo
    echo "## AC coverage (AC-01..AC-18)"
    echo
    echo "| AC ID | Status |"
    echo "|---|---|"

    for ac_id in "${EXPECTED_AC_IDS[@]}"; do
        ac_status="missing"
        if [[ -n "${pass_map[$ac_id]:-}" ]]; then
            ac_status="pass"
        elif [[ -n "${fail_map[$ac_id]:-}" ]]; then
            ac_status="fail"
        fi

        echo "| ${ac_id} | ${ac_status} |"
    done

    echo

    if [[ -s "$FAILURES_PATH" ]]; then
        echo "## Failed AC lines"
        echo
        echo '```text'
        cat "$FAILURES_PATH"
        echo '```'
        echo
    fi

    if [[ "${#missing_ids[@]}" -gt 0 ]]; then
        echo "## Missing AC IDs"
        echo
        for ac_id in "${missing_ids[@]}"; do
            echo "- ${ac_id}"
        done
        echo
    fi

    if [[ "${#fail_reasons[@]}" -gt 0 ]]; then
        echo "## Failure reasons"
        echo
        for reason in "${fail_reasons[@]}"; do
            echo "- ${reason}"
        done
        echo
    fi

    echo "## Raw output excerpt"
    echo
    echo '```text'
    tail -n 120 "$RAW_OUTPUT_PATH"
    echo '```'
    echo

    echo "## Artifact paths"
    echo
    echo "- Raw output: $RAW_OUTPUT_PATH"
    echo "- Summary JSON: $SUMMARY_PATH"
    echo "- Markdown report: $REPORT_PATH"
    echo "- Failed AC lines: $FAILURES_PATH"
} > "$REPORT_PATH"

if [[ -n "${GITHUB_STEP_SUMMARY:-}" && -f "$REPORT_PATH" ]]; then
    cat "$REPORT_PATH" >> "$GITHUB_STEP_SUMMARY"
fi

echo "Hooks BDD gate status: ${status^^}"
echo "Summary: ${summary_line:-<missing>}"
echo "AC counts: pass_ids=${#pass_ids[@]} fail_ids=${#fail_ids[@]} missing_ids=${#missing_ids[@]}"
echo "Report: $REPORT_PATH"

if [[ "$status" == "fail" ]]; then
    exit 1
fi
