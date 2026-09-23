# Step 1 evidence: release gate reference, branch deltas, bead census

Task: `task-1790091216-6caf`
Key: `code-assist:v3-complete:step-01:verify-gate-and-branches`
Branch: `v3/complete` at `df2449a`
Date: 2026-09-22
Source edits: none.

Every claim carries the command and an output excerpt.

## Claim 1. The release gate reference names a path that does not exist

### Command

```bash
git rev-parse --is-shallow-repository
grep -n 'release_gate' .ralph/specs/v3-autoloops-cutover.spec.md
for r in origin/integration/v3-prerelease origin/wip/v3-prerelease-rollup origin/main; do
  git cat-file -e "$r:.ralph/specs/v3-ga-readiness.spec.md" && echo "$r PRESENT" || echo "$r ABSENT"
done
```

### Output excerpt

```text
false
```

```text
10:release_gate: .ralph/specs/v3-ga-readiness.spec.md
```

```text
origin/integration/v3-prerelease         ABSENT
origin/wip/v3-prerelease-rollup          ABSENT
origin/main                              ABSENT
```

### Finding

Confirmed. The cutover spec declares `release_gate:` at line 10 and also lists
`v3-ga-readiness.spec.md` under `related:` at line 8, so the reference is single
targeted. The file is absent on all three branches by `git cat-file -e`, which
tests the object at the exact ref without checkout. The clone is not shallow
(`false`), so the absent result is a real absence and not a graft artifact.

The `origin/integration/v3-prerelease` spec directory holds one v3 spec, the
cutover spec itself. `origin/main` holds no `v3-*` spec at all:

```text
--- origin/integration/v3-prerelease
v3-autoloops-cutover.spec.md
--- origin/wip/v3-prerelease-rollup
v3-autoloops-cutover.spec.md
--- origin/main
(no v3-* entries)
```

Consequence: the brief's fourth claim holds. The named gate must be authored, or
the reference corrected, before the cutover spec can be read as satisfied. Step
14 of the plan owns authoring `.ralph/specs/v3-ga-readiness.spec.md`.

## Claim 2. The two v3 branches are divergent, not stacked

### Command

```bash
git fetch origin
git rev-list --count origin/integration/v3-prerelease..origin/wip/v3-prerelease-rollup
git rev-list --count origin/wip/v3-prerelease-rollup..origin/integration/v3-prerelease
git log --format='%h %s' origin/integration/v3-prerelease..origin/wip/v3-prerelease-rollup
git log --format='%h %s' origin/wip/v3-prerelease-rollup..origin/integration/v3-prerelease
git merge-base origin/integration/v3-prerelease origin/wip/v3-prerelease-rollup
```

### Output excerpt

```text
rollup-only: 22
base-only:   8
```

Tips after fetch:

```text
origin/integration/v3-prerelease 22fc1fd1fbe6157207df70148401943f6b00c823
origin/main                      edc2b3268c9bd0c08a12c8193a7ace7ab2789261
origin/wip/v3-prerelease-rollup  6e2545d63e21c0fe34dc6bd48ad0c7b2929145c0
```

Merge base: `2e1fc52e3286520540eb2538fa5d5ae53baf72c9`.

The 22 commits the rollup has and the base lacks:

```text
6e2545d Merge WIP live harness smoke preset
7c9b0ff Merge WIP TUI stream history and backpressure fixes
c90001e Merge WIP Ralph-owned Autoloop state
3322fd7 docs(engine): correct final gate evidence
2242eec fix(smoke): harden live provider safety gates
5b7876c fix(tui): bound stream identities and lifecycle lines
1e67e52 chore: auto-commit before merge (loop primary)
faa2b71 fix(smoke): validate canonical completion without retries
86066b6 fix(tui): protect reconciled history under line pressure
ebeb81f fix(smoke): abort immediately on missing provider handoff
1a2a43d fix(smoke): render executable contracts for every role
0a5f660 fix(smoke): resolve provider-visible run evidence path
49434db chore: satisfy strict touched-crate clippy
3d4b8ca fix: satisfy clippy lifetime lint
2ac3c1f style: apply workspace rustfmt
e275303 fix: preserve bounded TUI stream history
de2eaa4 fix: bound backend stream identity and backpressure
3c8eaed docs(presets): explain manual live smoke
91b094d test(smoke): cover fake live harness matrix
47f8dfd feat(tools): validate live smoke evidence
830ecfe feat(tools): add bounded live harness smoke runner
e0178bf feat(presets): add manual live harness smoke
```

The 8 commits the base has and the rollup lacks:

```text
22fc1fd feat(cli): restore ralph run --rpc and native ralph resume (#368)
70b3360 feat(cli): restore ralph run --rpc and native ralph resume
8276db0 fix(telegram): wire /stop and /restart and skip stale guidance (#367)
893f129 Merge pull request #366 from mikeyobrien/cursor/v3-telegram-autoloop-74c5
47a568e Merge pull request #365 from mikeyobrien/cursor/v3-ralph-bench-autoloop-74c5
775e98a fix(cli): invert RObot enablement check for clippy if_not_else
42359f3 feat(robot): relay Telegram HITL through Autoloop control (#345)
92e4992 feat(bench): drive ralph-bench task execution on the autoloop engine (#346)
```

### Finding

The measured counts are 22 rollup-only and 8 base-only. The brief's table claims
22 and 8, so the table is correct. The task text says the brief claims 14 and 8;
14 does not appear in the brief and does not match the tree. The real number is
22 and it is the one the plan's Step 2 uses ("enumerate and classify the
22-commit rollup delta").

Two subtleties for Step 2 classification:

1. Three of the 22 are merge commits (`6e2545d`, `7c9b0ff`, `c90001e`) that carry
   the named WIP work: live harness smoke preset, TUI stream history and
   backpressure, and Ralph-owned autoloop state. Classification of the two
   direction lists must not double count the branch commits under their merges.
2. `1e67e52 chore: auto-commit before merge (loop primary)` is the landing sweep
   artifact. It is direct evidence for bead
   `ralph-orchestrator-landing-untracked-sweep-yxv`: a loop auto-commit landed on
   the rollup branch. Its content is worth reading when scoping the fix in Step 6.

Direction asymmetry matters for the merge. The base carries the four named
features the brief lists (#368 rpc and native resume, #367 telegram stop/restart,
#366, #365, #345 Telegram HITL relay, #346 ralph-bench on the engine) plus two
clippy commits. The rollup carries none of them, so a merge of rollup into base
keeps base behavior and adds the rollup's TUI and smoke work. The reverse merge
would drop them, which is why Step 2 reconciles rather than fast forwards.

## Claim 3. Bead tracker census

### Command

```bash
wc -l < .beads/issues.jsonl
python3 - <<'PY'
import json, collections
rows = [json.loads(l) for l in open('.beads/issues.jsonl') if l.strip()]
print(dict(collections.Counter(r.get('status') for r in rows)))
for r in rows:
    if r.get('status') != 'closed':
        print(r.get('status'), r.get('id'), '|', r.get('title'))
PY
```

### Output excerpt

```text
43
```

```text
{'closed': 36, 'open': 6, 'tombstone': 1}
```

```text
open ralph-orchestrator-ga3-c4-dashboard-dead-svf | C4: web dashboard data source severed - both backends read files autoloop never writes
open ralph-orchestrator-landing-untracked-sweep-yxv | Landing auto-commit sweeps untracked operator files into the loop branch
open ralph-orchestrator-tui-help-wave-stale-5hu | TUI help overlay still lists deleted Wave Workers section
open ralph-orchestrator-v3-autoloops-backend-a7e | v3: use autoloops SDK as orchestration backend
open ralph-orchestrator-v3-autoloops-backend-a7e.10 | Delete in-house engine once parity green; flip default
open ralph-orchestrator-v3-autoloops-backend-a7e.8 | Map ralph waves onto autoloop parallel model
tombstone ralph-orchestrator-vp6 | bug
```

### Finding

Confirmed exactly. 43 rows: 36 closed, 6 open, 1 tombstone. The six open ids are
`a7e`, `a7e.8`, `a7e.10`, `ga3-c4-dashboard-dead-svf`,
`landing-untracked-sweep-yxv`, `tui-help-wave-stale-5hu`. The tombstone is `vp6`.
This matches the brief's eighth ground-truth row and the tracker census the audit
must reconcile per bead.

Status is a hypothesis, not a verdict. The audit's purpose is to decide, per
open row, whether the tree agrees. `a7e.10` is already contradicted by the
Step 1 remnant finding (the module still exists, so the row is correctly open,
but the work is a relocate or rework rather than a deletion). `a7e.8` is
contradicted by the same log's absent-module check only with respect to wave
naming still live in `json_rpc.rs` and `rpc_source.rs`; the preset mapping itself
is unmeasured and belongs to Step 4.

The tracker file's last commit is `aff233d chore: sync beads issue history`, and
`.beads/` is clean in `git status`. No beads CLI exists, so any status update
must preserve the schema keys
(`id, title, status, priority, issue_type, dependencies, ...`) by hand.

## Summary

| Claim | Brief says | Measured | Verdict |
| --- | --- | --- | --- |---|
| release gate path exists on the branches | absent | absent on v3 base, rollup, and main | confirmed |
| rollup-only delta | 22 | 22 | confirmed |
| base-only delta | 8 | 8 | confirmed |
| tracker census | 36 closed, 6 open, 1 tombstone | 36 closed, 6 open, 1 tombstone | confirmed |

Two corrections to carry into the audit. The task text's "14" for the rollup
delta is wrong; the number is 22 and the brief already says 22. The rollup
contains a loop auto-commit (`1e67e52`) that is direct evidence for the landing
sweep defect.

No source edits. Handed to the Critic.

## Critic pass

Every acceptance command was re-run from a fresh shell rather than trusted from
this log. Results: the `release_gate` line matches once, the gate path is ABSENT
on all three refs, the deltas are 22 and 8, and the census is 43 rows with 36
closed, 6 open, 1 tombstone. All four claims reproduce. Verdict upheld.
