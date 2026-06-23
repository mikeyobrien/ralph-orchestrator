export const meta = {
  name: 'verify-tui-ux',
  description: 'Verify all ralph-tui UX surfaces via tmux to catch regressions from recent changes',
  whenToUse: 'After pushing TUI-touching changes; drives the TUI in isolated tmux sessions, judges each surface, and adversarially re-checks failures.',
  phases: [
    { title: 'Gate', detail: 'Build the guidance_test example binary and run cargo test -p ralph-tui (snapshot + unit gate)' },
    { title: 'Drive', detail: 'One agent per UX surface: launch the TUI in an isolated tmux session, send keys, capture panes, self-judge' },
    { title: 'Verify', detail: 'Independently re-run any FAILED surface to confirm a real regression vs. flake' },
    { title: 'Synthesize', detail: 'Compile a single regression report' },
  ],
}

const REPO = '/Users/rook/projects/ralph-orchestrator'
const BIN = `${REPO}/target/debug/examples/guidance_test`

// Shared playbook every live-scenario agent must follow. Keeps tmux sessions,
// HOME, and export workspaces fully isolated so agents can run concurrently.
const PLAYBOOK = `
You are verifying ONE UX surface of the ralph-tui terminal UI by driving the real
TUI in an isolated tmux session and judging captured screen output.

## The TUI under test
The prebuilt example binary is at:
  ${BIN}
It seeds DETERMINISTIC mock data (no live backend, no API):
  - iteration 1: hat "Builder", 30 lines like "[iter 1] Line N: doing some work..."
  - iteration 2: hat "Reviewer", 20 lines like "[iter 2] Line N: reviewing changes..."
  - max_iterations = 10, view starts on iteration 2 (latest, following_latest=true)
The TUI blocks until you press 'q' or Ctrl-C.

## Isolation setup (run these FIRST, in bash)
Pick a UNIQUE session name S and unique temp dirs using your scenario key + PID:
  S="tuiverify-<SCENARIO_KEY>-$$"
  WS="$(mktemp -d)"            # export workspace root (isolates .ralph/tui-exports)
  HM="$(mktemp -d)"            # fake HOME (isolates the example's events.jsonl)
Launch in a fixed-size pane (120x40) so layout is stable:
  tmux new-session -d -s "$S" -x 120 -y 40
  tmux send-keys -t "$S" "cd ${REPO} && HOME=$HM RALPH_WORKSPACE_ROOT=$WS ${BIN}" Enter
  sleep 2     # binary is prebuilt, startup is fast

## Driving and capturing
- Send keys with: tmux send-keys -t "$S" "<keys>"   (NO Enter unless the surface needs it)
  Special keys by name: Escape, Enter, Up, Down, Left, Right, BSpace
  Letters/symbols literally: tmux send-keys -t "$S" "e"   /   "E"   /   ":"   /   "/"
- After EACH key action: sleep 0.5, then capture with:
  tmux capture-pane -t "$S" -p
- Capture multiple times across your step sequence. Keep the raw captures as evidence.

## Cleanup (ALWAYS, even on failure)
  tmux send-keys -t "$S" "q" ; sleep 0.3
  tmux kill-session -t "$S" 2>/dev/null
  rm -rf "$WS" "$HM"

## Judging
Judge SEMANTICALLY — be lenient on whitespace/exact spacing, strict on:
  - required content present / absent at the right time
  - correct state transitions when keys are pressed
  - no garbled output, no panic/stack trace in the pane, no broken layout
If the pane shows a Rust panic, "thread 'main' panicked", or the binary never
rendered (blank/empty content area after launch), that is a FAIL/BLOCKED.

Return your structured verdict. Put the most decisive raw pane captures in 'captures'.
`

const VERDICT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['scenario', 'status', 'summary', 'checks', 'captures'],
  properties: {
    scenario: { type: 'string' },
    status: { type: 'string', enum: ['PASS', 'FAIL', 'BLOCKED'] },
    summary: { type: 'string', description: 'One or two sentences on the outcome' },
    checks: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['name', 'status', 'evidence'],
        properties: {
          name: { type: 'string' },
          status: { type: 'string', enum: ['pass', 'fail'] },
          evidence: { type: 'string' },
        },
      },
    },
    captures: { type: 'string', description: 'Key raw tmux capture-pane snippets used as evidence' },
  },
}

const GATE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['build_ok', 'tests_ok', 'build_summary', 'tests_summary'],
  properties: {
    build_ok: { type: 'boolean' },
    tests_ok: { type: 'boolean' },
    build_summary: { type: 'string' },
    tests_summary: { type: 'string', description: 'Pass/fail counts and any failing test names' },
  },
}

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['scenario', 'confirmed_regression', 'explanation'],
  properties: {
    scenario: { type: 'string' },
    confirmed_regression: { type: 'boolean', description: 'true if an independent re-run reproduces the failure' },
    explanation: { type: 'string' },
  },
}

// ---- UX surfaces to verify. One agent each, runnable concurrently. ----
const SCENARIOS = [
  {
    key: 'baseline',
    title: 'Startup + header/footer baseline',
    steps: `Launch and do NOT press any navigation keys yet. Capture the initial screen.
Verify:
- Header shows an iteration counter (e.g. "[iter 2/10]" or "iter 2/10"), an elapsed time (MM:SS),
  the current hat ("Reviewer" or "Builder"), and a mode indicator (auto/paused).
- Footer shows an activity indicator (active/idle/done) AND the NEW export hint "e export E all"
  (this hint was just added today — its absence is a regression).
- Content area shows the iteration-2 "reviewing changes..." lines.`,
  },
  {
    key: 'navigation',
    title: 'Iteration navigation (h/l/arrows)',
    steps: `From the start (iter 2), press Left (or "h") to go to iteration 1, capture, then Right (or "l")
to return to iteration 2, capture.
Verify:
- Going left changes the header counter to iteration 1 and the hat to "Builder", and content shows
  "[iter 1] ... doing some work..." lines.
- Going right returns to iteration 2 / "Reviewer" / "reviewing changes..." lines.
- Navigating left at iteration 1 (press Left again) does NOT crash or go below 1 (bounds hold).`,
  },
  {
    key: 'scrolling',
    title: 'Content scrolling (j/k/g/G)',
    steps: `On iteration 1 (press Left first to get its 30 lines), press "j" / Down several times to scroll
down, capture; press "G" to jump to bottom, capture; press "g" to jump to top, capture.
Verify:
- The visible content lines change as you scroll (different "Line N" values appear).
- "G" shows the last lines, "g" shows the first lines.
- No garbled/overlapping text while scrolling.`,
  },
  {
    key: 'search',
    title: 'Search mode (/ n N Esc)',
    steps: `Press "/" to enter search, type the word "reviewing", press Enter, capture. Then press "n"
(next match) and "N" (prev match), capturing each. Finally press Escape to exit search, capture.
Verify:
- Footer enters a search display showing the query and a match counter like "N/M".
- "n"/"N" move between matches without crashing.
- Escape cleanly exits search mode (search display gone, normal footer restored).`,
  },
  {
    key: 'help',
    title: 'Help overlay (?) + dismiss',
    steps: `Press "?" (Shift+/) to open the help overlay, capture. Then press Escape to dismiss, capture.
Verify:
- A help modal/overlay appears listing keybindings.
- It includes an EXPORT section or the "e"/"E" export keys (added today) — missing export help is a regression.
- It lists core keys: navigation (h/l), scroll (j/k/g/G), search (/), guidance (: and !), quit (q).
- Escape (or any key) dismisses it and returns to the normal view.`,
  },
  {
    key: 'guidance',
    title: 'Guidance input (: and !)',
    steps: `Press ":" to open next-boundary guidance input, type "do the thing", press Enter, capture.
Then press "!" to open urgent steer input, type "urgent fix", press Enter, capture.
Verify:
- After ":" the footer shows a guidance input line (e.g. "guidance (next)") while typing.
- After Enter a green flash confirms it was queued (e.g. "✓ guidance queued (next)").
- After "!" the footer shows an urgent input (e.g. "guidance (now!)") and on Enter a flash like
  "✓ guidance sent (now!)".
- Escape cancels an open input without submitting (test by opening ":" then pressing Escape).`,
  },
  {
    key: 'export',
    title: 'Export iteration buffers (e / E) — NEW today, highest priority',
    steps: `This is the freshly-shipped feature; verify it end to end.
1. Press "e" (export current iteration). sleep 1, capture.
   - Verify the footer shows a green flash like "✓ exported current iteration: .ralph/tui-exports/ralph-tui-current-<TIMESTAMP>.txt".
2. Press "E" (Shift+E, export all iterations). sleep 1, capture.
   - Verify a green flash like "✓ exported all iterations: .ralph/tui-exports/ralph-tui-all-<TIMESTAMP>.txt".
3. Verify the files were ACTUALLY written to disk (this is the real regression test). In bash:
     ls -la "$WS/.ralph/tui-exports/"
     cat "$WS"/.ralph/tui-exports/ralph-tui-current-*.txt | head -40
     cat "$WS"/.ralph/tui-exports/ralph-tui-all-*.txt | head -60
   - The "current" file must contain iteration-2 "reviewing changes..." content.
   - The "all" file must contain BOTH iteration 1 ("doing some work...") and iteration 2 content.
   - A red flash ("✗ export ... failed") or missing/empty files is a FAIL.
Put the ls output and file head excerpts in 'captures'.`,
  },
  {
    key: 'mouse-quit',
    title: 'Mouse toggle (m) + clean quit (q)',
    steps: `Press "m" to toggle mouse mode, capture. Press "m" again to toggle back, capture.
Then press "q" to quit and capture the pane after ~1s.
Verify:
- Toggling "m" shows/hides a mouse-mode hint in the footer (e.g. "Mouse: scroll (m)").
- "q" exits the TUI cleanly: the pane returns to a shell prompt and shows the example's
  "=== Guidance Test Results ===" / "=== Done ===" epilogue (printed to stderr on exit),
  with NO panic or stack trace.`,
  },
]

// ---------------- Phase 0: build + unit/snapshot gate ----------------
phase('Gate')
const gate = await agent(
  `Working in ${REPO}.
1. Build the TUI test-launcher example binary:
     cd ${REPO} && cargo build -p ralph-tui --example guidance_test 2>&1 | tail -30
   Confirm the binary exists at ${BIN}.
2. Run the deterministic TUI test gate (snapshot + unit tests, which today's export commit updated):
     cd ${REPO} && cargo test -p ralph-tui 2>&1 | tail -40
   Report the pass/fail counts and name any failing tests.
Return build_ok (does the binary exist & compile), tests_ok, and concise summaries.`,
  { label: 'gate:build+test', phase: 'Gate', schema: GATE_SCHEMA },
)

if (!gate || !gate.build_ok) {
  log('Build gate FAILED — the example binary did not compile; skipping live tmux scenarios.')
  return {
    gate,
    aborted: true,
    reason: 'guidance_test example failed to build; cannot drive the live TUI.',
  }
}
log(`Gate: build_ok=${gate.build_ok} tests_ok=${gate.tests_ok}. Driving ${SCENARIOS.length} UX surfaces in tmux.`)

// ---------------- Phases 1+2: drive each surface, then adversarially verify failures ----------------
const results = await pipeline(
  SCENARIOS,
  // Stage 1 — drive + self-judge the surface live in tmux
  (sc) =>
    agent(
      `${PLAYBOOK}\n\n## YOUR SURFACE: ${sc.title} (scenario key: "${sc.key}")\n\n${sc.steps}\n\nUse scenario key "${sc.key}" in the tmux session name. Set scenario="${sc.title}" in your verdict.`,
      { label: `drive:${sc.key}`, phase: 'Drive', schema: VERDICT_SCHEMA },
    ),
  // Stage 2 — only failures get an independent reproduction
  (verdict, sc) => {
    if (!verdict) {
      return { scenario: sc.title, status: 'BLOCKED', summary: 'agent returned no verdict', checks: [], captures: '', verify: null }
    }
    if (verdict.status === 'PASS') return { ...verdict, verify: null }
    return agent(
      `${PLAYBOOK}\n\n## INDEPENDENT RE-CHECK of a reported failure on surface: ${sc.title} (key "${sc.key}")\n\n` +
        `Another agent reported status="${verdict.status}". Reproduce from scratch in a fresh isolated tmux session and decide whether this is a REAL regression or a flake/timing artifact / test-harness mistake.\n\n` +
        `Their summary: ${verdict.summary}\nTheir steps to follow:\n${sc.steps}\n\n` +
        `Be skeptical of timing: if the only issue was capturing before a flash/render settled, re-run with a longer sleep before deciding. Set confirmed_regression=true ONLY if the failure genuinely reproduces.`,
      { label: `verify:${sc.key}`, phase: 'Verify', schema: VERIFY_SCHEMA },
    ).then((v) => ({ ...verdict, verify: v }))
  },
)

const clean = results.filter(Boolean)
const passed = clean.filter((r) => r.status === 'PASS')
const confirmedRegressions = clean.filter(
  (r) => r.status !== 'PASS' && (r.verify ? r.verify.confirmed_regression : true),
)
const flakes = clean.filter((r) => r.status !== 'PASS' && r.verify && !r.verify.confirmed_regression)

// ---------------- Phase 3: synthesize a single report ----------------
phase('Synthesize')
const report = await agent(
  `Write a concise terminal-friendly Markdown regression report for a ralph-tui UX verification run.\n\n` +
    `Build/unit gate: ${JSON.stringify(gate)}\n\n` +
    `Per-surface results (with adversarial verify where a failure was reported): ${JSON.stringify(clean)}\n\n` +
    `Structure:\n` +
    `1. One-line verdict: REGRESSIONS FOUND or NO REGRESSIONS.\n` +
    `2. Gate result (build + cargo test).\n` +
    `3. A table: Surface | Status | Note.\n` +
    `4. For each CONFIRMED regression: what broke, the evidence/capture, and the likely culprit commit ` +
    `(today's TUI changes were 3454c62 export feature [e/E keys, export.rs, footer hint] and snapshot updates).\n` +
    `5. Note any failures that the re-check downgraded to flakes.\n` +
    `Be precise and do not invent passing surfaces. Return ONLY the Markdown.`,
  { label: 'synthesize:report', phase: 'Synthesize' },
)

return {
  verdict: confirmedRegressions.length === 0 && gate.build_ok && gate.tests_ok ? 'NO REGRESSIONS' : 'REGRESSIONS FOUND',
  gate,
  passed: passed.map((r) => r.scenario),
  confirmedRegressions: confirmedRegressions.map((r) => ({ scenario: r.scenario, summary: r.summary })),
  flakes: flakes.map((r) => r.scenario),
  report,
}
