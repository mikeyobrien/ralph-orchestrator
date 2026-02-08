# ROP Self-Improvement Prompt

## Objective

Analyze functional test results from ANY testing framework and generate improvements to ralph-orchestrator's validation layer. This prompt enables AI-assisted self-improvement - the orchestrator using AI to enhance its own capabilities.

## Context

**Source Files:**
- @src/ralph_orchestrator/safety.py - Current SafetyGuard implementation
- @src/ralph_orchestrator/orchestrator.py - Core loop where validation gates execute

**Testing Framework Agnosticism:**
This prompt works with ANY functional testing output:
- iOS Simulator (xc-mcp, simctl)
- Puppeteer (browser automation)
- Playwright (cross-browser testing)
- pytest/unittest (Python tests)
- Jest/Mocha (JavaScript tests)
- Custom MCP tools
- Any stdout/stderr from validation commands

## Input Format

You will receive test results in this structure:

```xml
<test_results>
  <source>{framework_name}</source>
  <timestamp>{ISO8601}</timestamp>
  <summary>
    <total>{count}</total>
    <passed>{count}</passed>
    <failed>{count}</failed>
    <skipped>{count}</skipped>
  </summary>
  <results>
    <test name="{test_name}" status="passed|failed|skipped">
      <duration>{seconds}</duration>
      <output>{stdout/stderr}</output>
      <error>{error_message_if_failed}</error>
    </test>
    <!-- ... more tests ... -->
  </results>
  <raw_output>{original_framework_output}</raw_output>
</test_results>
```

If raw output doesn't match this structure, parse it intelligently based on common patterns:
- `PASS`/`FAIL`/`OK`/`ERROR` keywords
- Exit codes (0 = success, non-zero = failure)
- Stack traces indicate failures
- Duration patterns (`0.5s`, `500ms`, `took 2 seconds`)

## Requirements

### Phase 1: Analysis

1. **Parse Test Results**
   - Extract pass/fail/skip counts
   - Identify failure patterns (same test failing repeatedly, cascading failures)
   - Detect flaky tests (intermittent pass/fail)
   - Calculate success rate

2. **Pattern Recognition**
   - Group failures by category (timeout, assertion, crash, resource)
   - Identify root causes vs symptoms
   - Detect loops (same failure 3+ times)
   - Find improvement opportunities

3. **Correlation Analysis**
   - Map failures to orchestration iterations
   - Identify which agent actions preceded failures
   - Track failure trends across sessions

### Phase 2: Improvement Generation

Based on analysis, generate specific improvements:

1. **Validation Gates**
   ```python
   # Template for new validation check
   def check_{pattern_name}(self, context: dict) -> ValidationResult:
       """
       Detects: {what_it_detects}
       Triggered by: {test_pattern_that_revealed_need}
       """
       # Implementation
       pass
   ```

2. **Loop Detection Enhancements**
   - New similarity patterns for detect_loop()
   - Framework-specific loop signatures
   - Threshold adjustments based on observed patterns

3. **Safety Threshold Updates**
   - Recommended adjustments to max_iterations, max_runtime, max_cost
   - Evidence-based rationale from test results

4. **Recovery Strategies**
   - Suggested actions when specific failure patterns detected
   - Rollback triggers
   - Retry strategies with backoff

### Phase 3: Self-Application

Generate a concrete implementation plan:

1. **Changes to safety.py**
   - New validation methods
   - Enhanced loop detection
   - Updated thresholds

2. **Changes to orchestrator.py**
   - Integration points for new validators
   - Recovery action hooks

3. **New validation.py module (if needed)**
   - Agnostic ValidationResult protocol
   - Framework-specific adapters

## Output Format

Save output to: `.prompts/001-rop-self-improvement-do/rop-self-improvement.md`

```markdown
# ROP Self-Improvement Analysis

## Executive Summary
{2-3 sentences on key findings and recommended improvements}

## Test Results Analysis

### Source: {framework_name}
- Total: {n} | Passed: {n} | Failed: {n} | Skipped: {n}
- Success Rate: {percentage}%

### Failure Patterns Detected
| Pattern | Count | Severity | Root Cause |
|---------|-------|----------|------------|
| {name}  | {n}   | {H/M/L}  | {cause}    |

### Loop Detection
- Repeated failures: {yes/no}
- Loop signature: {pattern if detected}

## Recommended Improvements

### 1. New Validation Gate: {name}
**Purpose:** {what it prevents}
**Evidence:** {test failures that revealed need}

```python
{implementation}
```

### 2. {Additional improvements...}

## Implementation Plan

### Immediate (This Session)
1. {action}
2. {action}

### Follow-up (Next Session)
1. {action}

## Confidence Assessment
- Analysis confidence: {percentage}%
- Implementation risk: {low/medium/high}
- Estimated improvement: {percentage}% reduction in {metric}

## Metadata
<confidence>0.{XX}</confidence>
<dependencies>
  - {dependency}
</dependencies>
<open_questions>
  - {question}
</open_questions>
<assumptions>
  - {assumption}
</assumptions>
```

## Success Criteria

1. **Analysis Complete**: All test results parsed and patterns identified
2. **Improvements Actionable**: Generated code is syntactically valid Python
3. **Evidence-Based**: Every recommendation links to specific test failures
4. **Self-Applicable**: Changes can be applied to current ralph-orchestrator codebase
5. **Framework Agnostic**: Works regardless of which testing tool produced results

## SUMMARY.md Requirement

Create `.prompts/001-rop-self-improvement-do/SUMMARY.md`:

```markdown
# ROP Self-Improvement Summary

**{one-line substantive description of improvements identified}**

## Version
v1

## Key Findings
- {finding 1}
- {finding 2}
- {finding 3}

## Improvements Generated
- {improvement 1}: {brief description}
- {improvement 2}: {brief description}

## Decisions Needed
- {decision if any}

## Blockers
- {blocker if any}

## Next Step
{concrete next action}
```

## Usage

This prompt is invoked by ralph-orchestrator after running functional tests:

```python
# In orchestrator loop
test_results = await run_functional_tests(framework="playwright")
improvement_prompt = load_prompt("001-rop-self-improvement-do")
improvements = await ai.execute(improvement_prompt, context=test_results)
await apply_improvements(improvements)
```

The orchestrator improves itself through this feedback loop.
