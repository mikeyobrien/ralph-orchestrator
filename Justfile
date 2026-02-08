# Justfile for Ralph Orchestrator development
# https://github.com/casey/just

# Default recipe - show available commands
default:
    @just --list

# Run all checks (format, lint, test, check)
check: fmt-check lint test
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
ci: fmt-check lint test
    @echo "✅ CI checks passed"

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
