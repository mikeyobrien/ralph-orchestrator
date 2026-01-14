# Known Issues

> **Note:** When a known issue is fixed, remove it from this file. An empty file means no known issues.
>
> Generated from behavioral gap analysis on 2026-01-13
> **Validated against codebase:** 2026-01-14 (gap analysis iteration)

## Summary

| Priority | Issue | Impact | Status |
|----------|-------|--------|--------|
| 🟡 P1 | Planner behaviors instruction-only | No verification of compliance | ✅ Confirmed |
| 🟡 P1 | Builder behaviors instruction-only | No verification of compliance | ✅ Confirmed |
| 🟢 P2 | Scratchpad persistence not verified | State could be lost | ✅ Confirmed |
| 🟢 P2 | Hat display order is random | Minor UX confusion | ✅ Confirmed |

---

## 🟡 P1: Planner Behaviors Are Instruction-Only

**Behaviors:** PL-001 through PL-006, PL-008 through PL-015

**Problem:**
Planner is instructed to:
- Read specs directory
- Create/manage scratchpad
- Dispatch ONE task at a time
- Mark completed tasks [x]
- Prioritize ISSUES.md fixes
- Validate work matches spec

But orchestrator doesn't verify any of this. Planner could deviate without detection.

**Impact:**
- Planner could dispatch multiple tasks (overloading builder)
- Planner could ignore specs
- Planner could skip scratchpad updates
- Silent deviations from expected behavior

**Recommendation:**
Trust-but-verify approach (don't hard-fail, but log):
1. Parse `build.task` payload, warn if multiple tasks detected
2. After planner iteration, check scratchpad was modified
3. Log warnings for suspicious patterns
4. Use verification data for quality metrics

**Files:**
- `crates/ralph-core/src/event_loop.rs` (post-iteration verification)
- `crates/ralph-core/src/event_parser.rs` (parse build.task for task count)

---

## 🟡 P1: Builder Behaviors Are Instruction-Only

**Behaviors:** BU-001, BU-005 through BU-015

**Problem:**
Builder is instructed to:
- Implement ONE task per iteration
- Commit on success
- Mark task [x] in scratchpad
- Handle missing files gracefully
- Provide unblock recommendations

But orchestrator doesn't verify any of this.

**Impact:**
Similar to planner—builder could deviate without detection.

**Recommendation:**
Trust-but-verify approach:
1. Parse `build.done` for evidence of commit (commit hash in payload?)
2. Verify scratchpad has new [x] after builder iteration
3. Parse `build.blocked` for recommendation text, warn if missing
4. Log metrics for verification coverage

**Files:**
- `crates/ralph-core/src/event_loop.rs`
- `crates/ralph-core/src/event_parser.rs`

---

## 🟢 P2: Scratchpad Persistence Not Verified

**Behavior:** CB-001

**Problem:**
Spec says scratchpad content persists across iterations. Implementation tells hats about scratchpad path but doesn't verify it exists or was updated.

**Impact:**
If agent doesn't read/write scratchpad, state is lost. Usually works because Claude follows instructions, but not guaranteed.

**Recommendation:**
1. Read scratchpad at iteration start
2. Inject content snippet into prompt as context
3. After write-expected iterations (planner), verify modification time changed
4. Warn if scratchpad stale

**Files:**
- `crates/ralph-core/src/event_loop.rs`
- `crates/ralph-core/src/instructions.rs`

---

## 🟡 P1: Broken Preset - gap-analysis.yml

**Problem:**
The `presets/gap-analysis.yml` file uses YAML document separator `---` (line 186) to include a reference section at the end. The `serde_yaml` library doesn't support multi-document YAML parsing, causing a parse error.

**Error:**
```
YAML parse error: deserializing from YAML containing more than one document is not supported
```

**Impact:**
Users cannot use the gap-analysis preset at all. Running `ralph run --config presets/gap-analysis.yml` fails immediately.

**Fix Options:**
1. **Remove the separator**: Convert the reference section into a YAML comment block (`#` prefix)
2. **Move to separate file**: Put the reference material in a `gap-analysis-reference.md` file

**Location:** `presets/gap-analysis.yml:186`

---

## 🟡 P1: Broken Preset - review.yml

**Problem:**
The `presets/review.yml` file uses YAML document separator `---` (line 98) to include a reference section. Same root cause as gap-analysis.yml.

**Error:**
```
YAML parse error: deserializing from YAML containing more than one document is not supported
```

**Impact:**
Users cannot use the review preset. Running `ralph run --config presets/review.yml` fails immediately.

**Fix:**
Same as gap-analysis.yml - convert reference section to comments or move to separate file.

**Location:** `presets/review.yml:98`

---

## 🟡 P1: Broken Preset - refactor.yml

**Problem:**
The `presets/refactor.yml` file has an ambiguous routing configuration. The trigger `refactor.done` is claimed by both:
- `planner` hat (line 26)
- `verifier` hat (line 107)

The config validation correctly rejects this, but the preset ships broken.

**Error:**
```
Ambiguous routing: trigger 'refactor.done' is claimed by both 'planner' and 'verifier'
```

**Impact:**
Users cannot use the refactor preset. The validation catches this correctly, so it fails safely (no runtime confusion).

**Expected Behavior:**
Looking at the intended workflow:
```
refactor.task → [refactorer] → refactor.done → [verifier] → verify.passed → [planner]
```

The verifier should own `refactor.done`, and planner should trigger on `verify.passed` and `verify.failed` instead.

**Fix:**
Update `presets/refactor.yml` planner triggers:
```yaml
# Current (broken):
triggers: ["task.start", "task.resume", "refactor.done", "refactor.blocked", "verify.failed"]

# Fixed:
triggers: ["task.start", "task.resume", "verify.passed", "verify.failed", "refactor.blocked"]
```

**Location:** `presets/refactor.yml:26`

---

## 🟢 P2: Hat Display Order Is Random

**Problem:**
When displaying hats in dry-run output, the CLI uses `HashMap.keys()` which iterates in non-deterministic order. Users see hats in random order rather than the logical workflow order.

**Example:**
```
Dry run mode - configuration:
  Hats: reviewer, builder, planner  # Random order
```

Expected:
```
  Hats: planner, builder, reviewer  # Workflow order
```

**Impact:**
Minor UX confusion. Users may expect to see hats in the order they appear in the config file or in workflow order.

**Fix Options:**
1. Use `IndexMap` instead of `HashMap` for `config.hats` to preserve insertion order
2. Sort hat names alphabetically for consistent display
3. Sort by trigger order (task.start first, etc.)

**Location:** `crates/ralph-cli/src/main.rs:346`

---

## 🟡 P1: Broken ralph.yml in Repo Root

**Problem:**
The default `ralph.yml` configuration file in the repository root uses v1-style flat configuration that doesn't match the v2 nested schema. Specifically:
- `adapters.claude.tool_permissions` is a map (`allow_all: true`) but the schema expects a sequence (`["read", "write"]`)
- `adapters.acp.tool_permissions` contains undefined fields (`agent_command`, `agent_args`, `permission_mode`, `permission_allowlist`)

**Error:**
```
YAML parse error: adapters.claude.tool_permissions: invalid type: map, expected a sequence at line 49 column 7
```

**Impact:**
Users who clone the repo and try to use the default config (`ralph run`) get a parse error. The fallback (no config file) works, but having a broken default config is confusing.

**Current (broken):**
```yaml
adapters:
  claude:
    tool_permissions:         # Tool permissions for Claude
      allow_all: true         # Map - causes parse error
```

**Expected:**
```yaml
adapters:
  claude:
    tool_permissions: ["read", "write"]  # Sequence - matches schema
    # Or just remove the field (it's a dropped feature anyway)
```

**Fix Options:**
1. **Remove tool_permissions entirely** - it's a dropped feature per the comments in config.rs
2. **Update to sequence format** - if keeping for documentation purposes

**Location:** `ralph.yml:48-49`, `ralph.yml:91-95`

---

## Resolved Issues

_(Move issues here when fixed, then delete after next release)_

### ✅ P0: Backpressure Not Enforced (Fixed 2026-01-13)

**Behaviors:** BU-002, BU-003, BU-004

**Problem:** Builder could emit `build.done` without running tests/lint/typecheck. Orchestrator accepted any `build.done` event without verification.

**Solution:** 
- Added `BackpressureEvidence` struct to parse evidence from `build.done` payload
- Orchestrator validates evidence before accepting `build.done`
- Synthesizes `build.blocked` if evidence missing or checks failed
- Updated builder instructions with required evidence format

**Commit:** 9440d44c
