# Common Ralph Introspection Recipes

Use these patterns when the user's question matches. Each recipe is:
1. Trigger — what the user said
2. Fetch — which doc pages to pull
3. Check — what to grep or grep-equivalent in them
4. Answer shape — how to frame the reply

## "Why did my loop terminate?"

1. **Trigger**: User shares a `Loop terminated:` banner or exit code 2.
2. **Fetch**:
   - `reference/troubleshooting/index.md`
   - `concepts/hats-and-events/index.md` (for the completion model)
3. **Check**: look for the specific reason string (`max_iterations`,
   `max_runtime_seconds`, `completion_promise`, `required_events`, `error`).
   Each maps to a documented termination path.
4. **Answer shape**: name the termination reason, quote the doc on what it
   means, tell the user the exact config knob (in ralph.yml or preset) that
   controls it. If it's `max_iterations` with work still pending, suggest
   raising the cap or checking whether `required_events` was actually emitted.

## "Why isn't my hat firing?"

1. **Trigger**: user-authored hat doesn't activate after the expected event.
2. **Fetch**:
   - `concepts/hats-and-events/index.md`
   - `reference/troubleshooting/index.md` (ambiguous-routing section)
3. **Check**: confirm the hat's `triggers:` matches the emitted event
   **exactly** (no wildcards), and that only one hat claims that trigger
   (ralph rejects ambiguous routing at preflight).
4. **Answer shape**: run `ralph hats validate` + `ralph hats graph`; show the
   expected event chain; point at the specific trigger in the preset.

## "Which preset should I use for X?"

1. **Trigger**: "should I use code-assist or feature?"
2. **Fetch**:
   - `guide/presets/index.md` (preset decision matrix)
3. **Check**: pattern column + entry event + completion event.
4. **Answer shape**: one-line recommendation + why (cite the pattern it
   matches). Offer `ralph hats list-presets` so the user sees everything
   discoverable locally, including TOML presets from `~/.config/autoloop/presets/`.

## "How does Ralph decide a backend?"

1. **Trigger**: "why did it pick claude?", "how do I force kiro-acp?".
2. **Fetch**:
   - `guide/backends/index.md`
   - `reference/faq/index.md` (auto-detect precedence)
3. **Check**: resolution order is CLI flag (`-b`) > `cli.backend:` in config >
   auto-detect walking the default priority list.
4. **Answer shape**: print the precedence, show the user the exact override
   for their case (flag or config).

## "What is the .ralph/ directory for?"

1. **Fetch**:
   - `advanced/memory-system/index.md`
   - `advanced/task-system/index.md`
   - `concepts/memories-and-tasks/index.md`
2. **Check**: scratchpad, memories.md, tasks.jsonl, loop.lock, loops.json,
   merge-queue.jsonl purposes.
3. **Answer shape**: one sentence per file + which ralph command touches it.

## "How do I add a new backend?"

1. **Trigger**: "I want Ralph to support X model CLI".
2. **Fetch**:
   - `guide/backends/index.md` (existing patterns)
   - `api/ralph-adapters/index.md` (the adapter trait / executor types)
3. **Check**: the backend enum, `CliBackend::<name>()` factory, executor
   type (PTY/Stdio/Acp), priority-list insertion point.
4. **Answer shape**: list the crates to edit (`ralph-adapters` for the
   backend + executor, `ralph-cli/src/doctor.rs` for env-var diagnostics,
   `guide/backends.md` for docs, tests). Refer the user to
   `references/contributing.md` for the PR workflow.

## "Why is Ralph slow / stuck?"

1. **Fetch**:
   - `reference/troubleshooting/index.md` (idle-timeout section)
   - `advanced/diagnostics/index.md`
2. **Check**: `idle_timeout_secs`, backend cold-start cost, TUI subprocess
   mode, diagnostic log path (`.ralph/diagnostics/logs/`).
3. **Answer shape**: direct them to the diagnostic log filename convention;
   suggest raising `idle_timeout_secs` for slow backends (kiro-acp is ~20s
   cold start).

## "How do I reset Ralph state between runs?"

1. **Fetch**:
   - `guide/cli-reference/index.md` (`ralph clean`)
2. **Answer shape**: `ralph clean` clears `.ralph/agent/`; manual removal of
   `.ralph/loops.json` for loop registry; `.ralph/merge-queue.jsonl` for
   merge queue.

## "Does Ralph support X feature?"

The generic pattern:

1. `curl` llms.txt.
2. `grep` for the feature keyword in section titles and link descriptions.
3. If present → fetch that page, confirm, answer yes with source.
4. If absent → search `reference/changelog/index.md` for recent additions.
5. Still absent → search `specs/` in the repo (not on doc site).
6. Finally → source tree at
   <https://github.com/mikeyobrien/ralph-orchestrator/tree/main/crates>.

Do not assume features exist because "they should" — Ralph is deliberately
minimal. When unsure, say "not documented; here's where to confirm".

## "What changed in the latest version?"

1. **Fetch**:
   - `reference/changelog/index.md`
2. **Answer**: summarize the entries newer than the user's local
   `ralph --version`. Include PR numbers when linked (e.g. #316) for
   traceability.
