# Step 1 evidence: the three open defect beads

Task: `task-1790091216-f8a7`
Key: `code-assist:v3-complete:step-01:verify-bead-claims`
Branch: `v3/complete` at `df2449a`
Date: 2026-09-22
Source edits: none.

Each claim names the file and line that carries the defect behavior, or refutes
the claim. Every claim carries the command and an output excerpt.

## Bead 1. `ralph-orchestrator-ga3-c4-dashboard-dead-svf`

Tracker claim: `ralph-api` reads `.ralph/events.jsonl` through the
`current-events` marker, `EventLogger` has zero production callers, the Node
backend parses a stdout JSONL format that no longer exists, and the autoloop path
writes only `.ralph/autoloop-events.ndjson`.

### Commands

```bash
grep -rn 'event_watcher' crates/ --include=*.rs
grep -rn 'EventLogger' crates/ --include=*.rs
grep -n '#\[cfg(test)\]' crates/ralph-core/src/event_logger.rs
grep -rn 'current-events' crates/ --include=*.rs
grep -rn 'autoloop-events' crates/ backend/ --include=*.rs --include=*.ts
grep -rn 'RalphEventParser\|parseLine' backend/ralph-web-server/src --include=*.ts
grep -rn 'DASHBOARD_LIVE_STATE_CAVEAT' crates/ralph-cli/src/web.rs
```

### Output excerpt

Watcher entry points and parse site:

```text
crates/ralph-api/src/lib.rs:6:pub mod event_watcher;
crates/ralph-api/src/transport.rs:90:    let _watcher_handle = crate::event_watcher::spawn_watcher(
crates/ralph-api/src/event_watcher.rs:40:    let marker_path = workspace_root.join(".ralph/current-events");
crates/ralph-api/src/event_watcher.rs:70:        Some(path) => path,
crates/ralph-api/src/event_watcher.rs:100:        match serde_json::from_str::<EventRecord>(&line) {
```

`EventLogger` construction sites, all inside its own test module:

```text
crates/ralph-core/src/event_logger.rs:111:pub struct EventLogger {
crates/ralph-core/src/event_logger.rs:265:#[cfg(test)]
crates/ralph-core/src/event_logger.rs:279:        let mut logger = EventLogger::new(&path);
crates/ralph-core/src/lib.rs:73:pub use event_logger::{EventHistory, EventLogger, EventRecord};
```

The file is 593 lines and the test module starts at 265, so every
`EventLogger::new` call is a test. No other crate constructs it.

The only production writer of the `current-events` marker in v3:

```text
crates/ralph-cli/src/autoloop_robot.rs:34:    pub(crate) fn install(workspace: &Path) -> Result<Self> {
crates/ralph-cli/src/autoloop_robot.rs:50:        let marker = ralph_dir.join("current-events");
crates/ralph-cli/src/autoloop_robot.rs:52:        fs::write(&marker, format!("{HUMAN_EVENTS_REL}\n"))
crates/ralph-cli/src/autoloop_engine.rs:435:        current_events_guard = Some(
crates/ralph-cli/src/autoloop_engine.rs:436:            crate::autoloop_robot::CurrentEventsGuard::install(&workspace)
```

`HUMAN_EVENTS_REL` is `.ralph/human-events.jsonl` at `autoloop_robot.rs:25`. The
guard is installed at `autoloop_engine.rs:428-441` only when `!launch.tui` and
`config.robot.enabled` and the loop is primary.

The engine's structured stream, the path the watcher never sees:

```text
crates/ralph-cli/src/autoloop_engine.rs:393:    let events_path = engine_state_root.join("events.ndjson");
crates/ralph-core/src/engine_state.rs:16:    workspace_root.join(".ralph").join("autoloop")
crates/ralph-adapters/src/autoloop_runner.rs:238:            args.push("--events".to_string());
```

The Node parser and its live wiring:

```text
backend/ralph-web-server/src/runner/RalphEventParser.ts:4: * Parses Ralph orchestrator events from stdout lines.
backend/ralph-web-server/src/runner/RalphTaskHandler.ts:83:    const eventParser = new RalphEventParser((event) => {
backend/ralph-web-server/src/runner/RalphTaskHandler.ts:93:      eventParser.parseLine(entry.line);
backend/ralph-web-server/src/serve.ts:82:    createRalphTaskHandler({
backend/ralph-web-server/src/api/LogBroadcaster.ts:270:  broadcastEvent(taskId: string, event: RalphEvent): void {
```

The shipped disclosure:

```text
crates/ralph-cli/src/web.rs:27:const DASHBOARD_LIVE_STATE_CAVEAT: &str = "WARNING: The Ralph web dashboard does NOT render live loop state under the v3 autoloop engine yet; the parser port is tracked by ga3-c4-dashboard-dead-svf.";
crates/ralph-cli/src/web.rs:412:    println!("{DASHBOARD_LIVE_STATE_CAVEAT}");
```

### Finding

Verdict: `genuinely-open`. Every part of the claim reproduces, with two
corrections to the bead's wording.

1. The ralph-api watcher is real and wired. `spawn_watcher` runs at server
   startup (`transport.rs:90`), reads the path named by `.ralph/current-events`
   (`event_watcher.rs:40`, `:70`), parses each line as `EventRecord`
   (`:100`), and silently drops unparseable lines at debug or trace level
   (`:103`). `ralph web` launches exactly this server (`crates/ralph-cli/src/web.rs:474`,
   `cargo run -p ralph-api`).
2. `EventLogger` has zero production callers. Confirmed: the struct is defined
   at `event_logger.rs:111`, the `#[cfg(test)]` module starts at `:265`, and the
   only non-test reference is the re-export at `lib.rs:73`. Nothing in the tree
   writes `.ralph/events.jsonl` through it.
3. The Node backend stdout parser is also real and wired, not dead code.
   `RalphEventParser` is constructed at `RalphTaskHandler.ts:83` and fed every
   stdout line at `:93`, and the handler is registered at `serve.ts:82`. So the
   bead's phrase "parses a stdout JSONL format that no longer exists" is half
   right: the parser runs, but the v3 engine sends its structured stream to
   `--events <path>` (`autoloop_runner.rs:238`), not to stdout, so the parser has
   nothing structured to match.
4. Correction to the named event file. The bead says the autoloop path writes
   only `.ralph/autoloop-events.ndjson`. That path appears nowhere in production
   code. Its only references are test fixtures at `autoloop_robot.rs:451`,
   `:537`, `:597` and the unrelated
   `crates/ralph-bench/src/autoloop_task.rs:12`
   (`.ralph/bench-autoloop-events.ndjson`). The live engine stream is
   **`.ralph/autoloop/events.ndjson`**, built from `engine_state_root` at
   `autoloop_engine.rs:393` with the root pinned by
   `crates/ralph-core/src/engine_state.rs:16`.
5. Correction to the marker story. The marker is not absent in v3. The headless
   robot guard writes it and points it at `.ralph/human-events.jsonl`
   (`autoloop_robot.rs:50-52`). `EventRecord` tolerates the agent-written shape
   (the `iteration` and `hat` fields are `#[serde(default)]` at
   `event_logger.rs:31-38`), so in that one mode the watcher publishes human
   relay events and not engine events. In TUI runs the marker is never written,
   so the watcher logs "no active events file" (`event_watcher.rs:73`) and
   returns nothing.

Remaining work for Step 5: pick the honest outcome. Either point the watcher at
`.ralph/autoloop/events.ndjson` with an `AutoloopEvent` parser and prove it with
a captured payload, or mark the dashboard non-functional in the README and
delete the watcher plus `EventLogger` and the Node stdout parser. The warning at
`web.rs:412` already discloses the limitation, which is mitigation, not the
acceptance the bead requires.

## Bead 2. `ralph-orchestrator-landing-untracked-sweep-yxv`

Tracker claim: the "land the plane" auto-commit at loop end committed an
untracked operator-local file that merely sat in the worktree, and the commit
should cover only paths the loop touched, or an allowlist, or the ignore rules.

### Commands

```bash
grep -rn 'fn auto_commit_changes' crates/ --include=*.rs
sed -n '130,140p' crates/ralph-core/src/landing.rs
sed -n '144,196p' crates/ralph-core/src/git_ops.rs
grep -n 'ignore\|allowlist\|only\|scope' crates/ralph-core/src/git_ops.rs
git show --stat --format='%h %s' 1e67e52
grep -rn 'Landing\|land' crates/ralph-cli/src/completion_coord.rs
```

### Output excerpt

```text
crates/ralph-core/src/git_ops.rs:168:pub fn auto_commit_changes(
```

```text
crates/ralph-core/src/landing.rs:135:        // Step 2: Auto-commit uncommitted changes
crates/ralph-core/src/landing.rs:137:            match auto_commit_changes(workspace, &loop_id) {
```

```text
137:/// - Untracked files (not in .gitignore)
144:pub fn has_uncommitted_changes(path: impl AsRef<Path>) -> Result<bool, GitOpsError> {
182:            .args(["add", "-A"])
191:    // If nothing was staged after git add -A, return no commit
574:    fn test_auto_commit_only_gitignored_files() {
```

Doc comment above the function:

```text
/// This stages all changes (untracked, staged, unstaged) and creates a commit
/// with a standardized message.
```

Live evidence of the exact failure shape, a loop auto-commit on the rollup
branch:

```text
1e67e52 chore: auto-commit before merge (loop primary)
 .../tui-stream-history-backpressure.code-task.md   | 78 ++++++++++++++++++++++
 1 file changed, 78 insertions(+)
```

### Finding

Verdict: `genuinely-open`. The sweep is real.

- The defect behavior lives in `crates/ralph-core/src/git_ops.rs:182`, where
  `auto_commit_changes` runs `git add -A`. Its own doc comment at `:150-151`
  states it stages untracked, staged, and unstaged changes. Every untracked file
  outside `.gitignore` is in scope.
- `crates/ralph-core/src/landing.rs:137` is the caller that makes it a landing
  behavior. The bead cites `landing.rs`, which is the right call site but not
  the sweep line. Cite `git_ops.rs:182` in the audit.
- No allowlist and no touched-path scoping exists. The only filter is
  `.gitignore`, which git applies implicitly. The one relevant test is
  `test_auto_commit_only_gitignored_files` at `git_ops.rs:574`, which asserts
  that ignored files are not committed. Nothing protects an unrelated file that
  is untracked and not ignored, which is exactly the reported case.
- The failure is not theoretical. Commit `1e67e52` on the rollup branch is a
  loop auto-commit produced by this code path.

Remaining work for Step 6: write the failing regression test first (plant an
untracked, unrelated file, drive `auto_commit_changes`, assert it is not
committed), confirm RED, then narrow the scope to paths the loop touched or an
allowlist plus ignore rules, then confirm GREEN. The plan already requires the
RED then GREEN transition.

## Bead 3. `ralph-orchestrator-tui-help-wave-stale-5hu`

Tracker claim: the help overlay still shows a "Wave Workers: w Enter wave worker
view / h/l Cycle / Esc Exit" section from the deleted wave system, and no live
`w` wave keybinding should remain.

### Commands

```bash
grep -n -i 'wave' crates/ralph-tui/src/widgets/help.rs
grep -rn "Char('w')" crates/ralph-tui/src/
grep -rni 'wave' crates/ralph-tui/src/ --include=*.rs | wc -l
grep -rn 'WaveStarted' crates/ --include=*.rs
```

### Output excerpt

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

```text
crates/ralph-proto/src/json_rpc.rs:349:    WaveStarted {
crates/ralph-tui/src/rpc_source.rs:445:        RpcEvent::WaveStarted {
crates/ralph-core/src/diagnostics/orchestration.rs:117:            OrchestrationEvent::WaveStarted {
```

### Finding

Verdict: `genuinely-open`, and the bead understates the scope.

- The help section is present exactly as reported: header at `help.rs:114`,
  "Enter wave worker view" at `:119`, "Exit wave view" at `:127`. The file is
  226 lines.
- The `w` keybinding is live, not stale text. `input.rs:98` maps
  `KeyCode::Char('w')` to `Action::EnterWaveView`, and `app.rs:131-132` handles
  that action by calling `state.enter_wave_view()`. The view then drives
  `header.rs:140` (`[WAVE]` mode tag), `header.rs:40-46` (worker N/M), and the
  buffer swap in `app.rs:498-500`.
- The surface is 236 wave hits across `ralph-tui/src`, spanning `input.rs`,
  `app.rs`, `state.rs`, `header.rs`, and `rpc_source.rs`. The bead's "minor
  (help text only)" label is wrong. Step 7's demo ("no wave section and no live
  `w` keybinding") is still the right acceptance, but the change touches the
  keybinding, the action, the header mode, and the view state, not just help
  text.
- No live producer of wave events exists. `RpcEvent::WaveStarted` is defined at
  `json_rpc.rs:349` and consumed at `rpc_source.rs:445`, but nothing in the tree
  produces it. The only construction of a wave-start event is a serialization
  fixture inside `crates/ralph-core/src/diagnostics/orchestration.rs:117`. So
  the drill-down view cannot activate from engine data today. Whether that is a
  port or a delete is Step 4's call; Step 7 must follow it.

Remaining work for Step 7: once Step 4 decides the wave surface outcome, either
delete the help section, the `w` keybinding, the `EnterWaveView` action, and the
wave view state if waves are not certified, or update the help text if they are.
The current acceptance wording assumes deletion, and this evidence supports
deletion. The assertion belongs in a widget test or an exercised overlay.

## Summary for the audit

| Bead | Tracker claim | Verdict | Defect location |
| --- | --- | --- | --- |
| ga3-c4-dashboard-dead-svf | readers severed | genuinely-open | `crates/ralph-api/src/event_watcher.rs:40,70,100`; `crates/ralph-core/src/event_logger.rs:111`; `backend/ralph-web-server/src/runner/RalphEventParser.ts:40` |
| landing-untracked-sweep-yxv | untracked sweep | genuinely-open | `crates/ralph-core/src/git_ops.rs:182` called from `crates/ralph-core/src/landing.rs:137` |
| tui-help-wave-stale-5hu | stale help text | genuinely-open | `crates/ralph-tui/src/widgets/help.rs:114`; live keybinding at `crates/ralph-tui/src/input.rs:98` |

Three corrections to carry into the audit. The engine stream is
`.ralph/autoloop/events.ndjson`, not `.ralph/autoloop-events.ndjson`. The landing
sweep line is `git_ops.rs:182`, not a line in `landing.rs`. The TUI wave surface
is broader than help text and includes a live keybinding with no event producer.

No source edits. Handed to the Critic.

## Critic pass

Every cited line was re-read from a fresh shell rather than trusted from this
log. All of them reproduce: `event_watcher.rs:40` and `:100`,
`event_logger.rs:111` and `:265`, `autoloop_engine.rs:393`,
`engine_state.rs:16`, `RalphEventParser.ts:40`, `RalphTaskHandler.ts:83` and
`:93`, `web.rs:412`, `git_ops.rs:182`, `landing.rs:137`, `help.rs:114`,
`input.rs:98`. The `WaveStarted` grep still returns only the diagnostics enum
and fixture plus the `json_rpc.rs:349` definition and the `rpc_source.rs:445`
consumer, so the no-producer finding holds. Verdicts upheld: all three beads are
`genuinely-open`.
