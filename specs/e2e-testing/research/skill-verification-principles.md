# Skill Verification Principles (from superpowers/writing-skills)

## Core Philosophy: TDD for Documentation

**The Iron Law:** "NO SKILL WITHOUT A FAILING TEST FIRST"

Writing skills is Test-Driven Development applied to process documentation:
- **Test case** = Pressure scenario with subagent
- **RED (failing)** = Agent violates rule without skill (baseline)
- **GREEN (passing)** = Agent complies when skill present
- **REFACTOR** = Close loopholes while maintaining compliance

**Critical insight:** If you didn't watch an agent fail without the skill, you don't know if the skill teaches the right thing.

---

## Skill Types and Testing Strategy

| Skill Type | Examples | Test Approach | Success Criteria |
|------------|----------|---------------|------------------|
| **Discipline-Enforcing** | TDD, verification-before-completion | Pressure scenarios with combined pressures | Agent follows rule under maximum pressure |
| **Technique** | condition-based-waiting, root-cause-tracing | Application to new scenarios, edge cases | Agent successfully applies technique |
| **Pattern** | mental models, reducing-complexity | Recognition scenarios, counter-examples | Agent correctly identifies when/how to apply |
| **Reference** | API docs, command references | Retrieval scenarios, gap testing | Agent finds and correctly applies info |

---

## Application to E2E Testing

For Ralph's E2E test harness, we should verify:

### 1. **Baseline Behavior (RED)**
- Run Ralph WITHOUT specific configurations
- Document what goes wrong or doesn't work
- Identify patterns in failures

### 2. **Correct Behavior (GREEN)**
- Run Ralph WITH proper configuration
- Verify the system behaves correctly
- All features work as designed

### 3. **Loophole Closing (REFACTOR)**
- Test edge cases
- Verify error handling
- Ensure graceful degradation

---

## Validation Criteria for E2E Tests

A test is ready when:

1. **Baseline documented:** We know what failure looks like
2. **Compliance verified:** System works under normal conditions
3. **Edge cases covered:** Loopholes closed with explicit tests
4. **Prompt effectiveness validated:** Agent follows instructions correctly
5. **Backend compatibility verified:** Works across Claude, Kiro, OpenCode

---

## Key Testing Principles

1. **One excellent test beats many mediocre ones** - Focus on comprehensive scenarios
2. **Test under pressure** - Validate behavior when things go wrong
3. **Document rationalizations** - When tests fail, capture why
4. **Close loopholes explicitly** - Add tests for discovered edge cases
