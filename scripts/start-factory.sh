#!/usr/bin/env bash
# Start the web dashboard and factory workers.
#
# Usage:
#   ./scripts/start-factory.sh              # 2 workers (default)
#   ./scripts/start-factory.sh 4            # 4 workers
#   ./scripts/start-factory.sh 3 -b kiro    # 3 workers, kiro backend
#
# Ctrl-C stops everything.

set -euo pipefail

NUM_WORKERS="${1:-2}"
shift 2>/dev/null || true  # consume $1 so remaining args pass through

BACKEND_PORT=3000
FRONTEND_PORT=5173
API_URL="http://localhost:${BACKEND_PORT}"

cleanup() {
    echo ""
    echo "Shutting down..."
    # Kill the process group children
    [[ -n "${WEB_PID:-}" ]]     && kill "$WEB_PID"     2>/dev/null || true
    [[ -n "${FACTORY_PID:-}" ]] && kill "$FACTORY_PID" 2>/dev/null || true
    wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "Starting web dashboard (backend:${BACKEND_PORT}, frontend:${FRONTEND_PORT})..."
cargo run -q --bin ralph -- web \
    --backend-port "$BACKEND_PORT" \
    --frontend-port "$FRONTEND_PORT" &
WEB_PID=$!

# Wait for the API to be ready
echo "Waiting for API server..."
for i in $(seq 1 30); do
    if curl -sf "${API_URL}/rpc/v1" -X POST \
        -H 'Content-Type: application/json' \
        -d '{"apiVersion":"v1","id":"health","method":"system.health","params":{}}' \
        >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

echo "Starting factory with ${NUM_WORKERS} workers..."
cargo run -q --bin ralph -- factory \
    -w "$NUM_WORKERS" \
    --api-url "$API_URL" \
    "$@" &
FACTORY_PID=$!

echo ""
echo "  Dashboard: http://localhost:${FRONTEND_PORT}/factory"
echo "  API:       ${API_URL}/rpc/v1"
echo "  Workers:   ${NUM_WORKERS}"
echo ""

wait
