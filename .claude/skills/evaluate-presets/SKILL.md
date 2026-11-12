---
name: evaluate-presets
description: "Evaluate and audit Ralph's built-in presets: run the evaluation scripts, check hat routing, review a preset's design. Use after changing a preset, hat routing, or the core prompt."
metadata:
  internal: true
---

# Evaluate presets

Two jobs: run the evaluation scripts against the builtin presets, and audit a preset's design. For launching, monitoring, and measuring a single loop, or comparing two presets on one task, follow `skills/ralph-loop/references/headless-runs.md`.

## Run the evaluation scripts

```bash
./tools/evaluate-preset.sh <preset> [claude|kiro]   # one preset (default backend: claude)
./tools/evaluate-all-presets.sh claude              # every preset in its PRESETS list
```

- The presets evaluated, and each one's test task, expected iterations, and timeout, live in `tools/evaluate-all-presets.sh` (`PRESETS=`) and `tools/preset-test-tasks.yml`. Read them there rather than from this file.
- Runs take minutes to hours, so start them as background jobs and wait for the exit notification.
- Each run writes `.eval/logs/<preset>/<timestamp>/` (`output.log`, `session.jsonl`, `metrics.json`, `environment.json`, `merged-config.yml`). The suite writes `.eval/results/<suite-id>/SUMMARY.md` plus one JSON file per preset.
- Exit codes: `0` means the completion promise was reached. `124` means it timed out. Anything else is a failure; read `output.log`.

**Done when** every preset in the run has an exit code and a `metrics.json`, and each failure or timeout has a cause written next to it.

## Check hat routing

Each hat should run in its own iteration with fresh context: one hat, one published event, then stop.

| Pattern in `session.jsonl` / `output.log` | Meaning |
|---|---|
| iterations ≈ events published | Routing works |
| iterations well below events | Several hats' work done in one iteration |
| iterations well above events | Hats ending without publishing; `default_publishes` or recovery is firing |
| 0 events | Events are not being read from the JSONL file |

```bash
grep -c "ITERATION" .eval/logs/<preset>/latest/output.log
grep -c "bus.publish" .eval/logs/<preset>/latest/session.jsonl
jq -r '.ts' .eval/logs/<preset>/latest/session.jsonl   # identical stamps = same-iteration publishes
```

When routing is broken, look first at the prompt text in `crates/ralph-core/src/hatless_ralph.rs`: the Event Publishing Guide and the EVENT WRITING section tell a hat to publish one event and stop. Then check that the active hat's instructions and the `## PENDING EVENTS` section reach the prompt.

## Audit a preset's design

Apply every check in `skills/ralph-hats/references/design.md` to every hat in the preset. Built-in presets add two of their own:
- **Tests assert protocol, not wording.** `crates/ralph-cli/src/presets.rs` checks events, triggers, paths, task keys, and commands, so a prompt rewrite doesn't break the tests.
- **Both copies stay in sync.** Edit `presets/`, then run `./scripts/sync-embedded-files.sh`, which mirrors to `crates/ralph-cli/presets/` for `include_str!`.

**Done when** every hat has passed every check, or each miss is written down with its reason.

## Fix what the evaluation finds

For each failure, create a code task (`/code-task-generator`, output to `.ralph/tasks/preset-fixes/`), implement it (`/code-assist`), then re-run `./tools/evaluate-preset.sh <preset>`.

## Prerequisites

- Cargo, to build Ralph.
- `yq` (optional), to load test tasks from YAML.
