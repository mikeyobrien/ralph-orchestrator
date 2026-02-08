# ROP Self-Improvement Prompt

**Meta-prompt for AI-assisted self-improvement of ralph-orchestrator's validation layer**

## Version
v1

## Purpose
Enable ralph-orchestrator to analyze functional test results from ANY testing framework and generate improvements to its own validation capabilities. AI building AI.

## Key Features
- Framework-agnostic: Works with Puppeteer, Playwright, iOS simulator, pytest, Jest, any MCP tool
- Pattern recognition: Detects failure patterns, loops, flaky tests
- Code generation: Outputs valid Python improvements to safety.py/orchestrator.py
- Self-applicable: Changes integrate directly into current codebase

## Input
Test results in XML structure or raw framework output (auto-parsed)

## Output
- Analysis report with failure patterns
- Python code for new validation gates
- Implementation plan with priorities
- Confidence assessment

## Usage
```python
test_results = await run_functional_tests(framework="any")
improvements = await ai.execute("001-rop-self-improvement-do", context=test_results)
await apply_improvements(improvements)
```

## Next Step
Run functional tests and invoke this prompt to bootstrap validation improvements
