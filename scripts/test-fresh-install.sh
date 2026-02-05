#!/usr/bin/env bash
# test-fresh-install.sh — Validate web dashboard setup works from a clean state.
#
# Usage:
#   ./scripts/test-fresh-install.sh           # Clone into tmpdir, full test
#   ./scripts/test-fresh-install.sh --local   # Delete node_modules, test in-place

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; exit 1; }
info() { echo -e "${YELLOW}→ $1${NC}"; }

# --- Pre-flight: check node/npm ---
info "Checking Node.js..."
if ! command -v node &>/dev/null; then
    fail "Node.js is not installed. Install 18+: https://nodejs.org/"
fi
NODE_VERSION=$(node --version)
NODE_MAJOR=$(echo "$NODE_VERSION" | sed 's/v//' | cut -d. -f1)
if [ "$NODE_MAJOR" -lt 18 ]; then
    fail "Node.js $NODE_VERSION is too old (need >= 18)"
fi
pass "Node.js $NODE_VERSION"

info "Checking npm..."
if ! command -v npm &>/dev/null; then
    fail "npm is not installed"
fi
NPM_VERSION=$(npm --version)
pass "npm $NPM_VERSION"

# --- Determine mode ---
LOCAL_MODE=false
if [ "${1:-}" = "--local" ]; then
    LOCAL_MODE=true
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [ "$LOCAL_MODE" = true ]; then
    info "Local mode: testing in $REPO_ROOT"
    WORK_DIR="$REPO_ROOT"

    info "Removing node_modules..."
    rm -rf "$WORK_DIR/node_modules"
    rm -rf "$WORK_DIR/backend/hats-web-server/node_modules"
    rm -rf "$WORK_DIR/frontend/hats-web/node_modules"
    pass "node_modules removed"
else
    TMPDIR=$(mktemp -d)
    trap 'rm -rf "$TMPDIR"' EXIT
    info "Clone mode: cloning into $TMPDIR"

    git clone --depth 1 "$REPO_ROOT" "$TMPDIR/hats"
    WORK_DIR="$TMPDIR/hats"
    pass "Cloned repository"
fi

cd "$WORK_DIR"

# --- npm install ---
info "Running npm install..."
npm install
pass "npm install succeeded"

# Verify lockfile artifact
if [ ! -f "node_modules/.package-lock.json" ]; then
    fail "node_modules/.package-lock.json not found after install"
fi
pass "node_modules/.package-lock.json exists"

# --- Build ---
info "Running npm run build..."
npm run build
pass "Build succeeded"

# --- Backend tests ---
info "Running backend tests (npm run test:server)..."
npm run test:server
pass "Backend tests passed"

# --- Frontend tests ---
info "Running frontend tests..."
npm run test -w @hats-web/dashboard
pass "Frontend tests passed"

echo ""
echo -e "${GREEN}All checks passed!${NC}"
