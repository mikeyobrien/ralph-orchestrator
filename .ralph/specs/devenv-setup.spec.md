---
status: implemented
gap_analysis: 2026-02-03
related: []
---

# Development Environment Setup

## Overview

Set up a proper, reproducible development environment for Ralph Orchestrator using Nix and devenv.sh. This ensures all contributors have identical tool versions and code quality checks are enforced automatically.

## Design

### Goals

1. **Reproducibility**: Every contributor gets the exact same toolchain
2. **Zero setup friction**: One command to get a working dev environment
3. **Quality enforcement**: Automatic formatting and linting checks
4. **Cross-platform**: Works on Linux, macOS, and WSL

### Tools Provided

- **Rust toolchain** (rustc, cargo, clippy, rustfmt, rust-analyzer)
- **just** - Command runner for common tasks
- **git** - Version control
- **cargo-watch** - File watcher for auto-rebuilds
- **cargo-nextest** - Better test runner

### Implementation

Uses **devenv.sh** with Nix Flakes:

```
flake.nix      # Entry point for nix develop
├── devenv.nix    # Dev environment configuration
├── devenv.yaml   # devenv settings
└── .envrc        # direnv integration (auto-activate)
```

### Activation Methods

1. **direnv** (recommended): Automatically activates when entering directory
2. **nix develop**: Manual activation for non-direnv users

## Acceptance Criteria

### Nix Flake
- **Given** User has Nix installed
- **When** User runs `nix develop`
- **Then** They enter a shell with all dev tools available

### Direnv Integration
- **Given** User has direnv installed and allowed
- **When** They enter the project directory
- **Then** The dev environment automatically activates

### Tool Availability
- **Given** Dev environment is active
- **When** User runs `cargo fmt --version`
- **Then** rustfmt is available and working

### Pre-commit Checks
- **Given** User attempts to commit
- **When** Pre-commit hooks run
- **Then** Formatting and linting checks pass before commit is allowed

### Just Tasks
- **Given** Dev environment is active
- **When** User runs `just check`
- **Then** All checks (fmt, lint, test) run successfully

## Files Created

- `flake.nix` - Nix flake entry point
- `devenv.nix` - devenv configuration with Rust toolchain
- `devenv.yaml` - devenv settings
- `.envrc` - direnv configuration
- `Justfile` - Common development tasks
- `.hooks/pre-commit` - Git pre-commit hook (backup for non-devenv users)

## Usage

```bash
# First time setup
git clone <repo>
cd ralph-orchestrator
direnv allow  # or: nix develop

# Development workflow
just check     # Run all checks
just fmt       # Format code
just lint      # Run clippy
just test      # Run tests
just build     # Build release binary
```

## Notes

- Requires Nix with flakes enabled
- Uses devenv for declarative dev environment management
- All Rust components come from Nix, not rustup
- Works offline after initial Nix evaluation
