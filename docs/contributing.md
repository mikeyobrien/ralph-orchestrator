# Contributing to Hats

Thank you for your interest in contributing to Hats! This guide will help you get started with contributing to the project.

## Code of Conduct

By participating in this project, you agree to abide by our [Code of Conduct](https://github.com/mikeyobrien/hats/blob/main/CODE_OF_CONDUCT.md). Please read it before contributing.

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
1. Run command: `python hats_orchestrator.py ...`
2. See error

## Expected Behavior
What should happen

## Actual Behavior
What actually happens

## Environment
- OS: [e.g., Ubuntu 22.04]
- Python: [e.g., 3.10.5]
- Hats Version: [e.g., 1.0.0]
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
git clone https://github.com/YOUR_USERNAME/hats.git
cd hats

# Create a virtual environment
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install development dependencies
pip install -e .
pip install pytest pytest-cov black ruff

# Install pre-commit hooks (optional)
pip install pre-commit
pre-commit install
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
   pytest

   # Run specific test
   pytest test_orchestrator.py::test_function

   # Check coverage
   pytest --cov=hats_orchestrator --cov-report=html
   ```

4. **Format code**
   ```bash
   # Format with black
   black hats_orchestrator.py

   # Lint with ruff
   ruff check hats_orchestrator.py
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

We follow PEP 8 with these preferences:

- **Line length**: 88 characters (Black default)
- **Quotes**: Double quotes for strings
- **Imports**: Sorted with `isort`
- **Type hints**: Use where beneficial
- **Docstrings**: Google style

**Example:**
```python
def calculate_cost(
    input_tokens: int,
    output_tokens: int,
    agent_type: str = "claude"
) -> float:
    """
    Calculate token usage cost.
    
    Args:
        input_tokens: Number of input tokens
        output_tokens: Number of output tokens
        agent_type: Type of AI agent
        
    Returns:
        Cost in USD
        
    Raises:
        ValueError: If agent_type is unknown
    """
    if agent_type not in TOKEN_COSTS:
        raise ValueError(f"Unknown agent: {agent_type}")
    
    rates = TOKEN_COSTS[agent_type]
    cost = (input_tokens * rates["input"] + 
            output_tokens * rates["output"]) / 1_000_000
    return round(cost, 4)
```

### Testing Guidelines

All new features require tests:

1. **Unit tests** for individual functions
2. **Integration tests** for workflows
3. **Edge cases** and error conditions
4. **Documentation** of test purpose

**Test Example:**
```python
def test_calculate_cost():
    """Test cost calculation for different agents."""
    # Test Claude pricing
    cost = calculate_cost(1000, 500, "claude")
    assert cost == 0.0105
    
    # Test invalid agent
    with pytest.raises(ValueError):
        calculate_cost(1000, 500, "invalid")
    
    # Test edge case: zero tokens
    cost = calculate_cost(0, 0, "claude")
    assert cost == 0.0
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
feat: add Gemini agent support
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
hats/
├── hats_orchestrator.py   # Main orchestrator
├── hats                   # CLI wrapper
├── tests/                  # Test files
│   ├── test_orchestrator.py
│   ├── test_integration.py
│   └── test_production.py
├── docs/                   # Documentation
│   ├── index.md
│   ├── guide/
│   └── api/
├── examples/               # Example prompts
├── .agent/                 # Runtime data
└── .github/               # GitHub configs
```

## Testing

### Run Tests

```bash
# All tests
pytest

# With coverage
pytest --cov=hats_orchestrator

# Specific test file
pytest test_orchestrator.py

# Verbose output
pytest -v

# Stop on first failure
pytest -x
```

### Test Categories

1. **Unit Tests**: Test individual functions
2. **Integration Tests**: Test component interaction
3. **E2E Tests**: Test complete workflows
4. **Performance Tests**: Test resource usage
5. **Security Tests**: Test input validation

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

1. **Version Bump**: Update version in code
2. **Changelog**: Update CHANGELOG.md
3. **Tests**: Ensure all tests pass
4. **Documentation**: Update if needed
5. **Tag**: Create version tag
6. **Release**: Create GitHub release

## Getting Help

### For Contributors

- 💬 [Discord Server](https://discord.gg/hats)
- 📧 [Email Maintainers](mailto:maintainers@hats.dev)
- 🗣️ [GitHub Discussions](https://github.com/mikeyobrien/hats/discussions)

### Resources

- [Development Setup Video](https://youtube.com/...)
- [Architecture Overview](advanced/architecture.md)
- [API Documentation](api/orchestrator.md)
- [Testing Guide](contributing/testing.md)

## Recognition

Contributors are recognized in:

- [CONTRIBUTORS.md](https://github.com/mikeyobrien/hats/blob/main/CONTRIBUTORS.md)
- Release notes
- Documentation credits

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to Hats! 🎉