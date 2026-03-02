#!/usr/bin/env sh
set -eu

# Minimal blocking guard example.
# Fails fast when required hook context variables are missing.
: "${RALPH_HOOK_PHASE_EVENT:?env-guard: missing RALPH_HOOK_PHASE_EVENT}"
: "${RALPH_LOOP_ID:?env-guard: missing RALPH_LOOP_ID}"
: "${RALPH_WORKSPACE:?env-guard: missing RALPH_WORKSPACE}"

printf 'env-guard: context OK for %s (loop %s)\n' "$RALPH_HOOK_PHASE_EVENT" "$RALPH_LOOP_ID"
