# Hat Collection Preset Evaluation Task

You are a systematic evaluator testing Ralph's hat collection presets.

## Objective

Test each of the 12 hat collection presets in `presets/` and document your findings in a single consolidated report at `tools/preset-evaluation-findings.md`.

## Presets to Evaluate

| # | Preset | Test Task |
|---|--------|-----------|
| 1 | `tdd-red-green.yml` | Add a `is_palindrome()` function to a test file |
| 2 | `adversarial-review.yml` | Review a simple user input handler for security |
| 3 | `socratic-learning.yml` | Understand how the `HatRegistry` works |
| 4 | `spec-driven.yml` | Specify and implement a `StringUtils::truncate()` function |
| 5 | `mob-programming.yml` | Implement a simple `Stack` data structure |
| 6 | `scientific-method.yml` | Debug why a mock test assertion might fail |
| 7 | `code-archaeology.yml` | Understand the history of `config.rs` |
| 8 | `performance-optimization.yml` | Profile and suggest optimizations for hat matching |
| 9 | `api-design.yml` | Design a simple `Cache` trait/interface |
| 10 | `documentation-first.yml` | Document a `RateLimiter` before implementing |
| 11 | `incident-response.yml` | Simulate responding to a "tests failing in CI" incident |
| 12 | `migration-safety.yml` | Plan migration from v1 to v2 config format |

## Evaluation Protocol

For EACH preset:

### Step 1: Setup
```bash
# Copy preset to test config
cp presets/<preset>.yml .ralph-test.yml

# Create test sandbox if needed
mkdir -p .eval-sandbox/<preset-name>
```

### Step 2: Execute
```bash
# Run ralph with the preset
ralph run -c .ralph-test.yml -p "<test task prompt>"
```

### Step 3: Observe & Document
Record in your findings:

1. **Hat Transitions**: Did events flow correctly between hats?
2. **Instructions Clarity**: Were hat instructions followed?
3. **Completion**: Did it reach LOOP_COMPLETE appropriately?
4. **Errors**: Any crashes, hangs, or unexpected behavior?
5. **Timing**: Roughly how long did each phase take?
6. **UX Friction**: What was confusing or annoying?

### Step 4: Categorize Issues

Use these categories:
- 🐛 **Bug**: Something is broken
- 🎨 **UX**: Could be more intuitive
- 📝 **Docs**: Documentation gap
- ⚡ **Perf**: Performance concern
- 💡 **Idea**: Enhancement suggestion

## Findings Document Structure

Create `tools/preset-evaluation-findings.md` with this structure:

```markdown
# Hat Collection Preset Evaluation Findings

**Evaluator**: Kiro CLI
**Date**: <timestamp>
**Ralph Version**: <version>

## Executive Summary
- Presets tested: X/12
- Working well: X
- Needs fixes: X
- Critical bugs: X

## Detailed Findings

### 1. tdd-red-green.yml

**Status**: ✅ Working | ⚠️ Partial | ❌ Broken

**Test Task**: <what you tested>

**Hat Flow**:
```
task.start → test_writer → test.written → implementer → ...
```

**What Worked**:
- ...

**Issues Found**:
- 🐛 ...
- 🎨 ...

**Recommendations**:
- ...

---

### 2. adversarial-review.yml
...
```

## Completion Criteria

You are done when:
1. All 12 presets have been tested
2. Findings are documented in `tools/preset-evaluation-findings.md`
3. You've identified at least 3 actionable improvements

Output `EVALUATION_COMPLETE` when finished.

IMPORTANT: Fix any blocking issues to complete the evaluation

## Tips

- If a preset hangs, note the last event published
- If a preset loops infinitely, note the cycle
- Test with simple tasks—we're evaluating the framework, not the LLM
- Be specific about error messages—copy them exactly
- Note which hats were actually invoked vs expected

## Environment

- Use Kiro CLI (`kiro-cli chat`)
- Work in the ralph-orchestrator-2.0 directory
- You can read preset files to understand expected behavior
- You can check `crates/ralph-core/src/hat_registry.rs` for routing logic
