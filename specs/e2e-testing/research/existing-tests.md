# Existing Ralph Test Infrastructure

## Test Coverage Summary

| Category | Count | Location |
|----------|-------|----------|
| Smoke/Replay Tests | 37+ | `crates/ralph-core/tests/smoke_runner.rs` |
| Integration Tests (CLI) | 40+ | `crates/ralph-cli/tests/` |
| EventLoop Tests | 5+ | `crates/ralph-core/tests/event_loop_ralph.rs` |
| Scenario Tests | 5+ | `crates/ralph-core/tests/scenarios.rs` |
| TUI Tests | 10+ | `crates/ralph-tui/tests/` |
| E2E Tests (Python) | 15+ | `tools/e2e/test_*.py` |
| **Total** | **~112+** | Multi-layer approach |

---

## Replay-Based Smoke Tests

**Purpose**: Deterministic testing without live API calls.

**How it works**:
- JSONL fixtures contain recorded sessions
- `SmokeRunner` replays output without calling backends
- Validates event parsing, completion detection, etc.

**Fixtures location**: `crates/ralph-core/tests/fixtures/`
- `basic_session.jsonl` - Claude CLI session
- `kiro/` - Kiro CLI sessions

---

## Python E2E Tests

**Location**: `tools/e2e/`

**Approach**:
1. Create tmux session
2. Launch Ralph with config
3. Capture output using `freeze` CLI
4. Validate using LLM-as-judge (semantic validation)
5. Cleanup tmux session

**Key files**:
- `conftest.py` - Fixtures for ralph_binary, tmux_session, llm_judge
- `test_iteration_lifecycle.py` - Multi-iteration validation
- `helpers/llm_judge.py` - Claude-based visual assertions

---

## What's Missing (E2E Gap)

Current tests use **replay fixtures** (recorded sessions) or **mocked backends**.

**Not currently tested with real backends**:
- Live API connectivity
- Real prompt effectiveness
- Actual backend compatibility
- Real-world error scenarios
- Cross-backend consistency

This is exactly what the new E2E harness will address.
