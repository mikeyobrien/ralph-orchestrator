# Ralph Engineering Order Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Add a Ralph hat collection, `engineering-order`, that can take almost any core software-engineering request and route it through the right disciplined workflow: research, architecture, implementation, debugging, review, test/QA, refactor, docs, maintenance, performance/security, or release readiness.

**Architecture:** Build this as a public builtin hat collection first, not a new orchestration subsystem. The Order is a router + duty-specific hats over Ralph's existing event bus, `TaskStore`, `MarkdownMemoryStore`, and runtime task commands. It should produce one inspectable dossier per run, use runtime tasks as the live queue, and force every track through evidence-backed validation before `ORDER_COMPLETE`.

**Tech Stack:** Rust workspace (`ralph-cli`, `ralph-core`, `ralph-api`), YAML Ralph presets, existing preset sync script, Bash preset evaluation harness, Node tRPC preset listing tests.

---

## Current context / assumptions

- Target repo: `/Users/rook/projects/ralph-orchestrator`.
- Current main checkout has untracked `.hermes/` and `tasks/`; implementation should happen in an isolated worktree/branch.
- Existing public builtin presets:
  - `code-assist` — TDD implementation.
  - `debug` — scientific root-cause loop.
  - `research` — read-only evidence synthesis.
  - `review` — adversarial code review.
  - `autoresearch` — measurable optimization experiments.
  - `pdd-to-code-assist` — full idea → design → implementation example.
- The new Order should reuse the patterns above, not replace them or add a Kanban clone.
- Proposed slug: `engineering-order`. If Mikey wants the fuller name, change the slug to `software-engineering-order` before implementation.

## Product shape

### What “core software engineering responsibilities” covers

| Responsibility | Track | Mutates code? | Required evidence |
|---|---|---:|---|
| New feature / enhancement | implementation | yes | tests first, demo path, adversarial case, review |
| Bug / flaky test / regression | debug | usually | reproduction, hypothesis, regression test, fix verification |
| Code review / PR review | review | no | file:line findings, runnable/adversarial checks when possible |
| Architecture / design | architecture | no by default | evidence-backed design options, tradeoffs, recommendation |
| Refactor / cleanup | refactor | yes | behavior-preserving tests, focused diff, review |
| Test/QA hardening | test | yes | failing/weak coverage identified, new test proves behavior |
| Docs / developer UX | docs | usually | docs build/check, command examples verified where practical |
| Dependency/security hardening | maintenance/security | yes | audit findings, minimal upgrade/fix, regression/security checks |
| Performance work | performance | yes | baseline metric, changed metric, correctness checks |
| Release readiness | release | maybe | clean diff, package/build smoke, changelog/docs, final gate |

Out of scope for this preset: market research, business strategy, open-ended product positioning, sales/support ops, or production deployment without explicit human approval.

### Core workflow

All tracks go through the same top-level gate:

1. `steward` classifies the request and writes the run dossier.
2. A track-specific hat gathers context or executes work.
3. `planner` ensures only the current wave of runtime tasks.
4. Mutating tracks use `builder` or `investigator` + `builder`.
5. `verifier` runs the strongest available evidence path.
6. `reviewer` adversarially inspects the result.
7. `finalizer` closes/reopens runtime tasks and emits either `queue.advance`, repair events, or `ORDER_COMPLETE`.

### Dossier layout

Every run uses:

```text
.agents/scratchpad/order/{run_slug}/
  dossier.md        # original request, classification, constraints, scope, one-way doors
  plan.md           # numbered step plan, one active wave at a time
  progress.md       # current step, active task ids/keys, verification log
  evidence.md       # commands/tests/manual checks with observed outputs
  decisions.md      # confidence 50-80 decisions and rationale
  final-report.md   # concise handoff written by finalizer
```

Runtime tasks remain the scheduler. Files above are evidence and handoff state only.

### Event contract

Use one universal completion contract so read-only and mutating tracks both work:

```yaml
event_loop:
  prompt_file: "PROMPT.md"
  completion_promise: "ORDER_COMPLETE"
  required_events:
    - "order.classified"
    - "order.plan.ready"
    - "order.validated"
  starting_event: "order.start"
  max_iterations: 150
  max_runtime_seconds: 21600
  checkpoint_interval: 5
```

Track-specific events are optional branches, but every successful branch must converge on `order.validated` before `ORDER_COMPLETE`.

#### Primary events

- `order.classified` — emitted by `steward`; payload includes `track`, `mutating`, `confidence`, `dossier_dir`, and `approval_required`.
- `order.plan.ready` — emitted by `planner`, `researcher`, `reviewer`, or `shipwright`; payload includes the active task id/key and dossier paths.
- `tasks.ready` — implementation/refactor/test/docs/security/perf current-wave task.
- `debug.hypothesis` / `debug.fix.ready` — debug branch.
- `implementation.ready` — builder finished one runtime task.
- `verification.failed` / `verification.passed` — verifier decision.
- `review.rejected` / `review.passed` — adversarial review decision.
- `release.failed` / `release.passed` — release branch decision.
- `order.validated` — final evidence gate for all tracks.
- `queue.advance` — another planned wave remains.
- `human.interact` — only for one-way doors or confidence <50.
- `ORDER_COMPLETE` — finalizer done.

### Hats

MVP hats:

1. `steward` — intake/classification, scope, approval gate, dossier creation.
2. `researcher` — read-only repo/context research for unknowns, architecture, dependency/security/perf discovery.
3. `planner` — numbered step plan and runtime task queue owner.
4. `builder` — TDD implementation/refactor/docs/test/security/perf task executor.
5. `investigator` — reproduces bugs, forms hypotheses, proposes minimal fix direction.
6. `verifier` — tests, builds, harnesses, repro checks, adversarial/manual evidence.
7. `reviewer` — adversarial code/design/release review, security and edge-case focus.
8. `shipwright` — release-readiness/package/docs/changelog/clean-diff checks.
9. `finalizer` — task closure, queue advancement, final report, `ORDER_COMPLETE`.

Guardrails for every hat:

- Do not spawn subagents inside the preset; hats are the decomposition.
- Prefer direct evidence over speculation.
- Use `$RALPH_BIN tools task ...` and `$RALPH_BIN emit ...`; plain prose does not advance the loop.
- Use confidence protocol:
  - >80: proceed autonomously.
  - 50-80: proceed with safe default and write `decisions.md`.
  - <50: emit `human.interact` with the exact ambiguity.
- Ask before one-way doors: destructive migrations, production deploys, publishing releases, deleting user data, credential changes, paid services.

---

## Step-by-step implementation plan

### Task 1: Create an isolated implementation worktree

**Objective:** Keep the broad preset work away from existing untracked main-checkout state.

**Files:**
- No source files changed in the main checkout.
- Worktree: `/Users/rook/projects/ralph-orchestrator.engineering-order`

**Steps:**

1. From `/Users/rook/projects/ralph-orchestrator`, create a branch worktree:

   ```bash
   git worktree add -b feat/engineering-order ../ralph-orchestrator.engineering-order main
   ```

2. Enter the worktree:

   ```bash
   cd /Users/rook/projects/ralph-orchestrator.engineering-order
   ```

3. Verify clean state:

   ```bash
   git status --short --branch
   ```

   Expected: branch `feat/engineering-order`, no dirty source files.

### Task 2: Add failing preset-embedding contract tests first

**Objective:** Lock the public builtin contract before adding the preset.

**Files:**
- Modify: `crates/ralph-cli/src/presets.rs`

**Steps:**

1. Update `test_list_presets_returns_all` expected public count from `6` to `7`.
2. Update `test_preset_names_returns_all_names` expected count from `6` to `7`, and assert `engineering-order` is present.
3. Add a focused contract test:

   ```rust
   #[test]
   fn test_engineering_order_contract() {
       let preset = get_preset("engineering-order").expect("engineering-order should exist");
       let config = RalphConfig::parse_yaml(preset.content)
           .expect("embedded preset YAML should parse");

       assert_eq!(config.event_loop.completion_promise, "ORDER_COMPLETE");
       assert_eq!(config.event_loop.starting_event.as_deref(), Some("order.start"));
       assert!(config.event_loop.required_events.contains(&"order.classified".to_string()));
       assert!(config.event_loop.required_events.contains(&"order.plan.ready".to_string()));
       assert!(config.event_loop.required_events.contains(&"order.validated".to_string()));

       for hat in [
           "steward",
           "researcher",
           "planner",
           "builder",
           "investigator",
           "verifier",
           "reviewer",
           "shipwright",
           "finalizer",
       ] {
           assert!(config.hats.contains_key(hat), "missing {hat} hat");
       }

       let finalizer = config.hats.get("finalizer").expect("finalizer hat exists");
       assert!(finalizer.publishes.contains(&"ORDER_COMPLETE".to_string()));
       assert!(finalizer.publishes.contains(&"queue.advance".to_string()));

       assert!(preset.content.contains(".agents/scratchpad/order/{run_slug}/"));
       assert!(preset.content.contains("confidence protocol"));
       assert!(preset.content.contains("one-way doors"));
   }
   ```

4. Run the narrow tests and verify they fail because the preset is not yet embedded:

   ```bash
   cargo test -p ralph-cli presets::tests::test_engineering_order_contract presets::tests::test_list_presets_returns_all presets::tests::test_preset_names_returns_all_names
   ```

   Expected: FAIL on missing `engineering-order` / count mismatch.

### Task 3: Create the canonical `engineering-order` preset YAML

**Objective:** Add the actual Order workflow in Ralph's native YAML shape.

**Files:**
- Create: `presets/engineering-order.yml`

**Steps:**

1. Start from this header and event skeleton:

   ```yaml
   # Engineering Order Preset
   #
   # General-purpose software engineering workflow for feature work, bugs,
   # reviews, architecture, tests, docs, maintenance, performance/security, and release readiness.

   description: "General software engineering order: classify, plan, implement/debug/review, verify, and finalize with evidence"

   event_loop:
     prompt_file: "PROMPT.md"
     completion_promise: "ORDER_COMPLETE"
     required_events: ["order.classified", "order.plan.ready", "order.validated"]
     starting_event: "order.start"
     max_iterations: 150
     max_runtime_seconds: 21600
     checkpoint_interval: 5
     idle_timeout_secs: 300

   cli:
     backend: "claude"
     prompt_mode: "arg"

   core:
     specs_dir: ".agents/scratchpad/"
     guardrails:
       - "Fresh context each iteration — re-read the dossier, plan, progress, and current runtime task."
       - "Runtime tasks are the canonical queue; dossier files are evidence and handoff only."
       - "Verification is mandatory before completion. Prefer direct runnable evidence."
       - "No subagents inside this preset; hats are the decomposition."
       - "Ask before one-way doors: destructive migrations, production deploys, publishing, credential changes, paid services, or irreversible data changes."
       - "Use confidence protocol: >80 proceed, 50-80 safe default plus decisions.md, <50 human.interact."

   hats:
     steward:
       name: "Steward"
       description: "Classifies the request, scopes the run, creates the dossier, and gates one-way doors."
       triggers: ["order.start", "queue.advance", "order.blocked"]
       publishes: ["order.classified", "human.interact", "ORDER_COMPLETE"]
       default_publishes: "human.interact"
       instructions: |
         ## STEWARD MODE — Intake And Track Selection
         ...
   ```

2. Fill in all nine hats with concrete instructions. Use the behavior from existing presets, but keep prompts shorter than `pdd-to-code-assist`:
   - `steward`: classify, write `dossier.md`, choose track, emit `order.classified` or `human.interact`.
   - `researcher`: read-only context, write/update `evidence.md`, emit `order.plan.ready` for read-only tracks or `research.ready` for planner.
   - `planner`: write numbered `plan.md`, ensure only current-wave runtime tasks with keys `order:{run_slug}:step-XX:{slug}`, emit `tasks.ready` or `order.plan.ready`.
   - `builder`: start one runtime task, write failing test first for code changes, implement minimal fix/feature, run focused tests, emit `implementation.ready` or `build.blocked`.
   - `investigator`: reproduce, form one hypothesis, test it, propose focused fix task, emit `debug.fix.ready` or `order.validated` if no mutation is needed.
   - `verifier`: run exact verification commands, manual/harness check, one adversarial case, update `evidence.md`, emit `verification.passed`/`verification.failed`.
   - `reviewer`: adversarial review of diff/design/result; for read-only review tasks this is the primary executor; emit `review.passed`/`review.rejected`.
   - `shipwright`: release checks: clean diff classification, package/build smoke, docs/changelog, no scratch junk, emit `release.passed`/`release.failed`.
   - `finalizer`: close/reopen tasks, decide queue advancement, write `final-report.md`, emit `order.validated` then `ORDER_COMPLETE` only when all gates are satisfied.

3. Add explicit examples in the `steward` prompt for the responsibility matrix above.
4. Ensure every hat says the turn is incomplete until a real `$RALPH_BIN emit ...` command succeeds.
5. Ensure every event payload carries `run_slug`, `track`, `dossier_dir`, and active `task_id`/`task_key` when a runtime task exists.

### Task 4: Add the preset to embedded builtin sources

**Objective:** Make `builtin:engineering-order` available in packaged Ralph binaries.

**Files:**
- Modify: `scripts/sync-embedded-files.sh`
- Modify: `crates/ralph-cli/src/presets.rs`
- Create via sync: `crates/ralph-cli/presets/engineering-order.yml`

**Steps:**

1. Add mirror mapping in `scripts/sync-embedded-files.sh` near other canonical presets:

   ```bash
   "presets/engineering-order.yml:crates/ralph-cli/presets/engineering-order.yml"
   ```

2. Add an `EmbeddedPreset` entry in `crates/ralph-cli/src/presets.rs`:

   ```rust
   EmbeddedPreset {
       name: "engineering-order",
       description: "General software engineering workflow for features, bugs, reviews, architecture, maintenance, and releases",
       content: include_str!("../presets/engineering-order.yml"),
       public: true,
   },
   ```

3. Run sync:

   ```bash
   ./scripts/sync-embedded-files.sh
   ```

4. Verify sync can pass in check mode:

   ```bash
   ./scripts/sync-embedded-files.sh check
   ```

### Task 5: Update public preset indexes and docs

**Objective:** Keep CLI/web/docs listings aligned.

**Files:**
- Modify: `presets/index.json`
- Modify: `presets/README.md`
- Modify: `docs/guide/presets.md`
- Modify if present/relevant: `skills/ralph-docs/references/common-questions.md`
- Modify if present/relevant: `skills/ralph-docs/references/llms-txt-map.md`

**Steps:**

1. Add an index entry:

   ```json
   {
     "name": "engineering-order",
     "description": "General software engineering order for features, bugs, reviews, architecture, tests, maintenance, performance/security, docs, and releases",
     "category": "development"
   }
   ```

2. In `presets/README.md`, add to Supported Builtins:

   ```markdown
   | `engineering-order` | `presets/engineering-order.yml` | General software engineering workflow across implementation, debug, review, architecture, QA, maintenance, and release readiness |
   ```

3. Add quick-start example:

   ```bash
   ralph run -c ralph.yml -H builtin:engineering-order -p "Fix the failing auth tests and harden the edge cases"
   ```

4. In `docs/guide/presets.md`, add usage guidance:
   - Use `engineering-order` when the request is mixed or ambiguous across core engineering duties.
   - Use narrower presets (`code-assist`, `debug`, `review`, `research`, `autoresearch`) when the track is already obvious and you want less orchestration overhead.

### Task 6: Add preset evaluation tasks

**Objective:** Let the existing preset evaluation harness dogfood the new Order.

**Files:**
- Modify: `tools/preset-test-tasks.yml`

**Steps:**

1. Add full task:

   ```yaml
   engineering-order: |
     Handle a mixed software-engineering request under `.eval-sandbox/engineering-order/`.
     Requirements:
     1. Classify the task and write an order dossier.
     2. Build a tiny runnable CLI status checker with one happy path and one failure path.
     3. Use tests first for the behavior.
     4. Run focused tests and a real manual CLI check.
     5. Perform an adversarial review pass.
     6. Write a final report summarizing files changed, evidence, and remaining risks.
   ```

2. Add smoke task:

   ```yaml
   engineering-order: |
     Build and verify a tiny CLI status checker in `.eval-sandbox/engineering-order/`.
     Requirements:
     1. Classify the request.
     2. Implement one tested happy path and one tested invalid input path.
     3. Run tests and one manual CLI command for each path.
     4. Complete only after verification and review evidence is recorded.
   ```

3. Add metadata:

   ```yaml
   complexity:
     engineering-order: complex

   expected_iterations:
     engineering-order: [6, 12]

   smoke_timeouts:
     engineering-order: 900
   ```

### Task 7: Make preset evaluation portable on macOS if needed

**Objective:** Avoid the known GNU `timeout` pitfall when smoke-testing on macOS.

**Files:**
- Modify if needed: `tools/evaluate-preset.sh`

**Steps:**

1. Replace the direct `timeout --foreground` assumption with a helper:

   ```bash
   resolve_timeout_command() {
       if command -v timeout >/dev/null 2>&1; then
           TIMEOUT_CMD=(timeout --foreground)
       elif command -v gtimeout >/dev/null 2>&1; then
           TIMEOUT_CMD=(gtimeout --foreground)
       else
           TIMEOUT_CMD=()
       fi
   }
   ```

2. Update `run_ralph_with_timeout`:

   ```bash
   run_ralph_with_timeout() {
       local timeout_seconds=$1
       shift
       if [[ ${#TIMEOUT_CMD[@]} -gt 0 ]]; then
           "${TIMEOUT_CMD[@]}" "$timeout_seconds" "${RALPH_CMD[@]}" "$@"
       else
           python3 - "$timeout_seconds" "${RALPH_CMD[@]}" "$@" <<'PY'
   import subprocess, sys
   timeout = int(sys.argv[1])
   cmd = sys.argv[2:]
   try:
       raise SystemExit(subprocess.run(cmd, timeout=timeout).returncode)
   except subprocess.TimeoutExpired:
       raise SystemExit(124)
   PY
       fi
   }
   ```

3. Call `resolve_timeout_command` after `resolve_ralph_command`.
4. Add a small shell fallback test if there is an existing script-test pattern; otherwise verify manually by temporarily clearing `PATH` of `timeout`/`gtimeout` in a controlled shell.

### Task 8: Add explicit graph/prompt contract tests

**Objective:** Catch shallow or broken Order YAML before live runs.

**Files:**
- Modify: `crates/ralph-cli/src/presets.rs`

**Steps:**

1. Extend `test_engineering_order_contract` with assertions for:
   - `steward.triggers` contains `order.start`.
   - `steward.publishes` contains `order.classified` and `human.interact`.
   - `planner.publishes` contains `tasks.ready` and `order.plan.ready`.
   - `builder.triggers` contains `tasks.ready`, `review.rejected`, and `verification.failed`.
   - `verifier.publishes` contains `verification.passed`, `verification.failed`, and `order.validated`.
   - `reviewer.publishes` contains `review.passed` and `review.rejected`.
   - `shipwright.publishes` contains `release.passed` and `release.failed`.
   - `finalizer.default_publishes` is not `ORDER_COMPLETE`; failures should fail closed.
2. Assert important prompt phrases exist:
   - `Runtime tasks are the canonical queue`.
   - `Do not spawn subagents`.
   - `one adversarial`.
   - `The turn is incomplete until`.
   - `Ask before one-way doors`.

### Task 9: Run narrow Rust tests until green

**Objective:** Verify the embedded preset parses and exposes the intended contract.

**Files:**
- No new files; test only.

**Commands:**

```bash
cargo test -p ralph-cli presets::tests::test_engineering_order_contract
cargo test -p ralph-cli presets::tests::test_preset_content_is_valid_yaml
cargo test -p ralph-cli presets::tests::test_public_presets_have_completion_path
cargo test -p ralph-cli presets::tests::test_public_presets_have_required_events
cargo test -p ralph-cli init::tests::test_format_preset_list
```

Expected: all pass.

### Task 10: Verify CLI listing and preflight

**Objective:** Prove a user can discover and select the new builtin.

**Files:**
- No new files; verification only.

**Commands:**

```bash
cargo run --bin ralph -- init --list-presets | tee /tmp/ralph-presets.txt
grep -F "engineering-order" /tmp/ralph-presets.txt
cargo run --bin ralph -- preflight -c ralph.yml -H builtin:engineering-order
```

Expected:
- `engineering-order` appears in list output.
- Preflight succeeds or reports only unrelated environment/backend issues; no preset-not-found or YAML parse errors.

### Task 11: Verify web/server preset listing still works

**Objective:** Keep dashboard/API listings in sync with `presets/index.json`.

**Files:**
- No new files; verification only.

**Commands:**

```bash
npm run test:server -- --test-name-pattern=presets
```

If the test runner does not support `--test-name-pattern`, run:

```bash
npm run test:server
```

Expected: preset listing tests pass and include the new builtin via `presets/index.json`.

### Task 12: Smoke-evaluate the Order with the preset harness

**Objective:** Prove the workflow can complete a representative mixed engineering task, not merely parse.

**Files:**
- Generated under `.eval/` and `.eval-sandbox/`; do not commit generated logs/artifacts unless the repo already tracks a specific fixture.

**Commands:**

```bash
cargo build --bin ralph
RALPH_EVAL_BINARY=target/debug/ralph ./tools/evaluate-preset.sh engineering-order claude smoke
```

Expected:
- Exit code `0` or a clearly classified loop-completion signal.
- Metrics show `completion_promise_reached: true` for `ORDER_COMPLETE`.
- Activated hats include at least `steward`, `planner`, `builder`, `verifier`, `reviewer`, and `finalizer` for the smoke task.
- `.eval/logs/engineering-order/latest/metrics.json` records completion.

If smoke fails:
1. Inspect `.eval/logs/engineering-order/latest/output.log` and `session.jsonl`.
2. Identify whether failure is prompt/event contract, backend timeout, missing tool, or real workflow issue.
3. Fix the preset/harness and rerun smoke before moving on.

### Task 13: Run repo-level validation

**Objective:** Satisfy repo quality gates after source changes.

**Commands:**

```bash
cargo test -p ralph-cli
cargo test -p ralph-api
cargo test
cargo run -p ralph-e2e -- --mock
./scripts/sync-embedded-files.sh check
git diff --check
```

Expected: all pass. If `cargo test` surfaces unrelated pre-existing failures, classify them and fix if tractable; otherwise document exact failing tests and why they are unrelated.

### Task 14: Cleanup generated artifacts and inspect diff

**Objective:** Leave a shippable diff, not a Ralph/preset-evaluation junk pile.

**Commands:**

```bash
git status --short
git diff --stat
git diff -- presets/engineering-order.yml crates/ralph-cli/src/presets.rs scripts/sync-embedded-files.sh presets/index.json presets/README.md docs/guide/presets.md tools/preset-test-tasks.yml
```

Cleanup rules:

- Do not commit `.eval/`, `.eval-sandbox/`, `.ralph/`, `.agents/scratchpad/`, generated logs, or prompt scratch files unless explicitly intended.
- If generated artifacts keep reappearing, update `.gitignore` in a focused way.
- Confirm the final diff contains only source/docs/tests/harness changes needed for the Order.

### Task 15: Commit coherent changes

**Objective:** Land one reviewable commit.

**Commands:**

```bash
git add \
  presets/engineering-order.yml \
  crates/ralph-cli/presets/engineering-order.yml \
  crates/ralph-cli/src/presets.rs \
  scripts/sync-embedded-files.sh \
  presets/index.json \
  presets/README.md \
  docs/guide/presets.md \
  tools/preset-test-tasks.yml

git add tools/evaluate-preset.sh  # only if Task 7 changed it

git commit -m "feat: add engineering order preset"
```

Expected: a single coherent commit containing the public builtin, docs, tests, and smoke-task support.

---

## Acceptance criteria

- Given a mixed engineering prompt, when a user runs `ralph run -c ralph.yml -H builtin:engineering-order -p "..."`, Ralph classifies the work and creates an order dossier.
- Given a mutating implementation/refactor/test/docs/security/perf task, the Order creates runtime tasks, executes one wave at a time, verifies with tests/commands, runs adversarial review, and only then completes.
- Given a bug prompt, the Order reproduces or explicitly fails to reproduce, forms a falsifiable hypothesis, applies a minimal fix if needed, and verifies the original repro plus regression.
- Given a read-only architecture/research/review prompt, the Order produces evidence-backed output without code changes and still emits `order.validated` before completion.
- Given a release-readiness prompt, the Order checks clean diff, generated artifact hygiene, package/build smoke, docs/changelog, and review before completion.
- Given an irreversible/destructive/publishing/deploying action, the Order emits `human.interact` instead of proceeding silently.
- `cargo test`, sync check, preflight, and preset smoke evaluation pass.

## Risks and tradeoffs

- **Risk: the preset becomes a bloated PDD clone.** Mitigation: make the first version a router + concise duty prompts; no new runtime semantics.
- **Risk: required events block read-only tracks.** Mitigation: force every track to emit universal `order.classified`, `order.plan.ready`, and `order.validated`, even when the “plan” is a review/research plan.
- **Risk: too many hats increase iteration count.** Mitigation: steward can route narrow tasks directly to one executor + verifier/reviewer/finalizer; full path only for mutating/mixed work.
- **Risk: `evaluate-preset.sh` fails on macOS due to GNU `timeout`.** Mitigation: Task 7 makes timeout portable before relying on smoke evaluation.
- **Risk: CLI/web preset listings drift.** Mitigation: update embedded `PRESETS`, `presets/index.json`, docs, sync script, and tests together.
- **Risk: agents skip `ralph emit` and only write prose.** Mitigation: every hat prompt explicitly says the turn is incomplete until a real emit succeeds; contract tests assert that phrase.

## Open questions

- Should the public name be `engineering-order`, `software-engineering-order`, or something more Ralph-specific like `se-order`? Plan assumes `engineering-order`.
- Should the Order eventually delegate to narrower builtins as subprocesses? MVP says no; use one preset and Ralph's existing event bus first.
- Should this become the recommended default in docs over `code-assist` for mixed tasks? Recommendation: docs should say `code-assist` remains default for obvious implementation; `engineering-order` is default for ambiguous/mixed engineering responsibility.

## Final verification checklist

Before calling the task done:

```bash
./scripts/sync-embedded-files.sh check
cargo test -p ralph-cli
cargo test -p ralph-api
cargo test
cargo run -p ralph-e2e -- --mock
cargo run --bin ralph -- init --list-presets | grep -F "engineering-order"
cargo run --bin ralph -- preflight -c ralph.yml -H builtin:engineering-order
RALPH_EVAL_BINARY=target/debug/ralph ./tools/evaluate-preset.sh engineering-order claude smoke
git diff --check
git status --short
```

The final report should include:

- Branch and commit SHA.
- Preset name and usage command.
- Tests run with actual pass/fail output.
- Smoke evaluation metrics path.
- Any known limitations or follow-up candidates.
