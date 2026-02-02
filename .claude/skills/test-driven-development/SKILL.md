---
name: test-driven-development
description: Unified TDD skill with three input modes — from spec, from task, or from description. Enforces test-first development using repository patterns, with proptest guidance and backpressure integration.
type: anthropic-skill
version: "1.0"
---

# Test-Driven Development

## Overview

One skill for all TDD workflows. Enforces test-first development using existing repository patterns. Three input modes handle different entry points — specs, task files, or ad-hoc descriptions — but the core cycle is always RED → GREEN → REFACTOR.

## Input Modes

Detect the input type and follow the corresponding mode:

### Mode A: From Spec (`.spec.md`)

Use when the input references a `.spec.md` file with Given/When/Then acceptance criteria.

1. **Locate and parse** the spec file — extract all Given/When/Then triples
2. **Generate one test stub per criterion** with `todo!()` bodies:
   ```rust
   /// Spec: <spec-file> — Criterion #<N>
   /// Given <given text>
   /// When <when text>
   /// Then <then text>
   #[test]
   fn <spec_name>_criterion_<N>_<slug>() {
       todo!("Implement: <then text>");
   }
   ```
3. **Verify stubs compile** but fail: `cargo test --no-run -p <crate>`
4. Proceed to the [TDD Cycle](#tdd-cycle) to make stubs pass

**Programmatic support:** `ralph_core::preflight::{extract_acceptance_criteria, extract_criteria_from_file, extract_all_criteria}` can parse criteria from spec files.

### Mode B: From Task (`.code-task.md`)

Use when the input references a `.code-task.md` file or a specific implementation task.

1. **Read the task** and identify acceptance criteria or requirements
2. **Discover patterns** (see [Pattern Discovery](#pattern-discovery))
3. **Design test scenarios** covering normal operation, edge cases, and error conditions
4. **Write failing tests** for all requirements before any implementation
5. Proceed to the [TDD Cycle](#tdd-cycle)

### Mode C: From Description

Use for ad-hoc tasks without a spec or task file.

1. **Clarify requirements** from the description
2. **Discover patterns** (see [Pattern Discovery](#pattern-discovery))
3. **Write failing tests** targeting the described behavior
4. Proceed to the [TDD Cycle](#tdd-cycle)

## Pattern Discovery

Before writing tests, discover existing conventions:

```bash
rg --files -g "crates/*/tests/*.rs"
rg -n "#\[cfg\(test\)\]" crates/
```

Read 2-3 relevant test files near the target code. Mirror:
- Test module layout, naming, and assertion style
- Fixture helpers and test utilities
- Use of `tempfile`, scenarios, or harnesses

## TDD Cycle

### 1) RED — Failing Tests

- Write tests for the exact behavior required
- Run tests to confirm failure **for the right reason**
- If tests pass without implementation, the test is wrong

### 2) GREEN — Minimal Implementation

- Write the minimum code to make tests pass
- No extra features or refactoring during this step

### 3) REFACTOR — Clean Up

- Improve implementation and tests while keeping tests green
- Align with surrounding codebase conventions
- Re-run tests after every change

## Proptest Guidance

Use `proptest` only when ALL of:
- Function is pure (no I/O, no time, no globals)
- Deterministic output for given input
- Non-trivial input space or edge cases

```rust
proptest! {
    #[test]
    fn round_trip(input in "[a-z0-9]{0,32}") {
        let encoded = encode(input.as_str());
        let decoded = decode(&encoded).expect("should decode");
        prop_assert_eq!(decoded, input);
    }
}
```

Don't introduce proptest as a new dependency without strong justification.

## Backpressure Integration

Include coverage evidence in completion events:

```bash
ralph emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass (82%)"
```

Run `cargo tarpaulin --out Html --output-dir coverage --skip-clean` when feasible. If coverage cannot be run, state why and include targeted test evidence instead.

## Test Location Rules

- Spec maps to a single module → inline `#[cfg(test)]` tests
- Spec spans multiple modules → integration test in `crates/<crate>/tests/`
- CLI behavior → `crates/ralph-cli/tests/`
- Follow existing patterns in the target crate

## Anti-Patterns

- Writing implementation before tests
- Generating tests that pass without implementation
- Copying tests from other crates without adapting to local patterns
- Adding proptest when a simple example test suffices
- Emitting completion events without coverage evidence
