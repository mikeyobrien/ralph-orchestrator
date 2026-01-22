# E2E Test Harness - Implementation Plan

## Implementation Checklist

- [ ] Step 1: Create crate scaffold and CLI skeleton
- [ ] Step 2: Implement WorkspaceManager
- [ ] Step 3: Implement AuthChecker and Backend detection
- [ ] Step 4: Implement RalphExecutor
- [ ] Step 5: Implement TestScenario trait and first scenario
- [ ] Step 6: Implement TestRunner with basic reporting
- [ ] Step 7: Add Tier 1 scenarios (Connectivity)
- [ ] Step 8: Add Tier 2 scenarios (Orchestration Loop)
- [ ] Step 9: Implement MetaRalphAnalyzer
- [ ] Step 10: Add Tier 5 scenarios (Hat Collections)
- [ ] Step 11: Add Tier 6 scenarios (Memory System)
- [ ] Step 12: Implement full Reporter (Markdown + JSON)
- [ ] Step 13: Add remaining tiers (Events, Capabilities, Errors)
- [ ] Step 14: Polish and documentation

---

## Step 1: Create Crate Scaffold and CLI Skeleton

**Objective:** Set up the new `ralph-e2e` crate with a working CLI that parses arguments and prints help.

**Implementation Guidance:**
1. Create `crates/ralph-e2e/Cargo.toml` with dependencies:
   - `clap` for CLI parsing
   - `tokio` for async runtime
   - `serde` + `serde_json` for serialization
   - `chrono` for timestamps
   - `colored` for terminal output
2. Create `src/main.rs` with clap-based CLI
3. Create `src/lib.rs` exporting public API
4. Add to workspace `Cargo.toml`

**Test Requirements:**
- `cargo build -p ralph-e2e` succeeds
- `ralph-e2e --help` shows usage
- `ralph-e2e --version` shows version
- `ralph-e2e --list` shows "No scenarios implemented yet"

**Integration:**
- Add `ralph-e2e` to workspace members in root `Cargo.toml`
- Ensure CI builds the new crate

**Demo:**
```bash
$ cargo run -p ralph-e2e -- --help
ralph-e2e 0.1.0
E2E test harness for Ralph orchestrator

USAGE:
    ralph-e2e [OPTIONS] [BACKEND]

ARGUMENTS:
    [BACKEND]    Backend to test: claude, kiro, opencode, all [default: all]

OPTIONS:
    -v, --verbose        Show detailed output
    --list               List available scenarios
    ...
```

---

## Step 2: Implement WorkspaceManager

**Objective:** Create isolated test workspaces in `.e2e-tests/` that can be inspected after runs.

**Implementation Guidance:**
1. Create `src/workspace.rs` with `WorkspaceManager` struct
2. Implement `create_workspace(scenario_id)` → creates `.e2e-tests/{scenario_id}/`
3. Implement `cleanup(scenario_id)` → removes workspace
4. Implement `cleanup_all()` → removes all workspaces
5. Add `.e2e-tests/` to `.gitignore`

**Test Requirements:**
```rust
#[test]
fn test_workspace_creation() {
    let ws = WorkspaceManager::new(".e2e-tests");
    let path = ws.create_workspace("test-scenario").unwrap();
    assert!(path.exists());
    ws.cleanup("test-scenario").unwrap();
    assert!(!path.exists());
}
```

**Integration:**
- Used by TestRunner to isolate each scenario
- Workspace contains: `ralph.yml`, `prompt.md`, `.agent/`

**Demo:**
```bash
$ cargo run -p ralph-e2e -- claude --keep-workspace
# After run:
$ ls .e2e-tests/
claude-connect/  claude-single-iter/  report.md
$ ls .e2e-tests/claude-connect/
ralph.yml  prompt.md  .agent/
```

---

## Step 3: Implement AuthChecker and Backend Detection

**Objective:** Detect available backends and verify authentication before running tests.

**Implementation Guidance:**
1. Create `src/auth.rs` with `AuthChecker` struct
2. Create `src/backend.rs` with `Backend` enum
3. Implement `Backend::is_available()` → checks if CLI exists (`which claude`)
4. Implement `Backend::is_authenticated()` → runs simple auth check
5. Implement `Backend::version()` → extracts CLI version
6. Implement `AuthChecker::check_all()` → returns `Vec<BackendInfo>`

**Test Requirements:**
```rust
#[test]
fn test_backend_detection() {
    let backend = Backend::Claude;
    // This test will pass/skip based on environment
    if backend.is_available() {
        assert!(backend.command() == "claude");
    }
}
```

**Integration:**
- CLI shows backend status before running tests
- Unavailable backends → skip their tests (not fail)

**Demo:**
```bash
$ cargo run -p ralph-e2e -- --list
Checking backends...
  ✅ Claude (claude 1.0.5) - Authenticated
  ✅ Kiro (kiro-cli 0.3.2) - Authenticated
  ❌ OpenCode - Not installed

Available scenarios:
  Tier 1: Connectivity
    claude-connect    Basic connectivity test for Claude
    kiro-connect      Basic connectivity test for Kiro
    opencode-connect  Basic connectivity test for OpenCode (SKIPPED - not installed)
```

---

## Step 4: Implement RalphExecutor

**Objective:** Execute `ralph run` with a given configuration and capture all output.

**Implementation Guidance:**
1. Create `src/executor.rs` with `RalphExecutor` struct
2. Implement `run(config: &ScenarioConfig)` → spawns ralph process
3. Capture stdout, stderr, exit code
4. Parse `.agent/` directory for scratchpad, events
5. Implement timeout handling
6. Return `ExecutionResult` with all captured data

**Test Requirements:**
```rust
#[tokio::test]
async fn test_executor_captures_output() {
    let executor = RalphExecutor::new(workspace);
    let config = ScenarioConfig::minimal();
    let result = executor.run(&config).await.unwrap();
    assert!(result.stdout.len() > 0 || result.stderr.len() > 0);
    assert!(result.exit_code.is_some());
}
```

**Integration:**
- Used by TestRunner for each scenario
- Provides full context for reporting

**Demo:**
```bash
# Internal: executor runs ralph and captures everything
ExecutionResult {
    exit_code: 0,
    stdout: "[Iteration 1]\nI'll help you...",
    stderr: "",
    duration: 12.3s,
    scratchpad: Some("## Tasks\n- [x] Create function"),
    events: [Event { topic: "build.done", ... }],
}
```

---

## Step 5: Implement TestScenario Trait and First Scenario

**Objective:** Define the TestScenario trait and implement the simplest scenario (claude-connect).

**Implementation Guidance:**
1. Create `src/scenarios/mod.rs` with `TestScenario` trait
2. Create `src/scenarios/claude.rs` with `ClaudeConnectScenario`
3. Implement `setup()` → creates minimal ralph.yml and prompt
4. Implement `run()` → executes and checks assertions
5. Implement assertion helpers (`output_contains`, `exit_code_is`, etc.)

**Test Requirements:**
```rust
#[tokio::test]
async fn test_claude_connect_scenario() {
    let scenario = ClaudeConnectScenario::new();
    assert_eq!(scenario.id(), "claude-connect");
    assert_eq!(scenario.backend(), Backend::Claude);
    // Setup creates valid config
    let config = scenario.setup(&workspace).unwrap();
    assert!(config.config_file.exists());
}
```

**Integration:**
- TestRunner discovers and runs scenarios
- First end-to-end test of the harness

**Demo:**
```bash
$ cargo run -p ralph-e2e -- claude --filter connect
Running 1 scenario...

Tier 1: Connectivity
  ✅ claude-connect (12.3s)
     └─ Response received: ✅
     └─ Exit code 0: ✅
     └─ No errors: ✅

All tests passed!
```

---

## Step 6: Implement TestRunner with Basic Reporting

**Objective:** Orchestrate scenario execution and display results in terminal.

**Implementation Guidance:**
1. Create `src/runner.rs` with `TestRunner` struct
2. Implement `run_scenarios(filter, backend)` → executes matching scenarios
3. Implement `collect_results()` → aggregates TestResults
4. Create `src/reporter.rs` with basic terminal output
5. Show progress during execution (scenario name, spinner)
6. Show summary at end (passed/failed counts)

**Test Requirements:**
```rust
#[tokio::test]
async fn test_runner_executes_scenarios() {
    let runner = TestRunner::new(workspace, vec![Box::new(ClaudeConnectScenario)]);
    let results = runner.run_all().await;
    assert_eq!(results.len(), 1);
}
```

**Integration:**
- CLI invokes TestRunner
- Results flow to Reporter

**Demo:**
```bash
$ cargo run -p ralph-e2e -- claude

🧪 E2E Test Harness v0.1.0
━━━━━━━━━━━━━━━━━━━━━━━━━━

Backend: Claude (claude 1.0.5)
Auth: ✅ Authenticated

Running 1 scenario...

Tier 1: Connectivity
  ✅ claude-connect (12.3s)

━━━━━━━━━━━━━━━━━━━━━━━━━━
🟢 PASSED: 1 of 1 tests passed
```

---

## Step 7: Add Tier 1 Scenarios (Connectivity)

**Objective:** Implement connectivity tests for all three primary backends.

**Implementation Guidance:**
1. Add `KiroConnectScenario` in `src/scenarios/kiro.rs`
2. Add `OpenCodeConnectScenario` in `src/scenarios/opencode.rs`
3. Each scenario:
   - Creates minimal config for its backend
   - Sends simple prompt: "Say hello"
   - Asserts: response received, exit code 0, no errors

**Test Requirements:**
- Unit tests for each scenario's setup
- Integration test that runs all connectivity scenarios (may skip if backend unavailable)

**Integration:**
- All three scenarios registered with TestRunner
- Skipped gracefully if backend not installed

**Demo:**
```bash
$ cargo run -p ralph-e2e -- all --filter connect

Running 3 scenarios...

Tier 1: Connectivity
  ✅ claude-connect (12.3s)
  ✅ kiro-connect (8.1s)
  ⏭️ opencode-connect (skipped - backend not installed)

━━━━━━━━━━━━━━━━━━━━━━━━━━
🟢 PASSED: 2 passed, 1 skipped
```

---

## Step 8: Add Tier 2 Scenarios (Orchestration Loop)

**Objective:** Implement scenarios that test the full Ralph orchestration loop.

**Implementation Guidance:**
1. `ClaudeSingleIterScenario` - Complete one iteration
2. `ClaudeMultiIterScenario` - Run 3 iterations, verify progression
3. `ClaudeCompletionScenario` - Verify LOOP_COMPLETE detection
4. Add iteration counting to ExecutionResult
5. Add termination reason parsing

**Test Requirements:**
- Verify iteration count matches expected
- Verify completion promise detected
- Verify scratchpad updated between iterations

**Integration:**
- These scenarios take longer (30-60s each)
- Need more complex prompts and configs

**Demo:**
```bash
$ cargo run -p ralph-e2e -- claude

Tier 2: Orchestration Loop
  ✅ claude-single-iter (23.1s)
     └─ Completed in 1 iteration: ✅
     └─ Scratchpad updated: ✅
  ✅ claude-multi-iter (45.6s)
     └─ Completed in 3 iterations: ✅
     └─ Events emitted: 3 ✅
  ✅ claude-completion (18.2s)
     └─ LOOP_COMPLETE detected: ✅
```

---

## Step 9: Implement MetaRalphAnalyzer

**Objective:** Use Ralph to analyze test results and generate rich insights.

**Implementation Guidance:**
1. Create `src/analyzer.rs` with `MetaRalphAnalyzer` struct
2. Create embedded analyzer config (`ralph-analyzer.yml`)
3. Implement `build_analysis_prompt(results)` → formats all test context
4. Implement `analyze(results)` → runs ralph, parses output
5. Parse `analyze.complete` event from Ralph output
6. Merge analysis with raw results

**Test Requirements:**
```rust
#[tokio::test]
async fn test_analyzer_produces_output() {
    let analyzer = MetaRalphAnalyzer::new();
    let raw_results = vec![mock_failed_result(), mock_passed_result()];
    let analyzed = analyzer.analyze(&raw_results).await.unwrap();
    assert!(analyzed[0].diagnosis.is_some()); // Failed test has diagnosis
    assert!(analyzed[1].analysis.is_some());  // Passed test has analysis
}
```

**Integration:**
- Called after all scenarios complete
- Results include both raw assertions and Ralph analysis

**Demo:**
```bash
$ cargo run -p ralph-e2e -- claude

Running 5 scenarios... done (2m 15s)

Analyzing results with meta-Ralph...
  ⚙️ Building analysis prompt...
  ⚙️ Running ralph analyzer...
  ✅ Analysis complete

[Report now includes rich diagnosis and optimization suggestions]
```

---

## Step 10: Add Tier 5 Scenarios (Hat Collections)

**Objective:** Test hat-based workflows with real backends.

**Implementation Guidance:**
1. `HatSingleScenario` - Execute with single custom hat
2. `HatMultiWorkflowScenario` - Planner → Builder delegation
3. `HatInstructionsScenario` - Verify hat instructions followed
4. `HatEventRoutingScenario` - Events route to correct hat
5. `HatBackendOverrideScenario` - Per-hat backend selection
6. Each scenario needs multi-hat ralph.yml configs

**Test Requirements:**
- Verify hat instructions appear in prompt
- Verify events route correctly
- Verify hat persona reflected in output

**Integration:**
- These are "GREEN + Pressure" tests
- Include quality criteria for optimization analysis

**Demo:**
```bash
Tier 5: Hat Collections
  ✅ hat-single (31.2s) [Quality: Optimal]
  🟡 hat-instructions (45.2s) [Quality: Good]
     └─ ⚠️ Agent acknowledged but behavior unchanged
  ✅ hat-multi-workflow (62.3s) [Quality: Optimal]
     └─ Planner → Builder → Done: ✅
```

---

## Step 11: Add Tier 6 Scenarios (Memory System)

**Objective:** Test the persistent memory system with real backends.

**Implementation Guidance:**
1. `MemoryAddScenario` - Add memory via CLI
2. `MemorySearchScenario` - Search memories
3. `MemoryInjectionScenario` - Verify auto-injection
4. `MemoryPersistenceScenario` - Memories survive across runs
5. Need to setup `.agent/memories.md` in workspace

**Test Requirements:**
- Verify `ralph memory add` creates entry
- Verify `ralph memory search` finds it
- Verify memories appear in prompt when `inject: auto`

**Integration:**
- Tests depend on memory CLI commands working
- Multi-run scenarios for persistence testing

**Demo:**
```bash
Tier 6: Memory System
  ✅ memory-add (5.2s)
     └─ Memory created: mem-1737372000-a1b2
  ✅ memory-search (3.1s)
     └─ Found 1 matching memory
  🟡 memory-injection (38.2s) [Quality: Acceptable]
     └─ ⚠️ Agent needed 2 attempts to use memories
```

---

## Step 12: Implement Full Reporter (Markdown + JSON)

**Objective:** Generate comprehensive agent-readable reports.

**Implementation Guidance:**
1. Implement `MarkdownReporter` with full report structure
2. Implement `JsonReporter` for programmatic access
3. Include all sections from design:
   - Summary with pass/fail verdict
   - Failed tests with full context and diagnosis
   - Passed tests with quality scores and optimizations
   - Recommendations prioritized by severity
   - Quick fix commands
4. Collapsible sections for large context blocks

**Test Requirements:**
- Generated markdown is valid
- Generated JSON parses correctly
- All test results included in report

**Integration:**
- Reports written to `.e2e-tests/report.md` and `report.json`
- Terminal shows summary + path to full report

**Demo:**
```bash
$ cargo run -p ralph-e2e -- claude

[... test execution ...]

━━━━━━━━━━━━━━━━━━━━━━━━━━
🟡 MIXED: 12 passed, 2 failed, 1 skipped

Reports written to:
  📄 .e2e-tests/report.md (detailed, agent-readable)
  📊 .e2e-tests/report.json (machine-readable)

$ head -20 .e2e-tests/report.md
# E2E Test Report

## 🔴 FAILED

**Generated:** 2025-01-20T14:30:00Z
**Verdict:** 2 tests failed - action required
...
```

---

## Step 13: Add Remaining Tiers (Events, Capabilities, Errors)

**Objective:** Complete the test scenario coverage.

**Implementation Guidance:**
1. **Tier 3: Events**
   - `ClaudeEventsScenario` - Event XML parsing
   - `ClaudeBackpressureScenario` - build.done evidence
2. **Tier 4: Capabilities**
   - `ClaudeToolUseScenario` - Tool invocation
   - `ClaudeStreamingScenario` - NDJSON parsing
3. **Tier 7: Error Handling (RED phase)**
   - `TimeoutScenario` - Graceful timeout
   - `MaxIterationsScenario` - Termination at limit
   - `AuthFailureScenario` - Bad credentials handling
   - `BackendUnavailableScenario` - Missing CLI handling

**Test Requirements:**
- Error scenarios verify graceful failure
- Capability scenarios verify features work end-to-end

**Integration:**
- Full test suite now covers all 7 tiers
- ~29 total scenarios

**Demo:**
```bash
$ cargo run -p ralph-e2e -- all

Running 29 scenarios...

Tier 1: Connectivity (3 scenarios)
  ✅ claude-connect, ✅ kiro-connect, ⏭️ opencode-connect

Tier 2: Orchestration (5 scenarios)
  ✅ claude-single-iter, ✅ claude-multi-iter, ...

[... all tiers ...]

Tier 7: Error Handling (4 scenarios)
  ✅ timeout-handling, ✅ max-iterations, ...

━━━━━━━━━━━━━━━━━━━━━━━━━━
🟢 PASSED: 26 passed, 0 failed, 3 skipped
```

---

## Step 14: Polish and Documentation

**Objective:** Final cleanup, documentation, and integration.

**Implementation Guidance:**
1. Add `README.md` for the crate
2. Add usage examples to CLAUDE.md
3. Run clippy and fix warnings
4. Add `ralph-e2e` to CI workflow (optional, may need secrets)
5. Create example report for documentation
6. Add `--skip-analysis` flag for faster runs

**Test Requirements:**
- `cargo clippy -p ralph-e2e` passes
- `cargo test -p ralph-e2e` passes
- `cargo doc -p ralph-e2e` generates docs

**Integration:**
- Document in CLAUDE.md under "E2E Testing" section
- Add to smoke test instructions

**Demo:**
```bash
$ ralph-e2e --help
# Full help text with examples

$ ralph-e2e claude
# Complete run with all features

$ cat .e2e-tests/report.md
# Beautiful, agent-readable report
```

---

## Timeline Estimate

| Step | Complexity | Dependencies |
|------|------------|--------------|
| 1 | Low | None |
| 2 | Low | Step 1 |
| 3 | Medium | Step 1 |
| 4 | Medium | Steps 1-2 |
| 5 | Medium | Steps 1-4 |
| 6 | Medium | Step 5 |
| 7 | Low | Step 6 |
| 8 | Medium | Step 7 |
| 9 | High | Step 6 |
| 10 | Medium | Steps 6, 9 |
| 11 | Medium | Steps 6, 9 |
| 12 | High | Step 9 |
| 13 | Medium | Step 12 |
| 14 | Low | All |

**Critical Path:** 1 → 2 → 4 → 5 → 6 → 9 → 12

---

## Success Criteria

The implementation is complete when:

1. ✅ `ralph-e2e claude` runs all Claude scenarios
2. ✅ `ralph-e2e all` runs scenarios for all available backends
3. ✅ Reports are agent-readable with full context
4. ✅ Meta-Ralph analysis provides diagnosis and optimizations
5. ✅ Writing-skills TDD principles are applied (RED/GREEN/REFACTOR)
6. ✅ All tests pass: `cargo test -p ralph-e2e`
7. ✅ Documentation complete in CLAUDE.md
