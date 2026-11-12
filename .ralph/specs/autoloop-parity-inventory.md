---
status: draft
created: 2026-11-12
related:
  - .ralph/specs/script-hats-and-event-gates.spec.md
  - https://github.com/mikeyobrien/autoloop
---

# autoloop → ralph parity inventory

autoloop began as a spin-off experiment from ralph-orchestrator and is being folded back in. This inventory lists every autoloop capability, gives its ralph equivalent, and ranks what to bring back. Sources: the autoloop `main` branch (`packages/{core,harness,cli,backends,dashboard,kanban,gh-sync,linear-sync,issue-sync-core,presets}`) and the ralph `main` branch at #415. Status is one of parity, partial, missing, or ralph-ahead. Each "missing" was checked by searching ralph for several names.

## Headline finding

Ralph already loads autoloop directory presets. `TomlPresetSource` (`crates/ralph-core/src/preset_source.rs`) and `resolve_preset_dir` (`crates/ralph-cli/src/main.rs`) find them under `./presets`, `~/.config/{ralph,autoloop}/presets`, and `$RALPH_PRESETS_DIR`. The importer maps roles, emits, handoff, completion, `max_iterations`, `required_events`, and `harness.md`, and **silently drops the rest** of `autoloops.toml`: `backend.*`, per-role `backend_model`, `hooks.pre_run`, `review.*`, `memory.*`, `core.state_dir`, and the `{{STATE_DIR}}` / `{{TOOL_PATH}}` / `{{PRESET_DIR}}` placeholders. `presets/README.md` says autoloop presets "work unchanged". That is overstated for any preset that bundles scripts or pins models, such as autowiki. Much of the pullback is closing that importer gap, not building a new preset format.

## Priority 1: needed to run autoloop presets faithfully

| Capability | Ralph today | Status | Plan |
|---|---|---|---|
| Event gates (`[[gate]]`, `core/src/evidence.ts`) and script-only roles | hard-coded `build.done`/`review.done` string checks | missing | **Spec:** `script-hats-and-event-gates.spec.md` |
| Template placeholders `{{STATE_DIR}}`, `{{TOOL_PATH}}`, `{{PRESET_DIR}}` in prompts and hook commands (`harness/src/config-helpers.ts`) | no expansion | missing | Importer: expand placeholders, reject raw `.autoloop` paths |
| Per-role `backend_model` (Opus/Sonnet tiering) | `HatConfig.backend` exists, but the importer ignores the field | partial | Importer: map to hat `backend` args |
| Preset-level `backend.*`, including `disallowed_tools` | `cli.backend`, hat `disallowed_tools`; dropped on import | partial | Importer: map to the config overlay |
| Preset `hooks.pre_run` / `hooks.strict` running a bundled bootstrap script | `pre.loop.start` hooks exist; not mapped, no `PRESET_DIR` | partial | Importer: map to `pre.loop.start` with `on_error: block` |
| Preset bundles shipping `scripts/`, `skills/`, `hooks/` | `skills.dirs`; no preset-relative assets | partial | Resolve bundle paths relative to the preset dir |

## Priority 2: high-value capabilities ralph lacks

| Capability (autoloop file) | Ralph today | Status | Why it matters |
|---|---|---|---|
| Metareview (`harness/src/metareview.ts`; `review.enabled`, `every_iterations`, CONTINUE/REDIRECT/TAKEOVER/EXIT with confidence; fails closed) | none | missing | The supervision layer the autoloop presets rely on. TAKEOVER semantics need reading before a spec is written. |
| Backend circuit breaker and error classes (`harness/src/circuit-breaker.ts`, `backends/src/error-class.ts`) | `consecutive_failures` only | missing | Auth and quota failures should stop cleanly, and rate limits should back off, instead of burning failure counts |
| Provisional completion (`harness/src/provisional.ts`): a done-claim waits for gates or operator acceptance; recovered after a crash | completion terminates directly; auto-merge acts at once | missing | Stops landing and auto-merge from acting on a self-reported "done". Builds on gates. |
| Test-tamper screen (`harness/src/tamper.ts`) | none | missing | Blocks done-claims when the run edited gating tests or added skip/only. Cheap anti-reward-hacking. |
| `--architect`: generate a bespoke preset for the objective, then run it (`autoarchitect.toml`, `cli/src/commands/run.ts`) | none | missing | autoloop's headline feature. Should emit ralph YAML. |
| Preset chains (`cli/src/chains/*`; `chains.toml`, `--chain a,b`, budgets) | none (monolithic `pdd-to-code-assist`) | missing | Composes existing presets (autospec → code-assist → autoqa) without mega-collections |

## Priority 3: medium value

| Capability | Ralph today | Status |
|---|---|---|
| Postconditions: no TODOs, no secrets, no skipped tests, clean tree (`harness/src/postconditions.ts`) | none | missing. Can be expressed as gate `command`s once gates land |
| Acceptance criteria written as `text :: cmd` and run as checks (`harness/src/intent.ts`) | `preflight.rs` checks that Given/When/Then exists; never runs it | partial |
| Frozen paths (`event_loop.frozen_paths`) | none | missing |
| Post-run verify: `confirmed` / `false_done` / `unverifiable` (`cli/src/commands/verify.ts`) | none | missing |
| Completion-contract lint: warn when completion cannot be falsified (`harness/src/completion-lint.ts`) | preflight checks only | missing |
| Durable waits with timed auto-resume (`harness/src/wait.ts`, `auto-resume.ts`) | hook-driven suspend only | partial |
| `resume --add-iterations N -b <backend>` | `loops resume`, `run --continue` | partial |
| Live control per backend: mid-turn steer (pi RPC, claude-sdk), addressing detached runs by id (`harness/src/control/*`) | RPC steer, `UrgentSteerStore`, Telegram | partial |
| Two-tier memory (run plus project) with compact, prune, promote (`core/src/memory.ts`) | single store, budget truncation | partial |
| Fan-out stages with deterministic reducers: quorum, vote, dedup (`core/src/fanout.ts`) | waves plus an LLM aggregator hat | partial |
| Wave aggregate `first_success` / `timeout` modes | `WaitForAll` only | partial |
| Profiles: per-role prompt fragments by repo or user (`core/src/profiles.ts`) | `extra_instructions` only | partial |
| `--set key=value` for any config key | `-c core.x=y` for two keys | partial |
| Configurable state, journal, and memory paths, for parent orchestrators | `.ralph/` fixed | partial |
| Generic ACP providers (`acp:<provider>:<cmd>`, claude-agent-acp, hermes) | `kiro-acp` only | partial |
| Command-risk hard-deny floor (`backends/src/command-risk.ts`) | none | missing |
| Issue sync for GitHub and Linear on a shared core (`gh-sync`, `linear-sync`, `issue-sync-core`) | none | missing |
| Kanban: interactive tmux/PTY terminal per task in the browser | web `task.run` with log viewer | partial |
| Runs health: stuck / waiting / recently-failed (`core/src/runs-health.ts`) | liveness only | partial |
| Bundled TOML presets shipped in the binary | YAML builtins only | partial |
| `preset promote` for generated presets | none | missing |
| ralph as an ACP agent server (`cli/src/acp/*`) | `ralph mcp`, `run --rpc` | partial |

## Low value, or ralph is already ahead

- **Ralph ahead:** worktrees and the merge queue, lifecycle hooks with suspend, HITL (Telegram, web robot), the web dashboard, task model, TUI, backend coverage.
- **Parity:** waves, per-role backend map, budgets and timeouts, role-scope audit, run ids, scratchpad, the shell command backend, emit/memory/task verbs, doctor and hooks commands, the directory preset core shape, preset listing.
- **Low:** stall guard on identical output, regex handoff keys, iteration diffs and metrics, `stats`, `config` CLI verbs, `--budget` flag, `inspect`, `triage`/`capabilities`, herdr sink, the `.miniloop` alias, env hardening, a versioned journal, soft tasks, JEV routing, a run-wide concurrency semaphore, `completion_must_be_last`, contained-file reads, the coordination ledger.

## Suggested sequence

1. **Event gates and script hats** (spec written; being built).
2. **Importer fidelity**: placeholders, `backend_model`, `backend.*`, `pre_run`, bundle-relative paths, then bundling the TOML presets. After this, autowiki and the others actually run unchanged.
3. **Safety and supervision**: the circuit breaker, provisional completion plus the tamper screen (on top of gates), then metareview.
4. **Composition**: chains, then `--architect` emitting ralph YAML, then `preset promote`.
5. **Integrations**: issue sync (GitHub and Linear share a core), generic ACP providers, the kanban terminal.

## Open items

- Metareview TAKEOVER behavior (`harness/src/iteration.ts`) has not been read in full.
- Whether any ralph adapter injects RPC `Steer` into a live backend turn. If one does, live control is closer to parity than listed.
- Whether ralph's hat `disallowed_tools` is a weaker block than autoloop's SDK-level tool removal.
