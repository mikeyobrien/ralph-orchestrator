# Step 1 evidence: in-house remnant, upstream closure, version drift

Task: `task-1790091216-4015`
Key: `code-assist:v3-complete:step-01:verify-remnant-and-drift`
Branch: `v3/complete` at `df2449a` (merge of `22fc1fd` and `351b9f6`)
Date: 2026-09-22
Source edits: none.

Every claim below carries the command and an output excerpt.

## Claim 1. Remnant size and production callers

### Command

```bash
wc -l crates/ralph-core/src/hat_registry.rs
grep -rn 'HatRegistry' crates/ --include=*.rs
grep -rn 'find_by_trigger\|has_subscriber\|can_publish\|get_for_topic\|\.subscribers(' \
  crates/ --include=*.rs | grep -v 'hat_registry.rs'
grep -n '#\[cfg(test)\]' crates/ralph-cli/src/hats.rs
```

### Output excerpt

```text
468 crates/ralph-core/src/hat_registry.rs
```

```text
crates/ralph-core/src/lib.rs:26:mod hat_registry;
crates/ralph-core/src/lib.rs:84:pub use hat_registry::HatRegistry;
crates/ralph-cli/src/hats.rs:17:use ralph_core::{HatRegistry, RalphConfig, truncate_with_ellipsis};
crates/ralph-cli/src/hats.rs:132:    let registry = HatRegistry::from_config(&config);
crates/ralph-cli/src/hats.rs:428:fn list_hats_json<W: Write>(writer: &mut W, registry: &HatRegistry) -> Result<()> {
...
crates/ralph-cli/src/hats.rs:1150:        let registry = HatRegistry::new();   # first test-module hit
```

```text
crates/ralph-cli/src/hats.rs:500:        if registry.has_subscriber(start) {
crates/ralph-cli/src/hats.rs:501:            let hat = registry.get_for_topic(start).unwrap();
crates/ralph-cli/src/hats.rs:529:            if !registry.has_subscriber(topic) {
```

```text
1127:#[cfg(test)]
```

### Finding

The module is 468 lines, not 21K lines, and it is not dead. It has exactly one
production consumer: `crates/ralph-cli/src/hats.rs`, the `ralph hats` operator
command. That command calls `HatRegistry::from_config` at `hats.rs:132` and the
routing methods `has_subscriber` and `get_for_topic` at `hats.rs:500`, `:501`,
`:529`. The `#[cfg(test)]` module starts at `hats.rs:1127`, so every hit below
that line is test-only. No other crate references `HatRegistry`.

Consequence for a7e.10: the bead is `genuinely-open`, but the work is not a bare
deletion. Either the type moves to its real owner (the `ralph hats` surface, or
a config-derived index in `ralph-core`), or `ralph hats` stops depending on it.
Deleting the module alone breaks the `ralph hats` build.

## Claim 2. Absent modules, and one that is not absent

### Command

```bash
find crates \( -name 'event_loop*' -o -name 'hatless_ralph*' -o -name 'event_bus*' -o -name 'wave_*' \) -print
find . -path ./target -prune -o -path ./.git -prune -o \
  \( -name 'wave_*' -o -name 'event_loop*' -o -name 'hatless_ralph*' \) -print
wc -l crates/ralph-proto/src/event_bus.rs
grep -rn 'EventBus' crates/ --include=*.rs
```

### Output excerpt

```text
crates/ralph-proto/src/event_bus.rs
```

The tree-wide find for `wave_*`, `event_loop*`, and `hatless_ralph*` printed
nothing. Exit 0.

```text
401 crates/ralph-proto/src/event_bus.rs
```

```text
crates/ralph-proto/src/lib.rs:15:mod event_bus;
crates/ralph-proto/src/lib.rs:25:pub use event_bus::EventBus;
crates/ralph-core/src/session_recorder.rs:3://! ... captures events from both the EventBus (routing events)
crates/ralph-tui/src/rpc_source.rs:8://! This replaces the in-process `EventBus` observer when running in subprocess mode.
```

### Finding

`event_loop/`, `hatless_ralph.rs`, and `wave_*` are absent from the tree. That
part of the brief holds.

The brief's claim that `event_bus` is "gone from the tree" is **false**.
`crates/ralph-proto/src/event_bus.rs` is present at 401 lines, declared as
`mod event_bus;` at `ralph-proto/src/lib.rs:15`, and re-exported as public API at
`ralph-proto/src/lib.rs:25`. Every remaining `EventBus` mention outside the
module is a doc comment. There is no production caller.

The engine-flip log was scoped to `ralph-core/src`, which is why it did not see
this. Read literally its sentence is correct: no `event_bus` module remains in
`ralph-core/src`. Read against the brief's wording ("gone from the tree") it is
wrong.

Consequence: the in-house engine leaves **two** remnants, not one.
`hat_registry.rs` is 468 lines with one live caller. `event_bus.rs` is 401 lines
with zero live callers, still exported. a7e.10 must cover both. The dead
`event_bus` module plus its re-export is the cheaper half.

## Claim 3. Upstream blocker closure and engine provisioning

### Command

```bash
npm view @mobrienv/autoloop repository.url version
for n in 34 35 37 38 39; do gh api "repos/mikeyobrien/autoloop/issues/$n" --jq ...; done
for n in 40 41 42; do gh api "repos/mikeyobrien/autoloop/pulls/$n" --jq ...; done
autoloop --version
```

### Output excerpt

```text
repository.url = 'git+https://github.com/mikeyobrien/autoloop.git'
version = '0.11.0'
```

```text
#34 closed [v3] `command` backend parity: cost telemetry + live control | https://github.com/mikeyobrien/autoloop/issues/34
#35 closed [v3] Declarative wave/parallel config: per-role concurrency + concurrent waves | https://github.com/mikeyobrien/autoloop/issues/35
#37 closed [v3] Versioned `stopReason` termination contract | https://github.com/mikeyobrien/autoloop/issues/37
#38 closed [v3] Lifecycle hooks engine: phase hooks, suspend/resume, I/O mutation | https://github.com/mikeyobrien/autoloop/issues/38
#39 closed [v3] Policy parity: completion-must-be-last option + emit-boundary file-mod audit | https://github.com/mikeyobrien/autoloop/issues/39
```

```text
#40 closed merged=true feat(observability): structured --events stream + versioned journal contract (#30, #31) | https://github.com/mikeyobrien/autoloop/pull/40
#41 closed merged=true feat(emit): completion-gate store override (#36) + opt-in evidence gates (#33) | https://github.com/mikeyobrien/autoloop/pull/41
#42 closed merged=true feat(hitl): blocking human-ask + respond control verb (#32) | https://github.com/mikeyobrien/autoloop/pull/42
```

```text
0.11.0
```

### Finding

Every upstream claim holds. Issues #34, #35, #37, #38, #39 are closed. PRs #40,
#41, #42 are merged. Published `@mobrienv/autoloop` is 0.11.0.

The brief's "No autoloop engine is installed on this machine" claim is stale.
`autoloop --version` reports `0.11.0` from
`/home/mobrienv/.npm-global/bin/autoloop`. The handoff section of the brief
already records this, so the two sections disagree and the handoff is the
current one.

PR #41 is the load-bearing one for Phase 2c.2. It merged the completion-gate
store override (#36) and the opt-in evidence gates (#33). Both are named as
candidate seams for the Jev-backed completion judge. PR #42 merged the blocking
human-ask and `respond` verb, which is the RObot contract. PR #40 merged the
structured `--events` stream and the versioned journal contract.

## Claim 4. Version drift between the authored target and 0.11.0

### Command

```bash
sed -n '140,230p' crates/ralph-cli/src/autoloop_preset_gen.rs
grep -rn '0\.1[01]\.' crates/ --include=*.rs | grep -i 'autoloop\|0\.10\|0\.11'
autoloop --version
```

### Output excerpt

```text
crates/ralph-cli/src/autoloop_preset_gen.rs:158:/// Map Ralph's normalized loop budgets to autoloop 0.10.x config overrides.
crates/ralph-cli/src/autoloop_preset_gen.rs:212:/// Translate Ralph's resolved CLI backend into autoloop 0.10.x config keys.
```

```text
crates/ralph-core/src/autoloop_health.rs:12:pub const MIN_AUTOLOOP_VERSION: &str = "0.10.0";
crates/ralph-core/src/autoloop_health.rs:15:pub const VENDORED_AUTOLOOP_VERSION: &str = "0.10.1";
crates/ralph-cli/src/doctor.rs:695:                "Autoloop 0.10.0 available (/opt/autoloop/bin/autoloop)",
crates/ralph-cli/src/doctor.rs:710:                "Ralph requires >= 0.10.0. Update it with: npm install -g @mobrienv/autoloop",
```

The two `doctor.rs` hits are fixtures, not operator-facing text. They sit inside
`#[cfg(test)] mod tests` at `doctor.rs:665`, in
`doctor_keeps_autoloop_check_for_explicit_display`. The production messages
interpolate the constants instead:

```bash
grep -n '#\[cfg(test)\]' crates/ralph-cli/src/doctor.rs | head -1
grep -rn 'MIN_AUTOLOOP_VERSION\|VENDORED_AUTOLOOP_VERSION' \
  crates/ralph-core/src/preflight.rs crates/ralph-cli/src/main.rs \
  crates/ralph-cli/src/engine_install.rs crates/ralph-cli/src/engine_provision.rs
grep -n '#\[cfg(test)\]' crates/ralph-cli/src/engine_provision.rs
```

```text
crates/ralph-cli/src/doctor.rs:665:#[cfg(test)]
crates/ralph-core/src/preflight.rs:450:                "The configured autoloop version is {version}; Ralph requires >= {MIN_AUTOLOOP_VERSION}. Update it with: {AUTOLOOP_INSTALL_HINT}"
crates/ralph-cli/src/main.rs:1325:            "autoloop was not found in Ralph's engine directory or on PATH. Ralph requires @mobrienv/autoloop >= {MIN_AUTOLOOP_VERSION}. Install it with: {AUTOLOOP_INSTALL_HINT}; or run: ralph doctor --install-engine (no Node required). For non-interactive first-run provisioning, set RALPH_AUTO_INSTALL_ENGINE=1"
crates/ralph-cli/src/main.rs:1328:            "The configured autoloop version is {version}, but Ralph requires >= {MIN_AUTOLOOP_VERSION}. Update it with: {AUTOLOOP_INSTALL_HINT}; run: ralph doctor --install-engine; or set RALPH_AUTO_INSTALL_ENGINE=1 for non-interactive first-run provisioning"
crates/ralph-cli/src/main.rs:1336:                "Warning: the configured autoloop version is {version}, but Ralph requires >= {MIN_AUTOLOOP_VERSION}. Proceeding because --skip-preflight was supplied. Update it with: {AUTOLOOP_INSTALL_HINT}"
crates/ralph-cli/src/engine_install.rs:212:            "installed engine version {version} is too old; Ralph requires >= {MIN_AUTOLOOP_VERSION}; the incomplete install was removed"
crates/ralph-cli/src/engine_provision.rs:54:        "autoloop engine not found — download v{VENDORED_AUTOLOOP_VERSION} to ~/.ralph/engine now? [Y/n]"
crates/ralph-cli/src/engine_provision.rs:60:        "autoloop engine v{found_version} at {} is too old; the vendored engine will outrank PATH — download v{VENDORED_AUTOLOOP_VERSION} to ~/.ralph/engine now? [Y/n]"
crates/ralph-cli/src/engine_provision.rs:105:#[cfg(test)]
```

```text
0.11.0
```

### Finding

The mapping was authored against autoloop 0.10.x. Both comments say so, at
`autoloop_preset_gen.rs:158` (budget overrides) and `:212` (backend keys). The
installed and published engine is 0.11.0. The drift is real and named.

Two constants lag further than the comments. `MIN_AUTOLOOP_VERSION` is `0.10.0`
and `VENDORED_AUTOLOOP_VERSION` is `0.10.1` at `autoloop_health.rs:12` and
`:15`. The accepted floor is therefore **one** minor version behind the installed
engine: `0.10.0` against `0.11.0`, not two.

No production surface carries a hardcoded `0.10` literal. Every operator-facing
version message interpolates those two constants, so correcting the constants
corrects every message. The production surfaces are `preflight.rs:450`,
`main.rs:1325`, `:1328`, `:1336`, `engine_install.rs:212`, and
`engine_provision.rs:54`, `:60`. `doctor.rs:695` and `:710` are not among them.
They sit inside `#[cfg(test)] mod tests` at `doctor.rs:665` and are fixtures.

What this task does **not** establish: whether 0.11.0 actually breaks a mapping.
That needs a run against the installed engine, which is Step 12 (Phase 3.5). The
candidate break points are the budget keys, the backend key set, and the
`stopReason` contract. Issue #37 (versioned `stopReason`) closed, so a contract
change between 0.10.x and 0.11.0 is plausible and unverified. Label this
**inferred**.

## Corrections to the brief

1. The in-house engine leaves two remnants, not one. `event_bus.rs` (401 lines,
   zero callers, still exported) is missing from the brief's list.
2. `hat_registry.rs` is not dead. It has one production caller, the `ralph hats`
   command. a7e.10 is a relocation or a caller rework, not a delete.
3. The "no autoloop engine is installed" claim is stale. 0.11.0 is on PATH.
4. The brief's line citations `2188` and `2434` for the engine guard are stale.
   The live lines are `512` and `2361`/`2362`, already noted in the engine-flip
   log.
