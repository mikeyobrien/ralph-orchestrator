# Contributing to Hats

Thank you for considering contributing to Hats! This document provides guidelines and information to help you contribute effectively.

## Code of Conduct

This project and everyone participating in it is governed by the [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) 1.75+
- At least one AI CLI backend ([Claude Code](https://github.com/anthropics/claude-code), [Kiro](https://kiro.dev/), [Gemini CLI](https://github.com/google-gemini/gemini-cli), etc.)

### Development Setup

```bash
# Clone the repository
git clone https://github.com/mikeyobrien/hats.git
cd hats

# Install git hooks for pre-commit checks
./scripts/setup-hooks.sh

# Build the project
cargo build

# Run tests
cargo test
```

## How to Contribute

### Reporting Bugs

Before creating bug reports, please check existing issues to avoid duplicates. When you create a bug report, include as many details as possible:

- **Use a clear and descriptive title**
- **Describe the exact steps to reproduce the problem**
- **Provide specific examples** (code snippets, config files, etc.)
- **Describe the behavior you observed and what you expected**
- **Include your environment** (OS, Rust version, backend CLI version)

### Suggesting Features

Feature suggestions are welcome! Please:

- **Check existing issues** to see if the feature has been suggested
- **Provide a clear description** of the feature and its use case
- **Explain why this feature would be useful** to most users

### Pull Requests

1. **Fork the repository** and create your branch from `main`
2. **Write tests** for new functionality
3. **Follow the code style** (run `cargo fmt` and `cargo clippy`)
4. **Update documentation** if needed
5. **Ensure all tests pass** before submitting

#### Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Write tests for new functionality
4. Ensure `cargo test` passes
5. Run `cargo clippy --all-targets --all-features`
6. Run `cargo fmt --check`
7. Commit your changes (`git commit -m 'Add amazing feature'`)
8. Push to the branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

### Commit Messages

- Use clear, descriptive commit messages
- Start with a verb in the present tense ("Add feature" not "Added feature")
- Reference issues when applicable (`Fixes #123`)

## Development Guidelines

### Philosophy

Read [AGENTS.md](AGENTS.md) for the full development philosophy. Key tenets:

1. **Fresh Context Is Reliability** - Each iteration clears context
2. **Backpressure Over Prescription** - Create gates that reject bad work
3. **The Plan Is Disposable** - Regeneration is cheap
4. **Disk Is State, Git Is Memory** - Files are the handoff mechanism

### Code Style

- Run `cargo fmt` before committing
- Address all `cargo clippy` warnings
- Follow Rust idioms and best practices
- Add doc comments for public APIs

### Testing

```bash
# Run all tests
cargo test

# Run smoke tests (replay-based, no API calls)
cargo test -p hats-core smoke_runner

# Run with coverage
cargo tarpaulin --out Html --output-dir coverage --skip-clean
```

**Important**: Always run smoke tests after making code changes. Smoke tests use recorded fixtures and are fast, free, and deterministic.

### Project Structure

```
hats/
├── crates/
│   ├── hats-cli/      # CLI application
│   ├── hats-core/     # Core library
│   ├── hats-tui/      # Terminal UI
│   ├── hats-adapters/ # Backend adapters
│   └── hats-e2e/      # End-to-end tests
├── presets/            # Pre-configured hat collections
├── specs/              # Design specifications
└── tasks/              # Code task files
```

### Creating Specs

Before implementing significant features:

1. Create a spec in `specs/` using the PDD methodology
2. Get the spec reviewed/approved
3. Implement following the spec

The bar: A new team member should be able to implement using only the spec and codebase.

### Recording Test Fixtures

To create new test fixtures from live sessions:

```bash
# Record a session
cargo run --bin hats -- run -c hats.claude.yml --record-session session.jsonl -p "your prompt"
```

See `crates/hats-core/tests/fixtures/` for fixture format details.

## Anti-Patterns to Avoid

- Building features into the orchestrator that agents can handle
- Complex retry logic (fresh context handles recovery)
- Detailed step-by-step instructions (use backpressure instead)
- Scoping work at task selection time (scope at plan creation)
- Assuming functionality is missing without code verification

## Need Help?

- **Issues**: Open an issue for bugs or feature requests
- **Discussions**: Use GitHub Discussions for questions
- **Documentation**: Check the [docs](https://mikeyobrien.github.io/hats/)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
