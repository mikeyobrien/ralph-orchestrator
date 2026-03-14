# Justfile for Ralph Orchestrator development
# https://github.com/casey/just

# Default recipe - show available commands
default:
    @just --list

# Run all checks (format, lint, embedded assets, test)
check: fmt-check lint embedded-check test
    @echo "✅ All checks passed"

# Format code using rustfmt
fmt:
    cargo fmt --all

# Check formatting without modifying files
fmt-check:
    cargo fmt --all -- --check

# Run clippy lints
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run tests
test:
    cargo test --all

# Sync embedded assets (crate-local mirrors + generated SOPs)
embedded-sync:
    ./scripts/sync-embedded-files.sh

# Check embedded assets are in sync
embedded-check:
    ./scripts/sync-embedded-files.sh check

# Update the pinned PDD upstream SHA from the canonical GitHub branch and resync
embedded-update-pdd-ref branch="main":
    ./scripts/sync-embedded-files.sh update-pdd-ref {{branch}}

# Type check without building
typecheck:
    cargo check --all

# Build release binary
build:
    cargo build --release

# Clean build artifacts
clean:
    cargo clean

# Full CI-like check (what CI will run)
ci: fmt-check lint embedded-check test
    @echo "✅ CI checks passed"

# Run tests with coverage report (local only, never in CI)
coverage:
    cargo llvm-cov --all-features --workspace --html --output-dir coverage
    @echo ""
    @echo "📊 Coverage report: coverage/html/index.html"

# Quick coverage summary without HTML report
coverage-summary:
    cargo llvm-cov --all-features --workspace --summary-only

# Run coverage and open report in browser
coverage-open: coverage
    open coverage/html/index.html 2>/dev/null || xdg-open coverage/html/index.html 2>/dev/null || echo "Open coverage/html/index.html manually"

# Calibrated hooks mutation rollout threshold (see docs/06-analysis/hooks-mutation-baseline-2026-03-01.md)
HOOKS_MUTATION_THRESHOLD := "55"

# Enforced hooks mutation CI gate (threshold + critical-path no-MISS invariant)
mutants-hooks-gate:
    HOOKS_MUTATION_THRESHOLD={{HOOKS_MUTATION_THRESHOLD}} ./scripts/hooks-mutation-gate.sh

# Setup development environment (install hooks)
setup:
    @echo "Development environment is managed by devenv.sh"
    @echo ""
    @echo "Prerequisites:"
    @echo "  1. Install Nix: https://nixos.org/download.html"
    @echo "  2. Install devenv: https://devenv.sh/getting-started/"
    @echo "  3. Install direnv: https://direnv.net/docs/installation.html"
    @echo ""
    @echo "Then run:"
    @echo "  direnv allow"
    @echo ""
    @echo "Or use nix develop:"
    @echo "  nix develop"
    @echo ""
    @echo "Installing git hooks..."
    ./scripts/setup-hooks.sh

# Enter development shell (for non-direnv users)
dev:
    nix develop

# Run pre-commit checks manually
pre-commit:
    @echo "🔍 Running pre-commit checks..."
    @just fmt-check
    @just lint
    @echo "✅ Pre-commit checks passed!"

# Hooks mutation baseline - run mutation tests on hooks-critical modules
# Requires cargo-mutants: cargo install cargo-mutants
# Outputs to /tmp/hooks-mutants-output
# Gates: operational score >= 55%, no MISS survivors in critical paths
mutants-baseline:
    @echo "Running mutation baseline on hooks-critical modules..."
    cargo mutants \
      --baseline skip \
      --file crates/ralph-core/src/hooks/executor.rs \
      --file crates/ralph-core/src/hooks/engine.rs \
      --file crates/ralph-core/src/preflight.rs \
      --file crates/ralph-cli/src/loop_runner.rs \
      -o /tmp/hooks-mutants-output \
      --no-times \
      --colors never \
      --caught \
      --unviable
    @echo ""
    @echo "Results in /tmp/hooks-mutants-output/"
    @echo "Run: cat /tmp/hooks-mutants-output/{caught,missed,timeout}.txt"
    @echo ""
    @echo "Gate checks:"
    @echo "1. Operational score >= 55%: Calculate as: caught / (caught + missed)"
    @echo "2. No MISS survivors in critical paths (loop_runner.rs:3467-3560, 3623-3635)"
