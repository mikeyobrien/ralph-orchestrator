#!/usr/bin/env bash
# Manual, paid smoke for the six live autoloop provider harnesses.

set -euo pipefail

SCRIPT_PATH=${BASH_SOURCE[0]}
case "$SCRIPT_PATH" in
  /*) ;;
  *) SCRIPT_PATH="$PWD/$SCRIPT_PATH" ;;
esac
SCRIPT_DIR=$(cd "${SCRIPT_PATH%/*}" && pwd -P)
SOURCE_ROOT=$(cd "$SCRIPT_DIR/.." && pwd -P)
PRESET_DIR="$SOURCE_ROOT/presets/live-harness-smoke"
RESULT_PARSER="$SOURCE_ROOT/tools/smoke_live_harness_results.py"

RALPH_COMMAND=${RALPH_BIN:-ralph}
AUTOLOOP_COMMAND=${AUTOLOOP_BIN:-autoloop}
KEEP_SMOKE_DIR=${KEEP_SMOKE_DIR:-0}
SMOKE_MAX_COST_USD=${SMOKE_MAX_COST_USD:-5}
SMOKE_TIMEOUT_SECONDS=${SMOKE_TIMEOUT_SECONDS:-2700}

case "$KEEP_SMOKE_DIR" in
  0|1) ;;
  *) printf 'ERROR: KEEP_SMOKE_DIR must be 0 or 1 (got %s)\n' "$KEEP_SMOKE_DIR" >&2; exit 2 ;;
esac
if [[ ! "$SMOKE_TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  printf 'ERROR: SMOKE_TIMEOUT_SECONDS must be a positive integer (got %s)\n' "$SMOKE_TIMEOUT_SECONDS" >&2
  exit 2
fi
if [[ ! "$SMOKE_MAX_COST_USD" =~ ^([0-9]+([.][0-9]*)?|[.][0-9]+)$ ]]; then
  printf 'ERROR: SMOKE_MAX_COST_USD must be a nonnegative number (got %s)\n' "$SMOKE_MAX_COST_USD" >&2
  exit 2
fi

# Resolve the complete matrix before creating a workspace or launching anything.
# This is intentionally not a provider-auth flow: each CLI must already be logged in.
resolve_executable() {
  local label=$1
  local command_name=$2
  local resolved
  if ! resolved=$(command -v "$command_name" 2>/dev/null); then
    printf 'ERROR: missing required executable %s (%s); install/authenticate it and retry\n' \
      "$label" "$command_name" >&2
    return 1
  fi
  case "$resolved" in
    /*) ;;
    *) resolved="$PWD/${resolved#./}" ;;
  esac
  printf '%s' "$resolved"
}

RALPH_RESOLVED=$(resolve_executable ralph "$RALPH_COMMAND") || exit 1
AUTOLOOP_RESOLVED=$(resolve_executable autoloop "$AUTOLOOP_COMMAND") || exit 1
GIT_RESOLVED=$(resolve_executable git git) || exit 1
resolve_executable claude claude >/dev/null || exit 1
resolve_executable codex codex >/dev/null || exit 1
resolve_executable opencode opencode >/dev/null || exit 1
resolve_executable pi pi >/dev/null || exit 1
resolve_executable hermes hermes >/dev/null || exit 1
resolve_executable kiro kiro-cli >/dev/null || exit 1
PYTHON_RESOLVED=$(resolve_executable python3 python3) || exit 1

WORKSPACE=
RUN_SUCCEEDED=0
RALPH_PID=
WATCHDOG_PID=
cleanup() {
  local status=$?
  trap - EXIT INT TERM HUP
  if [[ -n "$WATCHDOG_PID" ]]; then
    kill "$WATCHDOG_PID" 2>/dev/null || true
  fi
  if [[ -n "$RALPH_PID" ]]; then
    kill "$RALPH_PID" 2>/dev/null || true
  fi
  if [[ -n "$WORKSPACE" && -d "$WORKSPACE" ]]; then
    if [[ "$RUN_SUCCEEDED" == 1 && "$KEEP_SMOKE_DIR" != 1 ]]; then
      rm -rf "$WORKSPACE"
    else
      printf 'Smoke workspace retained: %s\n' "$WORKSPACE" >&2
    fi
  fi
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP

WORKSPACE=$(mktemp -d "${TMPDIR:-/tmp}/ralph-live-harness-smoke.XXXXXX")
CONFIG_PATH="$WORKSPACE/ralph-smoke.yml"
OUTPUT_PATH="$WORKSPACE/ralph-output.log"
TIMEOUT_MARKER="$WORKSPACE/timeout"
SHIM_DIR="$WORKSPACE/bin"
mkdir -p "$SHIM_DIR"
ln -s "$AUTOLOOP_RESOLVED" "$SHIM_DIR/autoloop"

"$GIT_RESOLVED" -C "$WORKSPACE" init -q
"$GIT_RESOLVED" -C "$WORKSPACE" config user.email smoke@example.invalid
"$GIT_RESOLVED" -C "$WORKSPACE" config user.name 'Ralph Live Harness Smoke'
printf '# Disposable Ralph live-harness smoke workspace\n' >"$WORKSPACE/README.md"
"$GIT_RESOLVED" -C "$WORKSPACE" add README.md
"$GIT_RESOLVED" -C "$WORKSPACE" commit -q -m init

# Single-quoted YAML scalars escape an embedded quote by doubling it.
yaml_preset=${PRESET_DIR//\'/\'\'}
printf '%s\n' \
  'core:' \
  '  engine: autoloop' \
  "  autoloop_preset: '$yaml_preset'" \
  'event_loop:' \
  '  max_iterations: 6' \
  "  max_runtime_seconds: $SMOKE_TIMEOUT_SECONDS" \
  "  max_cost_usd: $SMOKE_MAX_COST_USD" \
  >"$CONFIG_PATH"

RERUN_COMMAND="KEEP_SMOKE_DIR=1 SMOKE_MAX_COST_USD=$SMOKE_MAX_COST_USD SMOKE_TIMEOUT_SECONDS=$SMOKE_TIMEOUT_SECONDS $SCRIPT_PATH"
printf 'Running paid live harness smoke in %s (timeout=%ss, max_cost_usd=%s)\n' \
  "$WORKSPACE" "$SMOKE_TIMEOUT_SECONDS" "$SMOKE_MAX_COST_USD"

# Ralph owns the engine child; the outer watchdog is a second fail-closed wall-clock bound.
(
  cd "$WORKSPACE"
  unset RALPH_CONFIG RALPH_WORKSPACE_ROOT RALPH_MERGE_LOOP_ID
  PATH="$SHIM_DIR:$PATH" exec "$RALPH_RESOLVED" \
    --color never --config "$CONFIG_PATH" run \
    --prompt 'Run the exact live harness smoke contract.' \
    --max-iterations 6 --no-tui --skip-preflight --verbose
) >"$OUTPUT_PATH" 2>&1 &
RALPH_PID=$!

"$PYTHON_RESOLVED" -c '
import os, pathlib, signal, sys, time
seconds, pid, marker = int(sys.argv[1]), int(sys.argv[2]), pathlib.Path(sys.argv[3])
time.sleep(seconds)
marker.write_text(f"timeout after {seconds} seconds\\n")
try:
    os.kill(pid, signal.SIGTERM)
except ProcessLookupError:
    raise SystemExit(0)
time.sleep(10)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass
' "$SMOKE_TIMEOUT_SECONDS" "$RALPH_PID" "$TIMEOUT_MARKER" &
WATCHDOG_PID=$!

set +e
wait "$RALPH_PID"
RALPH_STATUS=$?
set -e
RALPH_PID=
kill "$WATCHDOG_PID" 2>/dev/null || true
wait "$WATCHDOG_PID" 2>/dev/null || true
WATCHDOG_PID=

JOURNAL_PATH="$WORKSPACE/.autoloop/journal.jsonl"
EVIDENCE_PATH="$WORKSPACE/.autoloop/missing-smoke-evidence.txt"
run_dirs=("$WORKSPACE"/.autoloop/runs/*)
if [[ ${#run_dirs[@]} -eq 1 && -d "${run_dirs[0]}" ]]; then
  EVIDENCE_PATH="${run_dirs[0]}/smoke-evidence.txt"
fi

parser_args=(
  "$RESULT_PARSER"
  --journal "$JOURNAL_PATH"
  --evidence "$EVIDENCE_PATH"
  --output "$OUTPUT_PATH"
  --workspace "$WORKSPACE"
  --rerun "$RERUN_COMMAND"
  --ralph-status "$RALPH_STATUS"
)
if [[ -f "$TIMEOUT_MARKER" ]]; then
  parser_args+=(--timed-out)
fi

set +e
"$PYTHON_RESOLVED" "${parser_args[@]}"
RESULT_STATUS=$?
set -e
if [[ "$RESULT_STATUS" -ne 0 ]]; then
  exit "$RESULT_STATUS"
fi

if [[ "$KEEP_SMOKE_DIR" == 1 ]]; then
  printf 'PASS: live smoke evidence retained in %s\n' "$WORKSPACE"
else
  printf 'PASS: live smoke complete; disposable workspace will be removed\n'
fi
RUN_SUCCEEDED=1
