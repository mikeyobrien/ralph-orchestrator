# Submitting Pull Requests

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive PR guidelines.

## Overview

Guidelines for submitting pull requests to Hats.

## Before Submitting

1. **Run tests**: `cargo test`
2. **Check formatting**: `cargo fmt --check`
3. **Run clippy**: `cargo clippy`
4. **Update documentation** if needed

## PR Checklist

- [ ] Tests pass locally
- [ ] Code follows style guide
- [ ] Documentation updated
- [ ] Commit messages are clear
- [ ] PR description explains changes

## PR Template

```markdown
## Summary
Brief description of changes

## Test Plan
How to verify the changes work

## Related Issues
Fixes #123
```

## See Also

- [Development Setup](setup.md) - Environment setup
- [Code Style](style.md) - Style guidelines
- [Testing](testing.md) - Test guidelines
