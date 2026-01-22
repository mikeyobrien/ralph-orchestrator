# Requirements Clarification

This document captures the Q&A process to refine requirements for the E2E test harness.

---

## Q1: What is the primary goal of this test harness?

Is this test harness primarily intended to:

a) **CI/CD integration** - Run automatically on PRs/merges to catch regressions, with pass/fail gates
b) **Manual validation** - Run on-demand by developers to verify backends work before releases
c) **Both** - Automated CI runs for quick checks, plus comprehensive manual runs for deeper validation
d) **Something else** - Please describe

**Answer:** (b) Manual validation - Run on-demand by developers to verify backends work before releases.

---

## Q2: What should the test harness actually validate?

When running against a real backend, what should the harness check for?

a) **Basic connectivity** - Can we successfully send a prompt and receive a response?
b) **Full orchestration loop** - Run a complete Ralph iteration cycle (plan → execute → verify)
c) **Specific capabilities** - Test particular features like tool use, streaming, error handling
d) **All of the above** - Comprehensive validation across all levels
e) **Something else** - Please describe

**Answer:** (d) All of the above - Comprehensive validation including connectivity, full orchestration loops, and specific capability testing.

---

## Q3: How should authentication/credentials be handled?

Since this uses real backends with real API calls, how should the harness manage credentials?

a) **Environment variables** - Expect `ANTHROPIC_API_KEY`, `KIRO_API_KEY`, etc. to be pre-configured
b) **Existing CLI auth** - Leverage whatever auth the CLI tools already have configured (e.g., `claude` CLI's stored credentials)
c) **Config file** - Read credentials from a dedicated test config file (gitignored)
d) **Mixed approach** - Use CLI auth when available, fall back to env vars
e) **Something else** - Please describe

**Answer:** (b/d) Mixed approach - Leverage existing CLI auth when available (preferred), fall back to environment variables when needed.

---

## Q4: How should test results be reported?

Since this is a manual validation tool, how should results be displayed to the developer?

a) **Terminal output only** - Pretty-printed pass/fail with details shown inline
b) **Terminal + log files** - Summary in terminal, detailed logs written to files
c) **Structured report** - Generate a markdown or JSON report file for each run
d) **Interactive** - Show progress in real-time with ability to inspect/retry individual tests
e) **Something else** - Please describe

**Answer:** (a/c) Terminal output with pretty-printed pass/fail, plus structured report files (markdown or JSON) for each run.

---

## Q5: How should tests be isolated/sandboxed?

Real backend tests could have side effects (creating files, tool calls, etc.). How should the harness handle isolation?

a) **Temp directory** - Run each test in an isolated temp directory that's cleaned up after
b) **Dedicated test workspace** - Use a persistent `.e2e-tests/` workspace that can be inspected after runs
c) **Docker container** - Run tests in a containerized environment for full isolation
d) **No isolation needed** - Tests will be designed to be non-destructive, no special sandboxing required
e) **Something else** - Please describe

**Answer:** (b) Dedicated test workspace - Use a persistent `.e2e-tests/` workspace that can be inspected after runs, but gitignored to avoid committing test artifacts.

---

## Q6: How should test scenarios be defined?

How should the harness know what tests to run against each backend?

a) **Hardcoded in Rust** - Test scenarios compiled into the harness binary
b) **YAML/TOML config files** - Define test scenarios in configuration files that can be edited without recompiling
c) **Markdown test specs** - Define tests in markdown files with frontmatter (similar to existing fixtures approach)
d) **Convention-based** - Discover tests from a directory structure (e.g., `tests/e2e/claude/*.test.yml`)
e) **Something else** - Please describe

**Answer:** (a) Hardcoded in Rust - Test scenarios compiled into the harness binary for simplicity.

---

## Q7: What specific capabilities need testing per backend?

Which capabilities are essential to validate for each backend? (Select all that apply or describe)

a) **Basic prompt/response** - Send a simple prompt, get a response
b) **Streaming** - Verify streaming output works correctly
c) **Tool use** - Verify the backend can call tools and handle results
d) **Multi-turn conversation** - Verify context is maintained across turns
e) **Error handling** - Verify graceful handling of rate limits, auth failures, etc.
f) **All of the above**
g) **Something else** - Please describe

**Answer:** Research needed - Study the Ralph repo to understand its actual feature set, test prompt effectiveness, and apply principles from the superpowers/writing-skills methodology. Tests should validate Ralph-specific capabilities, not just generic backend features.

---

## Q8: Research Findings Summary

Research completed via 10 parallel agents. Key findings:

### Ralph Feature Set (to test):
1. **Backend connectivity** - Claude, Kiro, OpenCode (+ Gemini, Codex, Amp, Copilot)
2. **Orchestration loop** - Iteration progression, termination conditions, event parsing
3. **Prompt handling** - Inline, file-based, large prompt handling
4. **Event system** - XML tag parsing, EventBus routing, backpressure validation
5. **Tool use** - Tool invocation, results, NDJSON streaming (Claude-specific)
6. **State management** - Scratchpad persistence, session recording, resume

### Writing-Skills Verification Principles:
- **TDD for documentation**: Test = pressure scenario with subagent
- **RED phase**: Establish baseline (agent fails without proper config)
- **GREEN phase**: Verify correct behavior with proper config
- **REFACTOR phase**: Close loopholes, test edge cases

### Prompt Effectiveness (what to validate):
1. Agent follows core instructions (reads specs, updates scratchpad)
2. Backpressure is respected (runs tests before claiming done)
3. Events are published correctly (proper XML format)
4. Completion is accurate (LOOP_COMPLETE only when truly done)

### Existing Test Infrastructure:
- Replay-based smoke tests (JSONL fixtures) - 37+ tests
- Python E2E tests with LLM-as-judge - 15+ tests
- **Gap**: No live backend testing with real API calls

See `research/` directory for detailed findings.

---

## Requirements Summary

| Requirement | Decision |
|-------------|----------|
| Primary goal | Manual validation before releases |
| Validation scope | Comprehensive (connectivity + loop + capabilities) |
| Authentication | CLI auth preferred, env vars fallback |
| Results reporting | Terminal + structured reports (MD/JSON) |
| Isolation | `.e2e-tests/` workspace (gitignored) |
| Test definition | Hardcoded in Rust |
| Methodology | TDD-inspired (baseline → correct → edge cases) |

