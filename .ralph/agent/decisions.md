# Decisions Log

## Format

Use this template for new entries:

```
## DEC-XXX (YYYY-MM-DDTHH:MMZ)
- Decision: <short statement>
- Chosen Option: <A/B/description>
- Confidence: <0-100>
- Alternatives Considered: <summary of options>
- Reasoning: <why this option was chosen>
- Reversibility: <High|Medium|Low>
- Timestamp: <UTC ISO 8601; use date-only if time unknown>
```

## DEC-001 (2026-01-31)
- Decision: Add an integration test for `ralph web` using fake node/npm/npx scripts and a PATH override to exercise the execute path.
- Confidence: 78
- Alternatives: (A) Only add unit tests for preflight/termination helpers with smaller coverage impact. (B) Refactor `execute` to accept injected command paths.
- Reasoning: Integration test covers large untested logic without altering production behavior; PATH override is scoped to the child process.
- Reversibility: High; test can be removed if flaky.

## DEC-002 (2026-01-31)
- Decision: Normalize Telegram bot tokens by trimming whitespace and treating empty env/keychain/config values as missing via `resolve_token_from`.
- Confidence: 82
- Alternatives: (A) Keep current precedence even if env var is empty. (B) Only normalize env tokens and leave keychain/config as-is.
- Reasoning: Prevents empty or whitespace env vars from blocking fallback sources and keeps token resolution predictable.
- Reversibility: High; adjust normalization or revert helper usage if needed.

## DEC-003 (2026-01-31)
- Decision: Extend `resolve_loop` to allow partial (suffix/contains) matches against merge queue entries, mirroring registry/worktree behavior.
- Confidence: 78
- Alternatives: (A) Keep merge queue exact-only lookup for safety. (B) Require explicit `loop-id` for queued/merged loops via CLI help update.
- Reasoning: CLI supports short IDs for loops in other sources; missing merge-queue partial match is inconsistent and causes "not found" for queued-only loops.
- Reversibility: High; revert branch or gate behind a flag if ambiguity arises.

## DEC-004 (2026-02-01)
- Decision: Strip common executable extensions (e.g., `.exe`, `.cmd`, `.bat`, `.com`) when canonicalizing custom backend command names for doctor checks.
- Confidence: 85
- Alternatives: (A) Preserve extensions and accept backend names like `claude.exe`. (B) Only strip on Windows with cfg-gating.
- Reasoning: Custom commands may include Windows-style extensions; normalizing avoids mislabeling known backends and keeps checks consistent across platforms.
- Reversibility: High; revert normalization or restrict it to Windows if needed.

## DEC-005 (2026-02-01)
- Decision: Target `crates/ralph-cli/src/loops.rs` with new tests for `merge_loop` error branches and default CLI execution to lift coverage in a high-uncovered module.
- Confidence: 78
- Alternatives: (A) Focus on event loop coverage with additional orchestration tests. (B) Add tests in `loop_runner` to cover async run paths.
- Reasoning: `merge_loop` branches are deterministic and testable without spawning external processes, providing safe coverage gains in a top-uncovered file.
- Reversibility: High; tests are additive and can be removed if they become brittle.

## DEC-006 (2026-02-01)
- Decision: Add a `--no-input` flag and auto-skip prompts when stdin is not a TTY for `ralph tutorial`.
- Confidence: 78
- Alternatives: (A) Always prompt and require interactive stdin. (B) Only auto-skip prompts without exposing a flag.
- Reasoning: Avoids hangs in non-interactive runs/tests while keeping the default tutorial interactive.
- Reversibility: High; flag and detection can be removed or adjusted without affecting core behavior.

## DEC-007 (2026-02-01)
- Decision: Resolve `ralph tools skill` workspace root by searching upward for ralph.yml when no --root is provided, and fall back to `.claude/skills` when skills.dirs is empty but the directory exists.
- Confidence: 78
- Alternatives: (A) Require explicit --root and leave skills.dirs empty unless configured. (B) Change SkillsConfig defaults globally to include .claude/skills.
- Reasoning: Makes `ralph tools skill` usable from subdirectories and in default workspaces that already have `.claude/skills`, without changing core config defaults.
- Reversibility: High; adjust resolution or remove fallback if it causes unexpected discovery.

## DEC-008 (2026-02-01)
- Decision: When `skills.dirs` is empty, search upward from the current directory (bounded to the workspace root) for `.claude/skills` and register that path, using a relative path when possible.
- Confidence: 78
- Alternatives: (A) Only use `root/.claude/skills` and require `--root` or config for other locations. (B) Search past the workspace root for any `.claude/skills` on the filesystem.
- Reasoning: Supports nested repo layouts where the workspace config is above the repo that owns the skills directory, preventing false "skill not found" errors without broadening discovery to unrelated parent directories.
- Reversibility: High; remove the fallback or restrict to explicit configuration if unexpected discovery occurs.

## DEC-009 (2026-02-01)
- Decision: If no skills directory is discovered under the workspace root, scan parent directories of the root for `.claude/skills` as a fallback.
- Confidence: 74
- Alternatives: (A) Require explicit skills.dirs configuration when the workspace root is nested. (B) Change workspace root resolution to prefer higher-level ralph.yml.
- Reasoning: Fixes nested-config layouts (like this workspace) without changing config semantics, while keeping the fallback narrow and only used when no local skills dir exists.
- Reversibility: High; remove the parent scan or gate it behind a flag if it causes unexpected discovery.

## DEC-010 (2026-02-01)
- Decision: Resolve configured relative skill directories by searching parent directories of the workspace root when the root-local path is missing, and store the resolved absolute path for skill discovery.
- Confidence: 82
- Alternatives: (A) Only fall back when skills.dirs is empty and ignore configured dirs. (B) Force users to pass --root or fix ralph.yml to point at the repo root.
- Reasoning: Honors configured relative paths while fixing nested-workspace layouts (e.g., `ralph.yml` inside a subdir) without surprising users with unrelated default discovery.
- Reversibility: High; revert to root-only resolution or gate the parent search behind a flag if it causes unexpected discovery.
