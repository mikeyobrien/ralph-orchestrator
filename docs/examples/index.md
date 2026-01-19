# Examples

Learn how to use Ralph Orchestrator through practical examples.

## Quick Examples

### Hello World

The simplest possible Ralph task:

```markdown
# PROMPT.md
Write a Python function that prints "Hello, World!"
Save it to hello.py. The orchestrator will continue iterations until completion.
```

Run with:
```bash
ralph run --prompt PROMPT.md --max-iterations 5
```

### Basic Math Function

Generate a calculator module:

```markdown
# PROMPT.md
Create a Python calculator module with:
- Functions for add, subtract, multiply, divide
- Error handling for division by zero
- Docstrings for all functions
- Save to calculator.py

The orchestrator will continue iterations until complete.
```

## Complete Examples

Explore our detailed example guides:

### 📝 [Simple Task](simple-task.md)
Build a command-line todo list application with file persistence.

### 🌐 [Web API](web-api.md)
Create a RESTful API with Flask, including authentication and database integration.

### 🛠️ [CLI Tool](cli-tool.md)
Develop a feature-rich command-line tool with argument parsing and configuration.

### 📊 [Data Analysis](data-analysis.md)
Process CSV data, generate statistics, and create visualizations.

## Example Categories

### Code Generation

**Use Case**: Automatically generate boilerplate code, utilities, or entire modules.

```markdown
Create a Python logging utility with:
- Colored console output
- File rotation
- JSON formatting option
- Multiple log levels
```

### Testing

**Use Case**: Generate comprehensive test suites for existing code.

```markdown
Write pytest tests for the user_auth.py module:
- Test all public functions
- Include edge cases
- Mock external dependencies
- Aim for 100% coverage
```

### Documentation

**Use Case**: Create or update project documentation.

```markdown
Generate comprehensive API documentation for this project:
- Document all public classes and functions
- Include usage examples
- Create a getting started guide
- Format as Markdown
```

### Refactoring

**Use Case**: Improve code quality and structure.

```markdown
Refactor the data_processor.py file:
- Split large functions (>50 lines)
- Extract common patterns
- Add type hints
- Improve variable names
- Maintain functionality
```

### Bug Fixing

**Use Case**: Identify and fix issues in code.

```markdown
Debug and fix the payment processing module:
- The calculate_tax() function returns wrong values
- Payment status isn't updating correctly
- Add logging to trace the issue
- Write tests to prevent regression
```

### Data Processing

**Use Case**: Transform and analyze data files.

```markdown
Process sales_data.csv:
- Clean missing values
- Calculate monthly totals
- Find top 10 products
- Generate summary statistics
- Export results to report.json
```

## Best Practices for Examples

### 1. Clear Objectives

Always specify exactly what you want:

✅ **Good**:
```markdown
Create a REST API endpoint that:
- Accepts POST requests to /api/users
- Validates email and password
- Returns JWT token on success
- Uses SQLite for storage
```

❌ **Bad**:
```markdown
Make a user API
```

### 2. Include Constraints

Specify limitations and requirements:

```markdown
Build a web scraper that:
- Uses only standard library (no pip installs)
- Respects robots.txt
- Implements rate limiting (1 request/second)
- Handles errors gracefully
```

### 3. Define Success Criteria

Make completion conditions explicit:

```markdown
Task is complete when:
1. All tests pass (run: pytest test_calculator.py)
2. Code follows PEP 8 (run: flake8 calculator.py)
3. Documentation is complete
4. All completion criteria are met
```

### 4. Provide Context

Include relevant information:

```markdown
Context: We're building a microservice for order processing.
Existing files: models.py, database.py

Create an order validation module that:
- Integrates with existing models
- Validates against business rules
- Returns detailed error messages
```

## Running Examples

### Basic Execution

```bash
# Run with default settings
ralph run --prompt examples/simple-task.md
```

### With Cost Limits

```bash
# Limit spending
ralph run \
  --prompt examples/web-api.md \
  --max-cost 5.0 \
  --max-tokens 100000
```

### Using Specific Agents

```bash
# Use Claude for complex tasks
ralph run \
  --agent claude \
  --prompt examples/cli-tool.md

# Use Gemini for research tasks
ralph run \
  --agent gemini \
  --prompt examples/data-analysis.md
```

### Development Mode

```bash
# Verbose output with frequent checkpoints
ralph run \
  --prompt examples/simple-task.md \
  --verbose \
  --checkpoint-interval 1 \
  --max-iterations 10
```

## Example Prompt Templates

### Web Application

```markdown
# Task: Create [Application Name]

## Requirements
- Framework: [Flask/FastAPI/Django]
- Database: [SQLite/PostgreSQL/MongoDB]
- Authentication: [JWT/Session/OAuth]

## Features
1. [Feature 1]
2. [Feature 2]
3. [Feature 3]

## File Structure
```
project/
├── app.py
├── models.py
├── routes.py
└── tests/
```

## Completion Criteria
- All endpoints working
- Tests passing
- Documentation complete
- All criteria completed
```

### Data Processing

```markdown
# Task: Process [Data Description]

## Input
- File: [filename.csv]
- Format: [CSV/JSON/XML]
- Size: [approximate size]

## Processing Steps
1. [Step 1: Load and validate]
2. [Step 2: Clean and transform]
3. [Step 3: Analyze]
4. [Step 4: Export results]

## Output
- Format: [JSON/CSV/Report]
- Include: [metrics, visualizations, etc.]

## Success Criteria
- No errors during processing
- Output validates against schema
- Performance: < [X] seconds
- All criteria completed
```

### CLI Tool

```markdown
# Task: Build [Tool Name] CLI

## Commands
- `tool command1` - [description]
- `tool command2` - [description]

## Options
- `--option1` - [description]
- `--option2` - [description]

## Requirements
- Argument parsing with argparse
- Configuration file support
- Colored output
- Progress bars for long operations

## Examples
```bash
tool process --input file.txt --output result.json
tool analyze --verbose
```

## Completion
- All commands working
- Help text complete
- Error handling robust
- All criteria completed
```

## Learning from Examples

### Study the Patterns

1. **Prompt Structure**: How successful prompts are organized
2. **Iteration Counts**: Typical iterations for different task types
3. **Token Usage**: Costs for various complexities
4. **Completion Time**: Expected runtime for tasks

### Experiment

1. Start with provided examples
2. Modify them for your needs
3. Compare different approaches
4. Share successful patterns

## Contributing Examples

Have a great example? Share it:

1. Create a new example file
2. Document the use case
3. Include expected results
4. Submit a pull request

## Troubleshooting Examples

### Task Not Completing

If examples run indefinitely:
- Check completion criteria clarity
- Verify agent can modify files
- Review iteration logs
- Adjust max iterations

### High Costs

If examples are expensive:
- Use simpler prompts
- Set token limits
- Choose appropriate agents
- Enable context management

### Poor Results

If output quality is low:
- Provide more context
- Include examples in prompt
- Specify constraints clearly
- Use more capable agents

## Next Steps

- Try the [Simple Task Example](simple-task.md)
- Explore [Web API Example](web-api.md)
- Build a [CLI Tool](cli-tool.md)
- Analyze [Data](data-analysis.md)

---

📚 Continue to [Simple Task Example](simple-task.md) →