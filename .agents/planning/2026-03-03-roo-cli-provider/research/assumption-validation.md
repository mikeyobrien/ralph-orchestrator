# Research: Assumption Validation Tests

All assumptions tested with live roo CLI invocations.

## Test Results (Round 1 — pre-fix)

| # | Assumption | Result | Evidence |
|---|-----------|--------|----------|
| 1 | `roo --print` works for headless | ✅ Verified | Produces text output and exits |
| 2 | `--prompt-file` works for large prompts | ✅ Verified | 7530-char prompt read from file successfully |
| 3 | `roo --version` for auto-detection | ✅ Verified | Returns "0.1.15" with exit code 0 |
| 4 | Conversation clears between iterations | ✅ Verified | Second invocation: "I have no memory of previous conversations" |
| 5 | Tool auto-approval is default | ✅ Verified | File write + read executed without any approval flag |
| 6 | Exit code 0 on success | ✅ Verified | Successful task returns exit code 0 |
| 7 | Event tags work with proper context | ✅ Verified | With Ralph-style system instructions, roo emits `<event>` tags and LOOP_COMPLETE correctly |
| 8 | `--ephemeral` breaks Bedrock | ✅ Verified | Cross-region inference error with ephemeral, works without |
| 9 | Exit code on API errors | ⚠️ Finding | Roo exits 0 even on API failure. Ralph must rely on LOOP_COMPLETE/event presence, not exit codes. |

## Test Results (Round 2 — after roo fixes)

Issues 1 (exit codes) and 2 (--ephemeral + Bedrock) were fixed in roo. Re-ran validation:

| # | Test | Result | Evidence |
|---|------|--------|----------|
| 1 | `--ephemeral` + Bedrock | ✅ **FIXED** | `roo --print --ephemeral` now works with Bedrock. Output: `[assistant] Hello!` |
| 2 | `--prompt-file` + `--ephemeral` | ✅ Verified | Large prompt via file + ephemeral mode works |
| 3 | Event tags + `--ephemeral` | ✅ Verified | `<event topic="build.done">` and `LOOP_COMPLETE` emitted correctly in ephemeral mode |
| 4 | Exit codes on errors | ⚠️ Still exits 0 | Invalid provider, bad API key still exit 0. Not a blocker — Ralph uses LOOP_COMPLETE detection. |

### Updated Design Decision
With `--ephemeral` fixed, the default headless command is now: `roo --print --ephemeral "prompt"` — clean disk state between iterations.

## Key Findings

### Finding 1: Event Tag Emission (Assumption 7)
Roo has built-in prompt injection detection. A bare "output this text" prompt with event tags is **refused** as a prompt injection attack. However, when the event protocol is provided as part of system instructions (as Ralph does), roo cooperates and emits event tags correctly.

**Implication**: The Ralph prompt format (with `## EVENT PROTOCOL` instructions) already works correctly with roo. No changes needed.

### Finding 2: Exit Code Always 0 (Assumption 9)
Roo exits with code 0 even when:
- API errors occur (Bedrock cross-region)
- `--exit-on-error` is specified

**Implication**: Ralph cannot use exit codes to detect roo failures. This is fine because Ralph's primary failure detection is via:
1. **Missing LOOP_COMPLETE token** → consecutive failure counter increments
2. **Missing event tags** → no events published → backpressure detects staleness
3. **Idle timeout** → roo process stuck retrying → Ralph kills it

### Finding 3: --ephemeral + Bedrock (Assumption 8)
`--ephemeral` causes roo to use a temp directory for storage, losing access to persisted Bedrock settings (cross-region inference config). This causes API failures.

**Implication**: `--ephemeral` must not be a default flag. Users on non-Bedrock providers (Anthropic direct, OpenRouter) might use it safely via `cli.args`.
