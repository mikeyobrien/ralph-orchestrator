# Examples

Practical examples showing Ralph in action.

## In This Section

| Example | Description |
|---------|-------------|
| [Simple Task](simple-task.md) | Basic traditional mode usage |
| [TDD Workflow](tdd-workflow.md) | Test-driven development with hats |
| [Automated PDD Design](pdd-design.md) | Example-only design workflow with simulated requirements interview |
| [Spec-Driven Development](spec-driven.md) | Example-only workflow pattern, not a shipped builtin |
| [Multi-Hat Workflow](multi-hat.md) | Complex coordination between hats |
| [Debugging](debugging.md) | Using Ralph to investigate bugs |

## Quick Examples

### Traditional Mode

Simple loop until completion:

```bash
ralph init --backend claude

cat > PROMPT.md << 'EOF'
Write a function that calculates factorial.
Include tests.
EOF

ralph run
```

### Hat-Based Mode

Using a built-in hat collection:

```bash
ralph init --backend claude

cat > PROMPT.md << 'EOF'
Implement a URL validator function.
Must handle:
- HTTP and HTTPS protocols
- IPv4 addresses
- Domain names
- Port numbers
EOF

ralph run -c ralph.yml -H builtin:code-assist
```

### Inline Prompts

Skip the prompt file:

```bash
ralph run -p "Add input validation to the signup form"
```

### Custom Configuration

Override defaults:

```bash
ralph run --max-iterations 50 -p "Refactor the authentication module"
```

## Example Workflows

### Feature Development

```bash
# Initialize core config
ralph init --backend claude

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

# Run Ralph with the default implementation hats
ralph run -c ralph.yml -H builtin:code-assist
```

### Bug Investigation

```bash
# Use debug hat collection
ralph run -c ralph.yml -H builtin:debug -p "Users report login fails on Safari. Error: 'Invalid token'. Investigate and fix."
```

### Code Review

```bash
# Use review hat collection
ralph run -c ralph.yml -H builtin:review -p "Review the changes in src/api/auth.rs for security issues"
```

## Full Examples

Detailed walkthroughs are available:

- [Simple Task](simple-task.md) — Step-by-step traditional mode
- [TDD Workflow](tdd-workflow.md) — Red-green-refactor with hats
- [Automated PDD Design](pdd-design.md) — Simulated interview that ends with a reviewed design package
- [Spec-Driven](spec-driven.md) — Example specification-first pattern
- [Multi-Hat](multi-hat.md) — Complex hat coordination
- [Debugging](debugging.md) — Bug investigation workflow
