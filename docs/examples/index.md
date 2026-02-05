# Examples

Practical examples showing Hats in action.

## In This Section

| Example | Description |
|---------|-------------|
| [Simple Task](simple-task.md) | Basic traditional mode usage |
| [TDD Workflow](tdd-workflow.md) | Test-driven development with hats |
| [Spec-Driven Development](spec-driven.md) | Specification-first approach |
| [Multi-Hat Workflow](multi-hat.md) | Complex coordination between hats |
| [Debugging](debugging.md) | Using Hats to investigate bugs |

## Quick Examples

### Traditional Mode

Simple loop until completion:

```bash
hats init --backend claude

cat > PROMPT.md << 'EOF'
Write a function that calculates factorial.
Include tests.
EOF

hats run
```

### Hat-Based Mode

Using the TDD preset:

```bash
hats init --preset tdd-red-green

cat > PROMPT.md << 'EOF'
Implement a URL validator function.
Must handle:
- HTTP and HTTPS protocols
- IPv4 addresses
- Domain names
- Port numbers
EOF

hats run
```

### Inline Prompts

Skip the prompt file:

```bash
hats run -p "Add input validation to the signup form"
```

### Custom Configuration

Override defaults:

```bash
hats run --max-iterations 50 -p "Refactor the authentication module"
```

## Example Workflows

### Feature Development

```bash
# Initialize with feature preset
hats init --preset feature

# Create detailed prompt
cat > PROMPT.md << 'EOF'
# Feature: User Dashboard

Add a user dashboard with:
- Profile summary widget
- Recent activity feed
- Quick action buttons

Use React components.
Follow existing UI patterns.
EOF

# Run Hats
hats run
```

### Bug Investigation

```bash
# Initialize with debug preset
hats init --preset debug

# Describe the bug
hats run -p "Users report login fails on Safari. Error: 'Invalid token'. Investigate and fix."
```

### Code Review

```bash
# Initialize with review preset
hats init --preset review

# Review specific files
hats run -p "Review the changes in src/api/auth.rs for security issues"
```

## Full Examples

Detailed walkthroughs are available:

- [Simple Task](simple-task.md) — Step-by-step traditional mode
- [TDD Workflow](tdd-workflow.md) — Red-green-refactor with hats
- [Spec-Driven](spec-driven.md) — Specification to implementation
- [Multi-Hat](multi-hat.md) — Complex hat coordination
- [Debugging](debugging.md) — Bug investigation workflow
