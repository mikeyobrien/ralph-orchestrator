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
PROCESS_GROUP_HELPER="$SOURCE_ROOT/tools/smoke_process_group.py"

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

# Resolve and authenticate the complete matrix before creating a workspace or
# launching anything. These commands are read-only status/catalog operations;
# this runner never invokes login, setup, refresh, or config mutation.
PREFLIGHT_ERRORS=()
RALPH_RESOLVED=
AUTOLOOP_RESOLVED=
GIT_RESOLVED=
CLAUDE_RESOLVED=
CODEX_RESOLVED=
OPENCODE_RESOLVED=
PI_RESOLVED=
HERMES_RESOLVED=
KIRO_RESOLVED=
PYTHON_RESOLVED=
add_preflight_error() {
  PREFLIGHT_ERRORS+=("$1")
}

resolve_executable() {
  local variable_name=$1
  local label=$2
  local command_name=$3
  local resolved
  if ! resolved=$(command -v "$command_name" 2>/dev/null); then
    add_preflight_error "missing required executable $label ($command_name)"
    printf -v "$variable_name" '%s' ''
    return
  fi
  case "$resolved" in
    /*) ;;
    *) resolved="$PWD/${resolved#./}" ;;
  esac
  printf -v "$variable_name" '%s' "$resolved"
}

resolve_executable RALPH_RESOLVED ralph "$RALPH_COMMAND"
resolve_executable AUTOLOOP_RESOLVED autoloop "$AUTOLOOP_COMMAND"
resolve_executable GIT_RESOLVED git git
resolve_executable CLAUDE_RESOLVED claude claude
resolve_executable CODEX_RESOLVED codex codex
resolve_executable OPENCODE_RESOLVED opencode opencode
resolve_executable PI_RESOLVED pi pi
resolve_executable HERMES_RESOLVED hermes hermes
resolve_executable KIRO_RESOLVED kiro kiro-cli
resolve_executable PYTHON_RESOLVED python3 python3

if [[ -n "$CLAUDE_RESOLVED" && -n "$PYTHON_RESOLVED" ]]; then
  claude_status=$("$CLAUDE_RESOLVED" auth status --json 2>/dev/null) || claude_status=
  if ! printf '%s' "$claude_status" | "$PYTHON_RESOLVED" -c \
    'import json, sys; raise SystemExit(0 if json.load(sys.stdin).get("loggedIn") is True else 1)' \
    2>/dev/null; then
    add_preflight_error "Claude auth status did not report loggedIn=true"
  fi
fi
if [[ -n "$CODEX_RESOLVED" ]] && ! "$CODEX_RESOLVED" login status >/dev/null 2>&1; then
  add_preflight_error "Codex login status failed"
fi
if [[ -n "$OPENCODE_RESOLVED" && -n "$PYTHON_RESOLVED" ]]; then
  opencode_status=$("$OPENCODE_RESOLVED" auth list --pure 2>/dev/null) || opencode_status=
  if ! printf '%s' "$opencode_status" | "$PYTHON_RESOLVED" -c \
    'import re, sys; text=re.sub(r"\x1b\[[0-9;]*m", "", sys.stdin.read()); raise SystemExit(0 if re.search(r"\b[1-9][0-9]* credentials?\b", text) else 1)'; then
    add_preflight_error "OpenCode auth list contains no credentials"
  fi
fi
if [[ -n "$PI_RESOLVED" && -n "$PYTHON_RESOLVED" ]]; then
  pi_status=$(PI_OFFLINE=1 "$PI_RESOLVED" --offline --list-models 2>/dev/null) || pi_status=
  if ! printf '%s' "$pi_status" | "$PYTHON_RESOLVED" -c \
    'import sys; lines=[line for line in sys.stdin.read().splitlines() if line.strip()]; raise SystemExit(0 if len(lines) > 1 else 1)'; then
    add_preflight_error "Pi offline model catalog has no configured models"
  fi
fi
if [[ -n "$HERMES_RESOLVED" ]]; then
  hermes_dump=$("$HERMES_RESOLVED" dump 2>/dev/null) || hermes_dump=
  hermes_provider=$(printf '%s' "$hermes_dump" | sed -n 's/^provider:[[:space:]]*//p' | head -n 1)
  hermes_status=
  if [[ -n "$hermes_provider" ]]; then
    hermes_status=$("$HERMES_RESOLVED" auth status "$hermes_provider" 2>/dev/null) || hermes_status=
  fi
  if [[ -z "$hermes_provider" || "$hermes_status" != *": logged in"* ]]; then
    add_preflight_error "Hermes selected provider is missing or not logged in"
  fi
fi
if [[ -n "$KIRO_RESOLVED" ]] && ! "$KIRO_RESOLVED" whoami --format json >/dev/null 2>&1; then
  add_preflight_error "Kiro whoami failed"
fi

if [[ ${#PREFLIGHT_ERRORS[@]} -ne 0 ]]; then
  printf 'ERROR: live harness preflight failed; no workspace was created and no provider backend was launched:\n' >&2
  printf '  - %s\n' "${PREFLIGHT_ERRORS[@]}" >&2
  exit 1
fi

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
    "$PYTHON_RESOLVED" "$PROCESS_GROUP_HELPER" terminate "$RALPH_PID" 2>/dev/null || true
    wait "$RALPH_PID" 2>/dev/null || true
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

# Ralph and every engine/provider descendant run in a dedicated process group.
# The outer watchdog is a second fail-closed wall-clock bound for that whole group.
(
  cd "$WORKSPACE"
  unset RALPH_CONFIG RALPH_WORKSPACE_ROOT RALPH_MERGE_LOOP_ID
  PATH="$SHIM_DIR:$PATH" exec "$PYTHON_RESOLVED" "$PROCESS_GROUP_HELPER" launch \
    "$RALPH_RESOLVED" --color never --config "$CONFIG_PATH" run \
    --prompt 'Run the exact live harness smoke contract.' \
    --max-iterations 6 --no-tui --skip-preflight --verbose
) >"$OUTPUT_PATH" 2>&1 &
RALPH_PID=$!

"$PYTHON_RESOLVED" -c '
import pathlib, subprocess, sys, time
seconds, pid, marker, python, helper = int(sys.argv[1]), sys.argv[2], pathlib.Path(sys.argv[3]), sys.argv[4], sys.argv[5]
time.sleep(seconds)
marker.write_text(f"timeout after {seconds} seconds\\n")
subprocess.run([python, helper, "terminate", pid], check=False)
' "$SMOKE_TIMEOUT_SECONDS" "$RALPH_PID" "$TIMEOUT_MARKER" "$PYTHON_RESOLVED" "$PROCESS_GROUP_HELPER" &
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
