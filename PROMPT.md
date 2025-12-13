# Task: Port Improvements from Loop to Ralph-Orchestrator

## Context

The `~/code/loop` project has evolved with security hardening, thread safety fixes, and advanced logging features that should be ported to `ralph-orchestrator`. **Users are already using the published version**, so all changes MUST preserve backwards compatibility.

---

## Completed Pre-requisites

### Claude SDK Update to Opus 4.5 (DONE)

Updated `adapters/claude.py` to use Claude Opus 4.5 as the default model:

- [x] Added `model` parameter to `__init__()` and `configure()`
- [x] Default model: `claude-opus-4-5-20251101`
- [x] Added `MODEL_PRICING` dict with current pricing for all Claude 4.5 models
- [x] Updated `_calculate_cost()` to use model-specific pricing
- [x] Passes `model` to `ClaudeAgentOptions` in SDK calls
- [x] Metadata now reflects actual model used

**Pricing (per million tokens):**
| Model | Input | Output |
|-------|-------|--------|
| claude-opus-4-5-20251101 | $5.00 | $25.00 |
| claude-sonnet-4-5-20250929 | $3.00 | $15.00 |
| claude-haiku-4-5-20251001 | $1.00 | $5.00 |

**Usage:** Users can override the model via `ClaudeAdapter(model="claude-sonnet-4-5-20250929")` or in `configure(model=...)`.

---

## Critical UX Constraints (DO NOT BREAK)

### CLI Interface (unchanged)
```bash
ralph init                          # Creates .agent/, PROMPT.md, ralph.yml
ralph status                        # Show progress
ralph clean                         # Clean workspace
ralph prompt [ideas]               # Generate prompt
ralph run                           # Run orchestrator
```

All `ralph run` options must remain identical:
- `-c, --config`, `-a, --agent`, `-p, --prompt`, `-i, --max-iterations`
- `-t, --max-runtime`, `-v, --verbose`, `-d, --dry-run`
- `--max-tokens`, `--max-cost`, `--checkpoint-interval`
- `--context-window`, `--no-git`, `--no-archive`, `--no-metrics`

### Configuration Format (ralph.yml - unchanged)
Existing YAML keys and structure must continue to work.

### Output Structure (unchanged)
```
.agent/
├── prompts/
├── checkpoints/
├── metrics/
├── plans/
├── memory/
└── cache/
```

### Adapter Interface (unchanged)
- `ToolAdapter` base class with `check_availability()`, `execute()`, `aexecute()`
- `ToolResponse(success, output, error, tokens_used, cost, metadata)` return type
- Auto-detection order and fallback behavior

### Web API Endpoints (unchanged)
All existing REST and WebSocket endpoints must remain functional.

### Git Checkpoint Behavior (unchanged)
- Commits at `checkpoint_interval` iterations
- Commit format: `"Ralph checkpoint {iteration}"`

---

## Improvements to Port (Priority Order)

### 1. SecurityValidator System (HIGH) - DONE
**Source:** `/home/arch/code/loop/ralph/utils/security.py` (398 lines)

Port the following capabilities:
- [x] Path traversal protection (block `..`, `/etc`, `/usr/bin`, `/root`)
- [x] Dangerous pattern detection (7+ regex patterns)
- [x] Sensitive data masking (16+ patterns: API keys, tokens, passwords, SSH, AWS creds)
- [x] Configuration value validation with range limits
- [x] Filename validation (reserved names: CON, PRN, AUX; control chars)
- [x] `safe_file_read()` and `safe_file_write()` wrappers

**Implementation:**
- Created `src/ralph_orchestrator/security.py` with `SecurityValidator` and `PathTraversalProtection` classes
- Added 47 unit tests in `tests/test_security.py`
- All tests pass

**Integration points:** (pending future iterations)
- Wrap file operations in orchestrator.py
- Add to context.py for prompt handling
- Integrate with logging to mask sensitive output

### 2. Thread-Safe Configuration (HIGH)
**Source:** `/home/arch/code/loop/ralph/core/config.py` (418 lines)

- [ ] Add `threading.RLock` to RalphConfig
- [ ] Thread-safe property accessors: `get_delay()`, `set_delay()`, etc.
- [ ] Extract ConfigValidator class for validation logic
- [ ] Ensure backwards compatibility with existing YAML loading

**Constraint:** Must not change `RalphConfig` constructor signature or required fields.

### 3. Advanced Logging with Rotation (HIGH)
**Source:** `/home/arch/code/loop/ralph/core/logger.py` (455 lines)

- [ ] Automatic log rotation at 10MB with 3 backups
- [ ] Thread-safe rotation with `threading.Lock`
- [ ] Unicode sanitization for encoding errors
- [ ] Security-aware logging (mask sensitive data before write)
- [ ] Dual interface: async methods + sync wrappers

**Constraint:** Must not break existing logging calls in orchestrator.py.

### 4. Graceful Signal Handling (HIGH)
**Source:** `/home/arch/code/loop/ralph/core/runner.py` (lines 87-114)

- [ ] Kill subprocesses FIRST (synchronous, signal-safe)
- [ ] Emergency shutdown flag for logger
- [ ] Async task cancellation after subprocess cleanup
- [ ] Schedule emergency cleanup on event loop

**Constraint:** Current SIGINT/SIGTERM behavior must remain functional.

### 5. Error Formatter (MEDIUM)
**Source:** `/home/arch/code/loop/ralph/core/claude_client.py` (ClaudeErrorFormatter class)

- [ ] Structured error messages with user-friendly suggestions
- [ ] Pattern matching for: timeout, process termination, connection errors
- [ ] Security-aware error sanitization (no information disclosure)
- [ ] Methods: `format_timeout_error()`, `format_process_terminated_error()`, etc.

**Integration:** Apply to adapter error handling in `adapters/claude.py`, `adapters/qchat.py`.

### 6. VerboseLogger Enhancement (MEDIUM)
**Source:** `/home/arch/code/loop/ralph/utils/verbose_logger.py`

- [ ] Session metrics tracking in JSON format
- [ ] Emergency shutdown capability
- [ ] Re-entrancy protection (prevent logging loops)
- [ ] Console output with Rich library integration

**Constraint:** `-v/--verbose` flag behavior must remain consistent.

### 7. Statistics Improvements (LOW)
**Source:** `/home/arch/code/loop/ralph/utils/stats.py`

- [ ] Memory-efficient iteration tracking (limit to 1,000 stored)
- [ ] Per-iteration: duration, success/failure, error messages
- [ ] Success rate computation

**Integration:** Enhance existing `metrics.py`.

---

## Implementation Strategy

### Phase 1: Security Foundation
1. Create `src/ralph_orchestrator/security.py` based on loop's implementation
2. Add security validation to config loading
3. Wrap file operations with safe_* functions
4. Add sensitive data masking to logging

### Phase 2: Thread Safety & Logging
1. Add RLock to RalphConfig with backwards-compatible accessors
2. Enhance logging module with rotation and thread safety
3. Add unicode sanitization

### Phase 3: Error Handling & Signals
1. Implement ClaudeErrorFormatter
2. Update signal handlers with subprocess-first cleanup
3. Add emergency shutdown mechanism

### Phase 4: Testing
1. Port relevant tests from `/home/arch/code/loop/tests/`:
   - `test_security_vulnerabilities.py`
   - `test_thread_safety_race_conditions.py`
   - `test_memory_leaks_and_resources.py`
2. Verify all existing tests still pass
3. Add backwards compatibility tests

---

## Verification Checklist

After implementation, verify:
- [ ] `ralph init` creates identical directory structure
- [ ] `ralph run` accepts all documented flags
- [ ] `ralph.yml` files from existing users load without errors
- [ ] Web API endpoints return same response schemas
- [ ] Metrics JSON format unchanged
- [ ] Git checkpoint commit messages unchanged
- [ ] All adapters (claude, qchat, gemini) work identically
- [ ] `--verbose` output enhanced but not breaking

---

## Reference Files

**Loop Source (improvements):**
- `/home/arch/code/loop/ralph/utils/security.py`
- `/home/arch/code/loop/ralph/core/config.py`
- `/home/arch/code/loop/ralph/core/logger.py`
- `/home/arch/code/loop/ralph/core/runner.py`
- `/home/arch/code/loop/ralph/core/claude_client.py`
- `/home/arch/code/loop/ralph/utils/verbose_logger.py`
- `/home/arch/code/loop/ralph/utils/stats.py`

**Ralph-Orchestrator Targets (preserve UX):**
- `/home/arch/code/ralph-orchestrator/src/ralph_orchestrator/__main__.py`
- `/home/arch/code/ralph-orchestrator/src/ralph_orchestrator/main.py`
- `/home/arch/code/ralph-orchestrator/src/ralph_orchestrator/orchestrator.py`
- `/home/arch/code/ralph-orchestrator/src/ralph_orchestrator/adapters/*.py`
- `/home/arch/code/ralph-orchestrator/src/ralph_orchestrator/web/server.py`
