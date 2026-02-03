# Prompt Engineering Guide

Effective prompt engineering is crucial for successful Ralph Orchestrator tasks. This guide covers best practices, patterns, and techniques for writing prompts that get results.

## Prompt File Basics

### File Format

Ralph Orchestrator uses Markdown files for prompts:

```markdown
# Task Title

## Objective
Clear description of what needs to be accomplished.

## Requirements
- Specific requirement 1
- Specific requirement 2

## Success Criteria
The task is complete when:
- Criterion 1 is met
- Criterion 2 is met

The orchestrator will run until iteration/time/cost limits are reached.
```

### File Location

Default prompt file: `PROMPT.md`

Custom location:
```bash
python ralph_orchestrator.py --prompt path/to/task.md
```

## Prompt Structure

### Essential Components

Every prompt should include:

1. **Clear Objective**
2. **Specific Requirements**
3. **Success Criteria**
4. **Completion Marker**

### Template

```markdown
# [Task Name]

## Objective
[One or two sentences describing the goal]

## Context
[Background information the agent needs]

## Requirements
1. [Specific requirement]
2. [Specific requirement]
3. [Specific requirement]

## Constraints
- [Limitation or boundary]
- [Technical constraint]
- [Resource constraint]

## Success Criteria
The task is complete when:
- [ ] [Measurable outcome]
- [ ] [Verifiable result]
- [ ] [Specific deliverable]

## Notes
[Additional guidance or hints]

---
The orchestrator will continue iterations until limits are reached.
```

## Prompt Patterns

### 1. Software Development Pattern

```markdown
# Build Web API

## Objective
Create a RESTful API for user management with authentication.

## Requirements
1. Implement user CRUD operations
2. Add JWT authentication
3. Include input validation
4. Write comprehensive tests
5. Create API documentation

## Technical Specifications
- Framework: FastAPI
- Database: PostgreSQL
- Authentication: JWT tokens
- Testing: pytest

## Endpoints
- POST /auth/register
- POST /auth/login
- GET /users
- GET /users/{id}
- PUT /users/{id}
- DELETE /users/{id}

## Success Criteria
- [ ] All endpoints functional
- [ ] Tests passing with >80% coverage
- [ ] API documentation generated
- [ ] Authentication working

The orchestrator will run until completion criteria are met or limits reached.
```

### 2. Documentation Pattern

````markdown
# Create User Documentation

## Objective
Write comprehensive user documentation for the application.

## Requirements
1. Installation guide
2. Configuration reference
3. Usage examples
4. Troubleshooting section
5. FAQ

## Structure
```
docs/
├── getting-started.md
├── installation.md
├── configuration.md
├── usage/
│   ├── basic.md
│   └── advanced.md
├── troubleshooting.md
└── faq.md
```

## Style Guide
- Use clear, concise language
- Include code examples
- Add screenshots where helpful
- Follow Markdown best practices

## Success Criteria
- [ ] All sections complete
- [ ] Examples tested and working
- [ ] Reviewed for clarity
- [ ] No broken links

The orchestrator will continue iterations until limits are reached.
````

### 3. Data Analysis Pattern

```markdown
# Analyze Sales Data

## Objective
Analyze Q4 sales data and generate insights report.

## Data Sources
- sales_data.csv
- customer_demographics.json
- product_catalog.xlsx

## Analysis Requirements
1. Revenue trends by month
2. Top performing products
3. Customer segmentation
4. Regional performance
5. Year-over-year comparison

## Deliverables
1. Python analysis script
2. Jupyter notebook with visualizations
3. Executive summary (PDF)
4. Raw data exports

## Success Criteria
- [ ] All analyses complete
- [ ] Visualizations created
- [ ] Insights documented
- [ ] Code reproducible

The orchestrator will run until limits are reached.
```

### 4. Debugging Pattern

```markdown
# Debug Application Issue

## Problem Description
Users report application crashes when uploading large files.

## Symptoms
- Crash occurs with files >100MB
- Error: "Memory allocation failed"
- Affects 30% of users

## Investigation Steps
1. Reproduce the issue
2. Analyze memory usage
3. Review upload handling code
4. Check server resources
5. Examine error logs

## Required Fixes
- Identify root cause
- Implement solution
- Add error handling
- Write regression tests
- Update documentation

## Success Criteria
- [ ] Issue reproduced
- [ ] Root cause identified
- [ ] Fix implemented
- [ ] Tests passing
- [ ] No regressions

The orchestrator will continue verification iterations until limits are reached.
```

## Best Practices

### 1. Be Specific

❌ **Bad:**
```markdown
Build a website
```

✅ **Good:**
```markdown
Build a responsive e-commerce website using React and Node.js with:
- Product catalog with search
- Shopping cart functionality
- Stripe payment integration
- User authentication
- Order tracking
```

### 2. Provide Context

❌ **Bad:**
```markdown
Fix the bug
```

✅ **Good:**
```markdown
Fix the memory leak in the image processing module that occurs when:
- Processing images larger than 10MB
- Multiple images are processed simultaneously
- The cleanup function in ImageProcessor.process() may not be releasing buffers
```

### 3. Define Success Clearly

❌ **Bad:**
```markdown
Make it work better
```

✅ **Good:**
```markdown
## Success Criteria
- Response time < 200ms for 95% of requests
- Memory usage stays below 512MB
- All unit tests pass
- No errors in 24-hour stress test
```

### 4. Include Examples

```markdown
## Example Input/Output

Input:
```json
{
  "user_id": 123,
  "action": "purchase",
  "items": ["SKU-001", "SKU-002"]
}
```

Expected Output:
```json
{
  "order_id": "ORD-789",
  "status": "confirmed",
  "total": 99.99,
  "estimated_delivery": "2024-01-15"
}
```
```

### 5. Specify Constraints

```markdown
## Constraints
- Must be Python 3.8+ compatible
- Cannot use external APIs
- Must complete in under 5 seconds
- Memory usage < 1GB
- Must follow PEP 8 style guide
```

## Iterative Prompts

Ralph Orchestrator modifies the prompt file during execution. Design prompts that support iteration:

### Self-Documenting Progress

```markdown
## Progress Log
<!-- Agent will update this section -->
- [ ] Step 1: Setup environment
- [ ] Step 2: Implement core logic
- [ ] Step 3: Add tests
- [ ] Step 4: Documentation

## Current Status
<!-- Agent updates this -->
Working on: [current task]
Completed: [list of completed items]
Next: [planned next step]
```

### Checkpoint Markers

```markdown
## Checkpoints
- [ ] CHECKPOINT_1: Basic structure complete
- [ ] CHECKPOINT_2: Core functionality working
- [ ] CHECKPOINT_3: Tests passing
- [ ] CHECKPOINT_4: Documentation complete
- [ ] All criteria verified
```

## Advanced Techniques

### 1. Multi-Phase Prompts

```markdown
# Phase 1: Research
Research existing solutions and document findings.

<!-- After Phase 1 complete, update prompt for Phase 2 -->

# Phase 2: Implementation
Based on research, implement the solution.

# Phase 3: Testing
Comprehensive testing and validation.
```

### 2. Conditional Instructions

```markdown
## Implementation

If using Python:
- Use type hints
- Follow PEP 8
- Use pytest for testing

If using JavaScript:
- Use TypeScript
- Follow Airbnb style guide
- Use Jest for testing
```

### 3. Learning Prompts

```markdown
## Approach
1. First, try the simple solution
2. If that doesn't work, research alternatives
3. Document what was learned
4. Implement the best solution

## Document Learnings
<!-- Agent fills this during execution -->
- Attempted: [approach]
- Result: [outcome]
- Learning: [insight]
```

### 4. Error Recovery

```markdown
## Error Handling
If you encounter errors:
1. Document the error in this file
2. Research the solution
3. Try alternative approaches
4. Update this prompt with findings

## Error Log
<!-- Agent updates this -->
```

## Prompt Security

### Sanitization

Ralph Orchestrator automatically sanitizes prompts for:
- Command injection attempts
- Path traversal attacks
- Malicious patterns

### Safe Patterns

```markdown
## File Operations
Work only in the ./workspace directory
Do not modify system files
Create backups before changes
```

### Size Limits

Default maximum prompt size: 10MB

Adjust if needed:
```bash
python ralph_orchestrator.py --max-prompt-size 20971520  # 20MB
```

## Testing Prompts

### Dry Run

Test prompts without execution:

```bash
python ralph_orchestrator.py --dry-run --prompt test.md
```

### Limited Iterations

Test with few iterations:

```bash
python ralph_orchestrator.py --max-iterations 3 --prompt test.md
```

### Verbose Mode

Debug prompt processing:

```bash
python ralph_orchestrator.py --verbose --prompt test.md
```

## Common Pitfalls

### 1. Vague Instructions

❌ **Avoid:**
- "Make it good"
- "Optimize everything"
- "Fix all issues"

✅ **Instead:**
- "Achieve 95% test coverage"
- "Reduce response time to <100ms"
- "Fix the memory leak in process_image()"

### 2. Missing Completion Criteria

❌ **Avoid:**
Forgetting to specify when the task is done

✅ **Instead:**
Always include clear completion criteria that the orchestrator can work towards

### 3. Overly Complex Prompts

❌ **Avoid:**
Single prompt with 50+ requirements

✅ **Instead:**
Break into phases or separate tasks

### 4. No Examples

❌ **Avoid:**
Describing desired behavior without examples

✅ **Instead:**
Include input/output examples and edge cases

## Prompt Library

### Starter Templates

1. [Web API Development](../examples/web-api.md)
2. [CLI Tool Creation](../examples/cli-tool.md)
3. [Data Analysis](../examples/data-analysis.md)
4. [Documentation Writing](../examples/documentation.md)
5. [Bug Fixing](../examples/bug-fix.md)
6. [Testing Suite](../examples/testing.md)

## Next Steps

- Explore [Cost Management](cost-management.md) for efficient prompts
- Review [Agent Selection](agents.md) for optimal results
- See [Examples](../examples/index.md) for real-world prompts
