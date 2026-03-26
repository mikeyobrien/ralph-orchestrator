#!/usr/bin/env bash
# Shared Rust CI parity gate for local development and GitHub Actions.
# By default, this runs the full local parity set:
#   - embedded files sync check
#   - cargo fmt --all -- --check
#   - cargo clippy --all-targets --all-features -- -D warnings
#   - cargo test -- --skip acp_executor::tests::test_create_terminal_and_output
#   - hooks BDD gate
#   - mock E2E smoke (non-blocking, matching CI)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Hooks inherit Git env vars (GIT_DIR/GIT_INDEX_FILE/etc.).
# Unset them so nested `git` calls in tests run against their own repos.
while IFS= read -r git_env_var; do
  unset "$git_env_var"
done < <(git rev-parse --local-env-vars 2>/dev/null || true)

RUN_SYNC=1
RUN_FMT=1
RUN_CLIPPY=1
RUN_TESTS=1
RUN_HOOKS_BDD=1
RUN_MOCK_E2E=1

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --skip-embedded-files)
      RUN_SYNC=0
      ;;
    --skip-format)
      RUN_FMT=0
      ;;
    --skip-clippy)
      RUN_CLIPPY=0
      ;;
    --skip-tests)
      RUN_TESTS=0
      ;;
    --skip-hooks-bdd)
      RUN_HOOKS_BDD=0
      ;;
    --skip-mock-e2e)
      RUN_MOCK_E2E=0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
  shift
done

TOOLCHAIN_MODE="path"
FMT_MODE="path"

if command -v rustup >/dev/null 2>&1; then
  echo "🔧 Ensuring CI-equivalent Rust toolchain (stable)..."
  components=()
  if [[ "$RUN_FMT" -eq 1 ]]; then
    components+=(--component rustfmt)
  fi
  if [[ "$RUN_CLIPPY" -eq 1 ]]; then
    components+=(--component clippy)
  fi
  rustup toolchain install stable \
    --profile minimal \
    "${components[@]}" \
    --no-self-update \
    >/dev/null
  TOOLCHAIN_MODE="rustup"
  FMT_MODE="rustup"
else
  echo "⚠️  rustup not found; using cargo/rustfmt from PATH (may differ from CI stable)." >&2

  if [[ "$RUN_FMT" -eq 1 ]]; then
    if cargo fmt --version >/dev/null 2>&1; then
      FMT_MODE="path"
    elif command -v nix >/dev/null 2>&1; then
      echo "⚠️  rustfmt not found in PATH; using nix shell fallback for fmt only." >&2
      FMT_MODE="nix"
    else
      echo "❌ rustfmt is not available (no rustup and no nix fallback)." >&2
      exit 1
    fi
  fi

  if [[ "$RUN_CLIPPY" -eq 1 ]] && ! cargo clippy --version >/dev/null 2>&1; then
    echo "❌ clippy is not available in PATH." >&2
    exit 1
  fi
fi

run_cargo() {
  if [[ "$TOOLCHAIN_MODE" == "rustup" ]]; then
    rustup run stable cargo "$@"
  else
    cargo "$@"
  fi
}

run_fmt_check() {
  case "$FMT_MODE" in
    rustup)
      rustup run stable cargo fmt --all -- --check
      ;;
    nix)
      nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#rustfmt -c cargo fmt --all -- --check
      ;;
    *)
      cargo fmt --all -- --check
      ;;
  esac
}

run_check() {
  local label="$1"
  shift

  echo
  echo "🔍 ${label}..."
  if ! "$@"; then
    echo
    echo "❌ ${label} failed!"
    exit 1
  fi
  echo "✅ ${label} passed"
}

run_non_blocking_check() {
  local label="$1"
  shift

  echo
  echo "🔍 ${label}..."
  if ! "$@"; then
    echo "⚠️  ${label} failed, but this step is non-blocking to match CI."
    return 0
  fi
  echo "✅ ${label} passed"
}

run_mock_e2e_smoke() {
  run_cargo build -p ralph-e2e
  ./target/debug/ralph-e2e --mock --skip-analysis
}

if [[ "$RUN_SYNC" -eq 1 ]]; then
  run_check "Embedded files sync check" ./scripts/sync-embedded-files.sh check
fi

if [[ "$RUN_FMT" -eq 1 ]]; then
  run_check "Formatting (cargo fmt --all -- --check)" run_fmt_check
fi

if [[ "$RUN_CLIPPY" -eq 1 ]]; then
  run_check "Clippy (all targets/features, warnings as errors)" \
    run_cargo clippy --all-targets --all-features -- -D warnings
fi

if [[ "$RUN_TESTS" -eq 1 ]]; then
  run_check "Tests (cargo test with CI skip list)" \
    run_cargo test -- --skip acp_executor::tests::test_create_terminal_and_output
fi

if [[ "$RUN_HOOKS_BDD" -eq 1 ]]; then
  run_check "Hooks BDD gate" ./scripts/hooks-bdd-gate.sh
fi

if [[ "$RUN_MOCK_E2E" -eq 1 ]]; then
  run_non_blocking_check "Mock E2E smoke (non-blocking, matching CI)" run_mock_e2e_smoke
fi

echo
echo "🎉 Rust CI parity gate passed!"
