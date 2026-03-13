#!/usr/bin/env bash
# smoke-core-presets.sh - Run the core preset suite in smoke mode with small real tasks
#
# Usage: ./tools/smoke-core-presets.sh [backend]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND=${1:-claude}

export RALPH_PRESET_TASK_VARIANT=smoke
exec "$SCRIPT_DIR/evaluate-all-presets.sh" "$BACKEND" smoke
