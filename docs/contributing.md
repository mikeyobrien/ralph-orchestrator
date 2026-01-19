# Contributing to Ralph Orchestrator

Thank you for your interest in contributing to Ralph Orchestrator! This guide will help you get started with contributing to the project.

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/CODE_OF_CONDUCT.md). Please read it before contributing.

## Ways to Contribute

### 1. Report Bugs

Found a bug? Help us fix it:

1. **Check existing issues** to avoid duplicates
2. **Create a new issue** with:
   - Clear title and description
   - Steps to reproduce
   - Expected vs actual behavior
   - System information
   - Error messages/logs

**Bug Report Template:**
```markdown
## Description
Brief description of the bug

## Steps to Reproduce
1. Run command: `ralph run -p "..."`
2. See error

## Expected Behavior
What should happen

## Actual Behavior
What actually happens

## Environment
- OS: [e.g., Ubuntu 22.04]
- Rust: [e.g., 1.75.0]
- Ralph Version: [e.g., 2.0.0]
- AI Agent: [e.g., claude]

## Logs
```
Error messages here
```
```

### 2. Suggest Features

Have an idea? We'd love to hear it:

1. **Check existing feature requests**
2. **Open a discussion** for major changes
3. **Create a feature request** with:
   - Use case description
   - Proposed solution
   - Alternative approaches
   - Implementation considerations

### 3. Improve Documentation

Documentation improvements are always welcome:

- Fix typos and grammar
- Clarify confusing sections
- Add missing information
- Create new examples
- Translate documentation

### 4. Contribute Code

Ready to code? Follow these steps:

#### Setup Development Environment

```bash
# Fork and clone the repository
git clone https://github.com/YOUR_USERNAME/ralph-orchestrator.git
cd ralph-orchestrator

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Build the project
cargo build

# Run tests
cargo test

# Install pre-commit hooks
./scripts/setup-hooks.sh
```

#### Development Workflow

1. **Create a branch**
   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b fix/issue-number
   ```

2. **Make changes**
   - Follow existing code style
   - Add/update tests
   - Update documentation

3. **Test your changes**
   ```bash
   # Run all tests
   cargo test

   # Run specific test
   cargo test test_function_name

   # Run smoke tests
   cargo test -p ralph-core smoke_runner
   ```

4. **Format and lint**
   ```bash
   # Format with rustfmt
   cargo fmt

   # Lint with clippy
   cargo clippy
   ```

5. **Commit changes**
   ```bash
   git add .
   git commit -m "feat: add new feature"
   # Use conventional commits: feat, fix, docs, test, refactor, style, chore
   ```

6. **Push and create PR**
   ```bash
   git push origin feature/your-feature-name
   ```

## Development Guidelines

### Code Style

We follow Rust conventions with these preferences:

- **Line length**: 100 characters
- **Use clippy**: All warnings should be addressed
- **Type safety**: Prefer strong typing over dynamic types
- **Error handling**: Use `Result` and `?` operator appropriately
- **Documentation**: Document public APIs with rustdoc

**Example:**
```rust
/// Calculate token usage cost.
///
/// # Arguments
///
/// * `input_tokens` - Number of input tokens
/// * `output_tokens` - Number of output tokens
/// * `agent_type` - Type of AI agent
///
/// # Returns
///
/// Cost in USD
///
/// # Errors
///
/// Returns an error if the agent type is unknown
pub fn calculate_cost(
    input_tokens: u64,
    output_tokens: u64,
    agent_type: &str,
) -> Result<f64, AgentError> {
    let rates = TOKEN_COSTS
        .get(agent_type)
        .ok_or_else(|| AgentError::UnknownAgent(agent_type.to_string()))?;

    let cost = (input_tokens as f64 * rates.input +
                output_tokens as f64 * rates.output) / 1_000_000.0;
    Ok((cost * 10000.0).round() / 10000.0)
}
```

### Testing Guidelines

All new features require tests:

1. **Unit tests** for individual functions
2. **Integration tests** for workflows
3. **Edge cases** and error conditions
4. **Documentation** of test purpose

**Test Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost() {
        // Test Claude pricing
        let cost = calculate_cost(1000, 500, "claude").unwrap();
        assert!((cost - 0.0105).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_invalid_agent() {
        // Test invalid agent
        let result = calculate_cost(1000, 500, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        // Test edge case: zero tokens
        let cost = calculate_cost(0, 0, "claude").unwrap();
        assert_eq!(cost, 0.0);
    }
}
```

### Commit Message Convention

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `test:` Test additions/changes
- `refactor:` Code refactoring
- `style:` Code style changes
- `chore:` Maintenance tasks
- `perf:` Performance improvements

**Examples:**
```bash
feat: add Kiro backend support
fix: resolve token overflow in long prompts
docs: update installation guide for Windows
test: add integration tests for checkpointing
refactor: extract prompt validation logic
```

### Pull Request Process

1. **Title**: Use conventional commit format
2. **Description**: Explain what and why
3. **Testing**: Describe testing performed
4. **Screenshots**: Include if UI changes
5. **Checklist**: Complete PR template

**PR Template:**
```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation update
- [ ] Performance improvement

## Testing
- [ ] All tests pass
- [ ] Added new tests
- [ ] Manual testing performed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-reviewed code
- [ ] Updated documentation
- [ ] No breaking changes
```

## Project Structure

```
ralph-orchestrator/
├── crates/
│   ├── ralph-core/       # Core orchestration logic
│   ├── ralph-tui/        # Terminal UI
│   └── ralph-cli/        # CLI entry point
├── src/                  # Main binary
├── tests/                # Integration tests
├── docs/                 # Documentation
├── examples/             # Example configs
└── .github/              # GitHub configs
```

## Testing

### Run Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p ralph-core

# Smoke tests (replay-based)
cargo test -p ralph-core smoke_runner

# Verbose output
cargo test -- --nocapture

# Stop on first failure
cargo test -- --test-threads=1
```

### Test Categories

1. **Unit Tests**: Test individual functions
2. **Integration Tests**: Test component interaction
3. **Smoke Tests**: Replay-based tests using recorded fixtures
4. **Performance Tests**: Test resource usage

## Documentation

### Building Docs Locally

```bash
# Install MkDocs
pip install mkdocs mkdocs-material

# Serve locally
mkdocs serve

# Build static site
mkdocs build
```

### Documentation Standards

- Clear, concise language
- Code examples for all features
- Explain the "why" not just "how"
- Keep examples up-to-date
- Include troubleshooting tips

## Release Process

1. **Version Bump**: Update version in `Cargo.toml`
2. **Changelog**: Update CHANGELOG.md
3. **Tests**: Ensure all tests pass
4. **Documentation**: Update if needed
5. **Tag**: Create version tag
6. **Release**: Create GitHub release

## Getting Help

### For Contributors

- 💬 [Discord Server](https://discord.gg/ralph-orchestrator)
- 📧 [Email Maintainers](mailto:maintainers@ralph-orchestrator.dev)
- 🗣️ [GitHub Discussions](https://github.com/mikeyobrien/ralph-orchestrator/discussions)

### Resources

- [Architecture Overview](advanced/architecture.md)
- [API Documentation](api/orchestrator.md)
- [Testing Guide](testing.md)

## Recognition

Contributors are recognized in:

- [CONTRIBUTORS.md](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/CONTRIBUTORS.md)
- Release notes
- Documentation credits

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to Ralph Orchestrator!
