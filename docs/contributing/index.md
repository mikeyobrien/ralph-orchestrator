# Contributing to Hats

We welcome contributions to Hats!

## In This Section

| Guide | Description |
|-------|-------------|
| [Development Setup](setup.md) | Set up your dev environment |
| [Code Style](style.md) | Coding standards and conventions |
| [Testing](testing.md) | Writing and running tests |
| [Submitting PRs](pull-requests.md) | Pull request process |

## Quick Start

```bash
# Clone the repo
git clone https://github.com/mikeyobrien/hats.git
cd hats

# Build
cargo build

# Run tests
cargo test

# Install git hooks
./scripts/setup-hooks.sh
```

## Ways to Contribute

### Report Bugs

Found a bug? [Open an issue](https://github.com/mikeyobrien/hats/issues/new) with:

- Description of the problem
- Steps to reproduce
- Expected vs actual behavior
- Hats version and backend used

### Suggest Features

Have an idea? [Start a discussion](https://github.com/mikeyobrien/hats/discussions/new) first to:

- Explain the use case
- Discuss potential approaches
- Get feedback before implementing

### Submit Code

1. Fork the repository
2. Create a feature branch
3. Write code with tests
4. Ensure all tests pass
5. Submit a pull request

### Improve Documentation

Documentation improvements are always welcome:

- Fix typos or unclear explanations
- Add examples
- Update outdated information
- Translate to other languages

## Development Philosophy

Hats follows the [Six Tenets](../concepts/tenets.md):

1. **Fresh Context Is Reliability**
2. **Backpressure Over Prescription**
3. **The Plan Is Disposable**
4. **Disk Is State, Git Is Memory**
5. **Steer With Signals, Not Scripts**
6. **Let Hats Hats**

Contributions should align with these principles.

## Anti-Patterns to Avoid

From the Hats philosophy:

- Building features into orchestrator that agents can handle
- Complex retry logic (fresh context handles recovery)
- Detailed step-by-step instructions (use backpressure instead)
- Scoping work at task selection time (scope at plan creation)
- Assuming functionality is missing without code verification

## Code of Conduct

Be respectful and constructive. We're all here to make Hats better.

## Getting Help

- [GitHub Discussions](https://github.com/mikeyobrien/hats/discussions)
- [Issue Tracker](https://github.com/mikeyobrien/hats/issues)
