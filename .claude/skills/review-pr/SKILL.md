---
name: review-pr
description: Use when asked to review a PR, run a code review loop, or invoke the ralph reviewer against a pull request number or GitHub URL
---

# Review PR

Run the `ralph.reviewer.yml` orchestration loop against a pull request. The loop checks out the PR in an isolated worktree, runs tests, reviews the diff, and produces a structured report.

## Usage

```
/review-pr <PR number or URL>
```

Accepts: `207`, `#207`, or `https://github.com/.../pull/207`

## Execution

### 1. Parse the PR argument

Extract the PR number from the argument. Strip `#` prefix or extract from URL path.

### 2. Run the reviewer loop

```bash
ralph run -H ralph.reviewer.yml -p "Review PR #<N>"
```

**Bash tool settings:**
- `timeout: 600000` (10 minutes)
- `run_in_background: true`

Use `TaskOutput` with `block: true` to wait for completion.

### 3. Display the report

Read and print `.ralph/REVIEW-REPORT.md` to the conversation.

If the report file doesn't exist (loop failed before the synthesizer hat), check for and display whatever intermediate files exist:
1. `.ralph/review-scope.md` — what was scoped
2. `.ralph/review-verification.md` — test results
3. `.ralph/review-findings.md` — review findings

### 4. Verify cleanup

Check that the review worktree was removed:

```bash
ls -d .worktrees/review-<N> 2>/dev/null
```

- If gone: cleanup succeeded, no action needed.
- If still present: **warn the user** but do NOT force-remove. Say what's there and let them decide.

Also note the presence of intermediate files (`.ralph/review-scope.md`, etc.) — they're useful for debugging but the user may want to clean them up later.

## Error Handling

| Situation | Action |
|-----------|--------|
| Ralph exits non-zero | Display error output. Suggest re-running with `RALPH_DIAGNOSTICS=1` |
| Report file missing | Display intermediate files that do exist (scope, verification, findings) |
| PR argument missing | Ask the user for the PR number |
| PR argument unparseable | Ask the user to provide a bare number, `#N`, or full GitHub URL |

## What This Skill Does NOT Do

- Does NOT validate the PR exists (the scoper hat handles that)
- Does NOT modify any source code (read-only review)
- Does NOT post comments to GitHub
- Does NOT own worktree cleanup (verifies only)
- Does NOT enable diagnostics by default
