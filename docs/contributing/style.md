# Code Style Guide

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive style guidelines.

## Overview

Hats follows Rust community conventions with project-specific additions.

## Rust Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting

## Pre-commit Hooks

```bash
# Install hooks
./scripts/setup-hooks.sh

# Hooks run automatically on commit:
# - cargo fmt --check
# - cargo clippy
```

## Documentation Style

- Use present tense ("adds" not "added")
- Keep lines under 100 characters
- Include examples for public APIs

## See Also

- [Development Setup](setup.md) - Environment setup
- [Testing](testing.md) - Test guidelines
- [Submitting PRs](pull-requests.md) - PR process
