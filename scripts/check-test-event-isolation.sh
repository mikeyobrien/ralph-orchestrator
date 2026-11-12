#!/usr/bin/env bash
# Regression check: the test suites must not write into the caller's loop state.
#
# When `cargo test` runs inside a ralph loop, the loop's RALPH_EVENTS_FILE and
# RALPH_WORKSPACE_ROOT are in the environment. Any test that spawns `ralph` (or
# reads those vars in-process) without setting its own target writes stray events
# such as `test.topic` into the live loop, where they can trigger hat handoffs.
#
# This script points both variables at a sentinel directory outside the workspace,
# runs the suites, and fails if anything under the sentinel changed.
#
# Usage:
#   ./scripts/check-test-event-isolation.sh                  # ralph-core, ralph-adapters, ralph-cli
#   ./scripts/check-test-event-isolation.sh ralph-cli        # only the named crates
#
# ralph-cli runs with --test-threads=1 (its wave/backend tests race on PATH in parallel).

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

crates=("$@")
if [ ${#crates[@]} -eq 0 ]; then
    crates=(ralph-core ralph-adapters ralph-cli)
fi

sentinel_dir="$(mktemp -d "${TMPDIR:-/tmp}/ralph-event-sentinel.XXXXXX")"
case "$sentinel_dir" in
    "$REPO_ROOT"/*) echo "sentinel dir must be outside the workspace: $sentinel_dir" >&2; exit 2 ;;
esac
sentinel_events="$sentinel_dir/events.jsonl"
printf '%s\n' '{"topic":"sentinel.marker","payload":"must stay the only line","ts":"1970-01-01T00:00:00Z"}' >"$sentinel_events"

snapshot() {
    (cd "$sentinel_dir" && find . -print0 | sort -z | xargs -0 sha256sum 2>/dev/null; find . | sort)
}
before="$(snapshot)"

export RALPH_EVENTS_FILE="$sentinel_events"
export RALPH_WORKSPACE_ROOT="$sentinel_dir"

suite_failures=()
for crate in "${crates[@]}"; do
    extra=()
    if [ "$crate" = "ralph-cli" ]; then
        extra=(-- --test-threads=1)
    fi
    echo "==> cargo test -p $crate ${extra[*]}"
    if ! cargo test -p "$crate" "${extra[@]}"; then
        suite_failures+=("$crate")
    fi
done

after="$(snapshot)"
status=0
if [ "$before" != "$after" ]; then
    echo
    echo "LEAK: tests modified the caller's loop state under $sentinel_dir" >&2
    diff <(echo "$before") <(echo "$after") >&2
    echo "--- sentinel events file now contains:" >&2
    cat "$sentinel_events" >&2
    status=1
else
    echo "OK: sentinel events file and workspace root unchanged"
    rm -rf "$sentinel_dir"
fi

if [ ${#suite_failures[@]} -gt 0 ]; then
    echo "suite failures: ${suite_failures[*]}" >&2
    status=1
fi

exit $status
