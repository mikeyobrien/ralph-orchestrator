# v3 completion audit

Phase 0 of the v3 completion brief. One row per bead, with the tracker claim, a
verdict, the command plus output excerpt that decides it, and the work that
remains.

- Branch: `v3/complete` at `df2449a`
- Date: 2026-09-22
- Base for the tracker: `.beads/issues.jsonl` at `aff233d`
- Source edits made by this audit: none
- Added after the audit, by operator steer `task-1790100842-ebbb`: the
  `## Related upstream context` section (`autoloop#86`). It records no verdict
  and edits no audit row

## Method

The tracker is a hypothesis. Every row was re-measured against the tree, the
remote refs, and the upstream repo. The four evidence logs this audit
consolidates are:

- `.ralph/specs/v3-complete/logs/step-01-engine-flip.md`
- `.ralph/specs/v3-complete/logs/step-01-remnant-drift.md`
- `.ralph/specs/v3-complete/logs/step-01-gate-branches.md`
- `.ralph/specs/v3-complete/logs/step-01-bead-claims.md`

Verdict vocabulary: `landed`, `stale-open`, `genuinely-open`, `blocked`.

Tracker census for the run, so the table can be checked against a count:

```bash
python3 -c "import json,collections;rows=[json.loads(l) for l in open('.beads/issues.jsonl') if l.strip()];print(len(rows),dict(collections.Counter(r['status'] for r in rows)))"
```

```text
43 {'closed': 36, 'open': 6, 'tombstone': 1}
```

Scope: the six open rows plus the tombstone, which is the set the brief puts in
front of this phase. The 36 closed rows are carried from the census and are not
re-audited here; see the closing section for why.

## Audit table

| Bead | Tracker claim | Verdict | Evidence | Remaining work |
| --- | --- | --- | --- | --- |
| `a7e` | Epic: replace the in-house engine with the autoloop SDK | genuinely-open | `python3` census above prints `6` open rows including `a7e.8` and `a7e.10`; `grep -c 'release_gate: .ralph/specs/v3-ga-readiness.spec.md' .ralph/specs/v3-autoloops-cutover.spec.md` prints `1` while `git cat-file -e origin/main:.ralph/specs/v3-ga-readiness.spec.md` exits non-zero | Close `a7e.8` and `a7e.10`, author the named GA gate spec, then close the epic per its own acceptance |
| `a7e.8` | Map ralph waves onto autoloop parallel model; certify wave-review; blocked on autoloop#35 | genuinely-open | `cargo test -p ralph-cli ports_wave_review_preset_to_declarative_autoloop_topology` prints `test autoloop_preset_gen::tests::ports_wave_review_preset_to_declarative_autoloop_topology ... ok` and `1 passed; 0 failed`; upstream `#35` is CLOSED per `gh api` in the remnant log | Run `presets/wave-review.yml` under autoloop and capture concurrent branches in the journal; resolve the 0.10.x to 0.11.0 drift (inferred) |
| `a7e.10` | Delete `event_loop`, `hat_registry`, `event_bus`, `hatless_ralph`, `wave_*`; flip the default | genuinely-open | `cat crates/ralph-core/src/config.rs` shows the v3 rejection at `:512` and the test `core_engine_rejects_removed_ralph_engine` at `:2689`; `wc -l crates/ralph-core/src/hat_registry.rs` prints `468` with a live consumer at `crates/ralph-cli/src/hats.rs:132`; `wc -l crates/ralph-proto/src/event_bus.rs` prints `401` with zero production callers | Relocate or rework `hat_registry.rs` with its justified `ralph hats` caller, delete `event_bus.rs`, keep the v3 rejection message |
| `ga3-c4-dashboard-dead-svf` | Both dashboard readers are severed | genuinely-open | `grep -n` shows the watcher at `crates/ralph-api/src/event_watcher.rs:40,100`, `EventLogger` at `crates/ralph-core/src/event_logger.rs:111` with `#[cfg(test)]` at `:265`, the Node parser at `backend/ralph-web-server/src/runner/RalphEventParser.ts:40`, and the shipped warning at `crates/ralph-cli/src/web.rs:412` | Port a parser for `.ralph/autoloop/events.ndjson` and prove it with a captured payload, or mark the dashboard non-functional in the README and delete both readers |
| `landing-untracked-sweep-yxv` | Landing auto-commit sweeps untracked operator files | genuinely-open | `grep -n 'add", "-A"' crates/ralph-core/src/git_ops.rs` prints `182`, called from `crates/ralph-core/src/landing.rs:137`; `git show --stat 1e67e52` prints `chore: auto-commit before merge (loop primary)` | Write the failing regression test first, narrow the commit scope to loop-touched paths or an allowlist plus ignore rules, confirm RED then GREEN |
| `tui-help-wave-stale-5hu` | Help overlay still lists the deleted Wave Workers section | genuinely-open | `grep -n Wave crates/ralph-tui/src/widgets/help.rs` prints `114`, `119`, `127`; `grep -n Char('w') crates/ralph-tui/src/input.rs` prints `98` with a live `Action::EnterWaveView`; `grep -rn WaveStarted crates/` returns no production producer | Follow Step 4's wave verdict: delete the help section, the `w` keybinding, the action, and the view state, or update the help text if waves are certified |
| `vp6` | Tombstone: `core.engine='ralph'` silently ran autoloop; no test | landed | `crates/ralph-core/src/config.rs:512` rejects any non-autoloop engine before warning suppression, message literal at `:2362`, test `core_engine_rejects_removed_ralph_engine` at `:2689`; the engine-flip log records `1 passed; 0 failed` | None. Terminal tombstone whose original defect has since landed |

## Per-bead evidence

### a7e (epic)

Command and census excerpt:

```bash
python3 -c "import json,collections;rows=[json.loads(l) for l in open('.beads/issues.jsonl') if l.strip()];print(len(rows),dict(collections.Counter(r['status'] for r in rows)))"
```

```text
43 {'closed': 36, 'open': 6, 'tombstone': 1}
```

The six open ids are `a7e`, `a7e.8`, `a7e.10`,
`ga3-c4-dashboard-dead-svf`, `landing-untracked-sweep-yxv`,
`tui-help-wave-stale-5hu`. Children `a7e.1` through `a7e.11` are otherwise
closed, so the epic is two children away from terminal.

The epic also requires the GA gate. Command and excerpt:

```bash
grep -n 'release_gate' .ralph/specs/v3-autoloops-cutover.spec.md
for r in origin/integration/v3-prerelease origin/wip/v3-prerelease-rollup origin/main; do
  git cat-file -e "$r:.ralph/specs/v3-ga-readiness.spec.md" && echo "$r PRESENT" || echo "$r ABSENT"
done
```

```text
10:release_gate: .ralph/specs/v3-ga-readiness.spec.md
origin/integration/v3-prerelease         ABSENT
origin/wip/v3-prerelease-rollup          ABSENT
origin/main                              ABSENT
```

Verdict `genuinely-open`. The epic's own high-level acceptance (a live
multi-role run, e2e and smoke passing, a migration guide and CHANGELOG entry) is
not measured here and is labeled inferred as of yet unmet; the two open children
alone keep the row open.

### a7e.8 (waves under autoloop)

The tracker says BLOCKED on autoloop#35. That blocker has cleared. From the
remnant and drift log, `gh api` re-confirms `#34`, `#35`, `#37`, `#38`, `#39`
closed and `#40`, `#41`, `#42` merged. So the row is no longer `blocked`, it is
`genuinely-open`.

The mapping has landed. Command and excerpt:

```bash
sed -n '15,20p' crates/ralph-cli/src/autoloop_preset_gen.rs
cargo test -p ralph-cli ports_wave_review_preset_to_declarative_autoloop_topology
```

```text
| `hats.<id>.concurrency` (when > 1)      | `[[role]] concurrency`            |
| `hats.<id>.aggregate`                   | `[[role]] aggregate`              |
```

```text
test autoloop_preset_gen::tests::ports_wave_review_preset_to_declarative_autoloop_topology ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 414 filtered out
```

The wave CLI is gone and the shipped-artifact guard passes. Command and excerpt:

```bash
grep -rn '"wave"\|wave emit' crates/ralph-cli/src/*.rs
cargo test -p ralph-core --test shipped_artifacts
```

```text
(no output, exit 1)
```

```text
test shipped_artifacts_do_not_reference_deleted_wave_cli ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

What is missing is the certification the bead asks for: an actual run of
`presets/wave-review.yml` under the engine with the concurrent branches visible
in the journal. The translation is proven by unit test, the runtime behavior is
not. Second gap: the generated keys are documented as autoloop 0.10.x
(`autoloop_preset_gen.rs:158`, `:212`) and the installed engine is 0.11.0
(`autoloop --version`), so whether `[[role]] concurrency` and `aggregate` still
bind is unverified. Labeled inferred; Step 12 owns the runtime check.

### a7e.10 (last in-house remnant)

The delete and the default flip are mostly landed. Command and excerpt:

```bash
sed -n '511,513p' crates/ralph-core/src/config.rs
grep -n 'fn core_engine_rejects_removed_ralph_engine' crates/ralph-core/src/config.rs
find crates \( -name 'event_loop*' -o -name 'hatless_ralph*' -o -name 'wave_*' \) -print
```

```text
        // Hard validation must run before warning suppression.
        if self.core.engine != "autoloop" {
            return Err(ConfigError::InvalidEngine {
```

```text
2689:    fn core_engine_rejects_removed_ralph_engine() {
```

```text
(no output)
```

The v3 message literal sits at `config.rs:2362`:

```text
        "Invalid core.engine '{engine}': the in-house engine was removed in v3; remove the field or set autoloop."
```

Two remnants survive, not one. Command and excerpt:

```bash
wc -l crates/ralph-core/src/hat_registry.rs crates/ralph-proto/src/event_bus.rs
sed -n '26p;84p' crates/ralph-core/src/lib.rs
sed -n '15p;25p' crates/ralph-proto/src/lib.rs
sed -n '132p' crates/ralph-cli/src/hats.rs
```

```text
  468 crates/ralph-core/src/hat_registry.rs
  401 crates/ralph-proto/src/event_bus.rs
```

```text
mod hat_registry;
pub use hat_registry::HatRegistry;
```

```text
mod event_bus;
pub use event_bus::EventBus;
```

```text
    let registry = HatRegistry::from_config(&config);
```

So the verdict is `genuinely-open`, but the tracker scope is stale. The
described work (delete the engine, flip the default) has landed. The residual is
a relocate-or-rework of `hat_registry.rs`, which has exactly one production
consumer at `hats.rs:132`, and a bare deletion of `event_bus.rs`, which has zero
production callers. Step 3 owns it.

### ga3-c4-dashboard-dead-svf

Command and excerpt for the watcher:

```bash
grep -rn 'event_watcher' crates/ --include=*.rs
sed -n '40p;100p' crates/ralph-api/src/event_watcher.rs
```

```text
crates/ralph-api/src/lib.rs:6:pub mod event_watcher;
crates/ralph-api/src/transport.rs:90:    let _watcher_handle = crate::event_watcher::spawn_watcher(
```

```text
    let marker_path = workspace_root.join(".ralph/current-events");
        match serde_json::from_str::<EventRecord>(&line) {
```

Command and excerpt for the dead logger:

```bash
sed -n '111p;265p' crates/ralph-core/src/event_logger.rs
grep -rn 'EventLogger' crates/ --include=*.rs
```

```text
pub struct EventLogger {
#[cfg(test)]
```

```text
crates/ralph-core/src/lib.rs:73:pub use event_logger::{EventHistory, EventLogger, EventRecord};
```

Every `EventLogger::new` line is below the test marker at `:265`, so there is no
production caller.

Command and excerpt for the v3 engine stream, which the watcher never sees:

```bash
sed -n '393p' crates/ralph-cli/src/autoloop_engine.rs
sed -n '16p' crates/ralph-core/src/engine_state.rs
grep -rn 'autoloop-events' crates/ backend/ --include=*.rs --include=*.ts
```

```text
    let events_path = engine_state_root.join("events.ndjson");
    workspace_root.join(".ralph").join("autoloop")
```

```text
crates/ralph-cli/src/autoloop_robot.rs:451 ... (test fixture)
crates/ralph-bench/src/autoloop_task.rs:12:const EVENTS_FILE: &str = ".ralph/bench-autoloop-events.ndjson";
```

Command and excerpt for the Node parser and its wiring:

```bash
sed -n '40p' backend/ralph-web-server/src/runner/RalphEventParser.ts
sed -n '83p;93p' backend/ralph-web-server/src/runner/RalphTaskHandler.ts
```

```text
export class RalphEventParser {
    const eventParser = new RalphEventParser((event) => {
      eventParser.parseLine(entry.line);
```

Command and excerpt for the shipped disclosure:

```bash
sed -n '412p' crates/ralph-cli/src/web.rs
```

```text
    println!("{DASHBOARD_LIVE_STATE_CAVEAT}");
```

Verdict `genuinely-open`. Both readers are severed from the live stream. The
disclosure at `web.rs:412` is mitigation, not the acceptance the bead requires.
Details and two corrections are in the bead-claims log.

### landing-untracked-sweep-yxv

Command and excerpt:

```bash
grep -n 'add", "-A"' crates/ralph-core/src/git_ops.rs
sed -n '150,151p' crates/ralph-core/src/git_ops.rs
sed -n '137p' crates/ralph-core/src/landing.rs
```

```text
182:            .args(["add", "-A"])
```

```text
/// This stages all changes (untracked, staged, unstaged) and creates a commit
/// with a standardized message.
```

```text
            match auto_commit_changes(workspace, &loop_id) {
```

There is no allowlist and no touched-path scoping. The only filter is
`.gitignore`, which git applies implicitly, and the one relevant test is
`test_auto_commit_only_gitignored_files` at `git_ops.rs:574`. The failure is
live, not theoretical. Command and excerpt:

```bash
git show --stat --format='%h %s' 1e67e52
```

```text
1e67e52 chore: auto-commit before merge (loop primary)
 .../tui-stream-history-backpressure.code-task.md   | 78 ++++++++++++++
 1 file changed, 78 insertions(+)
```

Verdict `genuinely-open`. Note the bead cites `landing.rs`, which is the caller;
the sweep line is `git_ops.rs:182`.

### tui-help-wave-stale-5hu

Command and excerpt:

```bash
grep -n -i 'wave' crates/ralph-tui/src/widgets/help.rs
grep -rn "Char('w')" crates/ralph-tui/src/
grep -rni 'wave' crates/ralph-tui/src/ --include=*.rs | wc -l
```

```text
crates/ralph-tui/src/widgets/help.rs:114:            "Wave Workers:",
crates/ralph-tui/src/widgets/help.rs:119:            Span::raw("      Enter wave worker view"),
crates/ralph-tui/src/widgets/help.rs:127:            Span::raw("    Exit wave view"),
```

```text
crates/ralph-tui/src/input.rs:98:        KeyCode::Char('w') => Action::EnterWaveView,
```

```text
236
```

No wave event producer exists. Command and excerpt:

```bash
grep -rn 'WaveStarted' crates/ --include=*.rs
```

```text
crates/ralph-core/src/diagnostics/orchestration.rs:34:    WaveStarted {
crates/ralph-core/src/diagnostics/orchestration.rs:117:            OrchestrationEvent::WaveStarted {
crates/ralph-proto/src/json_rpc.rs:349:    WaveStarted {
crates/ralph-tui/src/rpc_source.rs:445:        RpcEvent::WaveStarted {
```

`:34` is the variant and `:117` is a serialization fixture. `:349` defines the
RPC event, `:445` consumes it, and nothing produces it.

Verdict `genuinely-open`. The tracker calls this minor and help-text only. It is
not: the `w` keybinding is live at `input.rs:98`, the action is handled at
`app.rs:131`, and the view drives the header tag and buffer swap. Step 7 must
follow Step 4's decision on whether waves are certified or removed.

### vp6 (tombstone)

The tracker row is deleted: `delete_reason: delete`, `deleted_at:
2026-07-20T02:09:47.062711Z`, `deleted_by: rook`. Its original claim was that
`core.engine='ralph'` silently ran autoloop and no test covered it.

That defect has since landed. Command and excerpt:

```bash
sed -n '511,513p' crates/ralph-core/src/config.rs
sed -n '2361,2362p' crates/ralph-core/src/config.rs
grep -n 'fn core_engine_rejects_removed_ralph_engine' crates/ralph-core/src/config.rs
```

```text
        // Hard validation must run before warning suppression.
        if self.core.engine != "autoloop" {
            return Err(ConfigError::InvalidEngine {
```

```text
    #[error(
        "Invalid core.engine '{engine}': the in-house engine was removed in v3; remove the field or set autoloop."
```

```text
2689:    fn core_engine_rejects_removed_ralph_engine() {
```

Verdict `landed`. The row is terminal and no work remains.

## Corrections to the brief and the tracker

These were found while verifying and must not propagate into later steps.

1. **`.ralph/autoloop-events.ndjson` does not exist.** The live engine stream is
   `.ralph/autoloop/events.ndjson`, built from `engine_state_root` at
   `crates/ralph-cli/src/autoloop_engine.rs:393` with the root pinned at
   `crates/ralph-core/src/engine_state.rs:16`. The only `autoloop-events.ndjson`
   references are test fixtures and `ralph-bench`'s separate
   `bench-autoloop-events.ndjson`.
2. **`event_bus` is not gone from the tree.** The brief says it is.
   `crates/ralph-proto/src/event_bus.rs` is 401 lines, declared at
   `ralph-proto/src/lib.rs:15` and re-exported at `:25`, with zero production
   callers. a7e.10 covers two modules, not one.
3. **`hat_registry` is not dead.** It has exactly one production consumer, the
   `ralph hats` command at `crates/ralph-cli/src/hats.rs:132`, so a7e.10 is a
   relocate or rework, not a bare delete.
4. **The brief's `config.rs` line numbers are stale.** It cites `2188` and
   `2434`. The live lines are `512` and `2689`. The main merge shifted them.
5. **The landing sweep line is `git_ops.rs:182`, not in `landing.rs`.**
   `landing.rs:137` is the caller.
6. **The TUI wave surface is broader than help text.** It includes a live `w`
   keybinding, an action, a view state machine, and a header mode, with no
   event producer behind it.
7. **The tracker has no `a7e.18`.** The v2 gap analysis names
   `ralph-orchestrator-v3-autoloops-backend-a7e.18` as the replacement bead for
   dashboard live state. The census shows no such id; the open row is
   `ga3-c4-dashboard-dead-svf`. Either the id was never created or it was
   renamed. Step 5 must not look for it.
8. **The task text's rollup count of 14 is wrong.** The measured rollup-only
   delta is 22, which matches the brief's own table.
9. **Version drift is unverified.** `autoloop_preset_gen.rs:158` and `:212` say
   0.10.x; the installed and published engine is 0.11.0. Whether a mapping
   breaks is labeled inferred and is a Step 12 runtime check.
10. **Upstream is no longer a blocker.** Issues `#34`, `#35`, `#37`, `#38`,
    `#39` are closed and PRs `#40`, `#41`, `#42` are merged. The cutover spec's
    "blocked on upstream" narrative is obsolete.

## Related upstream context

Operator steer `task-1790100842-ebbb`
(`operator-steer:v3-complete:record-rfc-86`, P1, injected
`2026-09-22T18:14:02Z`) asked for one upstream reference to be recorded in this
context file. It is context for a later step. It is not evidence for any row in
the table above, it changes no verdict, and it reopens no blocker.

- **Issue:** [autoloop#86 — RFC: dispatch.jev, bus mode with a calibrated
  fail-safe router (opt-in)](https://github.com/mikeyobrien/autoloop/issues/86),
  state `open`, label `enhancement`, filed `2026-09-22T18:06:04Z`.
- **What it proposes:** an opt-in bus mode in which roles publish to one event
  bus and a calibrated decision model (Jev, System One) selects the next
  specialist per handoff. Uncertainty below a confidence floor falls back to
  the declared topology, then to a human ask, then stops fail-closed. Completion
  stays deterministic and the RFC states that nothing about the default mode
  changes. The RFC names the `[routing.jev]` component as the skeleton it
  extends from one decision at loop start to one decision per handoff.
- **Why it is recorded:** it is the upstream counterpart of two plan steps.
  Step 9, "Jev routing parity, no silent drop (Phase 2c.1)", owns the
  `core.routing.jev` surface, and Step 11, "Topology routing surface and
  feasibility (Phase 2c.3)", owns the routing surface whose demo already accepts
  either an implementation at a proven seam or "a filed upstream issue with the
  seam table". Step 11's planner pass must read this issue before choosing that
  branch, and Step 9's must reconcile the `[routing.jev]` wording below.
- **Correction 10 above stands.** The issues that blocked `a7e.8` (#34, #35,
  #37, #38, #39) are closed. #86 is newly open, opt-in, and default-mode
  neutral, so it is context, not a dependency.

The premise was re-measured this round rather than inherited from the steer:

```bash
# A. the installed engine, and the upstream repo it names
npm ls -g --depth=0 2>/dev/null | grep -i autoloop
python3 -c "import json,os; p=os.path.join(os.popen('npm root -g').read().strip(),'@mobrienv/autoloop/package.json'); d=json.load(open(p)); print(d['version'], d['repository']['url'])"

# B. the referenced issue
curl -sS -H 'Accept: application/vnd.github+json' \
  https://api.github.com/repos/mikeyobrien/autoloop/issues/86 \
| python3 -c "import json,sys; d=json.load(sys.stdin); print(d['number'], d['state'], '|', d['title']); print(d['html_url']); print('labels:', [l['name'] for l in d['labels']]); print('created:', d['created_at'])"

# C. the [routing.jev] skeleton the RFC names: does the engine ship it?
R="$(npm root -g)/@mobrienv/autoloop"
echo "files scanned: $(find "$R" -type f -not -name '*.map' | wc -l)"
for p in 'jev' 'typesafe\|noul\|routes_file'; do
  printf '%s -> %s occurrences\n' "$p" "$(grep -rniI --exclude=*.map -c "$p" "$R" 2>/dev/null | awk -F: '{s+=$2} END{print s+0}')"
done
T="$(mktemp -d)"; (cd "$T" && git init -q . && echo "pristine config show, jev matches: $(autoloop config show --preset code-assist 2>/dev/null | grep -ci jev)"); rm -rf "$T"
echo "upstream main, routing.jev paths:"
gh api -X GET 'search/code' -f q='routing.jev repo:mikeyobrien/autoloop' --jq '.items[].path' 2>/dev/null
echo "upstream main sha: $(gh api repos/mikeyobrien/autoloop/commits/main --jq '.sha' 2>/dev/null)"
echo "published latest: $(npm view @mobrienv/autoloop dist-tags.latest 2>/dev/null)"
```

```text
├── @mobrienv/autoloop@0.11.0
0.11.0 git+https://github.com/mikeyobrien/autoloop.git
86 open | RFC: dispatch.jev — bus mode with a calibrated fail-safe router (opt-in)
https://github.com/mikeyobrien/autoloop/issues/86
labels: ['enhancement']
created: 2026-09-22T18:06:04Z
files scanned: 6041
jev -> 0 occurrences
typesafe\|noul\|routes_file -> 0 occurrences
pristine config show, jev matches: 0
upstream main, routing.jev paths:
docs/reference/jev-routing.md
packages/harness/src/jev-routing.ts
docs/reference/configuration.md
packages/harness/src/emit.ts
packages/harness/test/harness/jev-routing.test.ts
packages/harness/test/harness/iteration.test.ts
packages/harness/test/harness/config-helpers.test.ts
upstream main sha: fce46cc12ce5fecdd4757796b681ec51a0deb1c9
published latest: 0.11.0
```

The link between the tracker and the installed package is direct: the local
package is `@mobrienv/autoloop` 0.11.0 and its `repository` field is
`github.com/mikeyobrien/autoloop`, so `#86` is an issue in the same tracker the
shipped engine comes from, alongside the `#34` to `#42` entries in correction
10.

Part C is a correction, not a citation. The brief's Phase 2c.1 opens with
autoloop already shipping `[routing.jev]` in a preset, and this RFC calls the
same component the skeleton it extends. That is true of the upstream repo and
false of the engine this branch ships against:

| Where | `[routing.jev]` present | Evidence |
| --- | --- | --- |
| Upstream `main` at `fce46cc`, 2026-09-19 | yes | code search returns 7 paths, with the implementation at `packages/harness/src/jev-routing.ts` and the doc at `docs/reference/jev-routing.md` |
| Installed `@mobrienv/autoloop` 0.11.0 | no | `jev`, `typesafe`, `noul`, and `routes_file` each occur 0 times across 6041 package files; a clean repo's `autoloop config show --preset code-assist` prints no `[routing]` section and 0 `jev` matches |
| npm registry | no newer release | `npm view @mobrienv/autoloop dist-tags.latest` is `0.11.0`, published 2026-09-10, which predates the `main` commit above |

Two consequences for the later steps:

- Step 9 cannot "confirm each path by execution" for `[routing.jev]` on the
  installed engine, because no reader for the block exists there. Ralph can
  still emit the block and fail closed, but a live-path confirmation needs an
  engine build newer than 0.11.0, and until one is published that half of
  Step 9's demo is blocked on an unreleased engine rather than on Ralph code.
- Step 11's seam table gains this RFC as candidate evidence, and its "filed
  upstream issue" branch now has a live counterpart to link.

One related question is left inferred rather than measured, because it is
Step 9's and Step 12's work: whether the same release gap also accounts for the
lifecycle-hook engine the brief attributes to autoloop #38 and the
completion-gate store override it attributes to #36.

## Closed rows

36 rows are closed and one is a tombstone. They were not re-audited one by one
because the brief scopes this phase to the open rows plus the tombstone, and
because the four Step 1 verification tasks did not produce per-closed-bead
evidence. Two closed rows are load-bearing and are covered above:

- `a7e.1` through `a7e.11` except `.8` and `.10` are closed. Their closure is
  consistent with the engine flip having landed (`config.rs:512`).
- `vp6` is the tombstone whose original defect has landed.

Anything beyond that is `inferred` from tracker status rather than measured. If
the GA gate requires per-bead proof for a closed row, that row needs its own
verification pass before the epic closes.
