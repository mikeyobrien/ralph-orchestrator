# Manual Live Harness Smoke

> **Internal, manual, and paid.** This preset is not a supported production
> builtin, does not appear in `presets/index.json`, and must never run against
> live providers in CI.

This smoke verifies one real ordinary shell-tool call and response through each
provider, in this fixed order:

| Order | Provider | Backend kind | Live invocation |
|---:|---|---|---|
| 1 | Claude | `claude-sdk` | Claude Agent SDK / `claude` |
| 2 | Codex | `command` | `codex exec --yolo` |
| 3 | OpenCode | `command` | `opencode run` |
| 4 | Pi | `pi` | persistent `pi` RPC |
| 5 | Hermes | `acp` (`hermes`) | `hermes acp` |
| 6 | Kiro | `acp` (`kiro`) | `kiro-cli acp` |

The runner passes only when it finds all six exact, ordered sentinel lines, the
exact response and handoff for every provider, and the literal native-journal
topic `smoke.complete`. A clean Ralph exit or generic `completion_event` stop
reason is not sufficient.

## Prerequisites

Run from a Ralph source checkout with:

- `ralph`, `autoloop`, `git`, and `python3` on `PATH`;
- `claude`, `codex`, `opencode`, `pi`, `hermes`, and `kiro-cli` installed;
- every provider CLI already authenticated with a usable default model/profile.

The script checks executable availability before creating a workspace, but it
does not log in or prove authentication. Verify each CLI's account/profile
separately before starting. A missing or unauthenticated provider is a hard
failure; there are no skips or retries.

## Cost and safety bounds

This command makes live paid model calls. The default limits are:

- exactly six iterations, one per provider;
- 300-second backend timeout per role;
- 2,700-second total timeout;
- USD 5 total cost cap.

Actual billing and cost reporting depend on the provider and authenticated
profile. Inspect the retained native journal/provider output for any cost data
that is available; the cap is a safety bound, not a price estimate.

The fixed probes write only to the run's engine state. The runner creates a new
disposable git repository outside the source checkout, writes an absolute
explicit-preset Ralph config there, and does not modify provider configuration.
It deletes successful workspaces by default and always retains failed ones.
Signals and timeout also retain the workspace. Do not run this from automated
tests or CI; the repository integration suite uses fake providers instead.

## Run

From the repository root:

```bash
tools/smoke-live-harnesses.sh
```

Retain successful evidence for manual inspection or dogfood recording:

```bash
KEEP_SMOKE_DIR=1 tools/smoke-live-harnesses.sh
```

A passing run prints six ordered `PASS` rows. With retained state, inspect:

```text
<workspace>/ralph-output.log
<workspace>/ralph-smoke.yml
<workspace>/.autoloop/journal.jsonl
<workspace>/.autoloop/runs/<run-id>/smoke-evidence.txt
```

The evidence file must be exactly:

```text
HARNESS_SMOKE:claude
HARNESS_SMOKE:codex
HARNESS_SMOKE:opencode
HARNESS_SMOKE:pi
HARNESS_SMOKE:hermes
HARNESS_SMOKE:kiro
```

Before sharing logs, inspect them for credentials, account metadata, prompts,
or other sensitive provider output. Never commit retained workspaces or runtime
`.autoloop/`/`.ralph/` state.

## Supported overrides

| Variable | Default | Purpose |
|---|---|---|
| `RALPH_BIN` | `ralph` | Ralph executable name or path |
| `AUTOLOOP_BIN` | `autoloop` | autoloop executable name or path |
| `KEEP_SMOKE_DIR` | `0` | Set to `1` to retain a successful workspace |
| `SMOKE_MAX_COST_USD` | `5` | Nonnegative total cost cap |
| `SMOKE_TIMEOUT_SECONDS` | `2700` | Positive integer total timeout |

Example with tighter bounds and explicit local binaries:

```bash
RALPH_BIN=target/debug/ralph \
AUTOLOOP_BIN="$HOME/bin/autoloop" \
KEEP_SMOKE_DIR=1 \
SMOKE_MAX_COST_USD=3 \
SMOKE_TIMEOUT_SECONDS=1800 \
tools/smoke-live-harnesses.sh
```

The six-iteration budget, provider order, backend kinds, commands, and provider
profiles are intentionally not overridable. This prevents silent matrix skips
or substitutions.

## Failure diagnosis and reruns

Any missing executable/authentication, timeout, backend error, malformed or
misordered evidence, missing exact response, missing handoff, or absent literal
`smoke.complete` produces a nonzero status. Failure output identifies the
backend/kind, process status, run ID, retained workspace, native journal and
stop detail, last relevant output, and an exact rerun command.

1. Read `ralph-output.log` and the reported native-journal records for the
   failing backend.
2. Correct the executable, authentication/profile, service, or harness issue.
3. Run the exact reported rerun command. It enables `KEEP_SMOKE_DIR=1` and
   preserves the same configured timeout and cost cap, but creates a fresh
   disposable workspace and performs one new paid attempt.
4. Compare the new six rows, journal handoffs, and evidence file. Do not treat a
   provider process exit of zero as a pass unless the runner itself exits zero.

There are no hidden retries and no resume path. Re-running intentionally incurs
another bounded set of live calls.
