# Preset Contracts

These are the shared behavioral contracts the shipped presets are expected to follow.

The goal is not to force every preset into identical YAML, but to keep the preset family coherent enough that runs can be evaluated and trusted consistently.

## Core principle
A preset should make autonomous forward progress without relying on accidental behavior.

That means each preset should have:
- clear hat ownership
- clear terminal semantics
- clear review / follow-up semantics
- explicit separation between artifact success and loop success

## Autonomous task loop contract
Used by implementation-style presets such as `code-assist`.

Expected roles:
- `planner`
- `builder`
- `critic`
- `finalizer`

Expected shape:
- planner scopes and advances work
- builder implements one bounded task at a time
- critic performs adversarial review
- finalizer decides continue vs complete
- completion event is owned by the finalizer, not emitted ad hoc by builder/critic

## Autonomous review loop contract
Used by `review`.

Expected roles:
- `reviewer`
- `analyzer`
- `closer`

Expected shape:
- reviewer produces the next review wave
- analyzer deepens the highest-risk findings
- closer decides follow-up vs `REVIEW_COMPLETE`

## Autonomous debug loop contract
Used by `debug`.

Expected roles:
- `investigator`
- `tester`
- `fixer`
- `verifier`

Expected shape:
- investigator frames one falsifiable hypothesis at a time
- tester validates or rejects the hypothesis
- fixer applies the minimal change
- verifier confirms the bug is gone
- only investigator should emit `DEBUG_COMPLETE`, and only after verified fix state

## Autonomous research loop contract
Used by `research`.

Expected roles:
- `researcher`
- `synthesizer`

Expected shape:
- researcher gathers bounded evidence for one wave
- synthesizer decides follow-up vs `RESEARCH_COMPLETE`
- only synthesizer should emit `RESEARCH_COMPLETE`

## PDD-to-code-assist contract
Used by `pdd-to-code-assist`.

Expected shape:
- requirements/design/planning/task-writing/build/review/finalize/validate stages stay distinct
- clarifications should resolve sanely for autonomous operation; the workflow must not pretend a human replied when no human path exists
- implementation completion must be driven by the designated late-stage hats, not by early-stage hats re-emitting stale completion-like events

## Why this file exists
This page documents the intended preset family shape so we can:
- test for drift
- judge conformance runs correctly
- keep shipped presets coherent as the runtime evolves
