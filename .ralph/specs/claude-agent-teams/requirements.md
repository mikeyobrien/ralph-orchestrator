# Requirements

## Q&A Record

_(Questions and answers will be appended as the requirements clarification progresses.)_

### Q1: SOP duplication vs conditional assembly

The issue proposes duplicating the PDD SOP into `pdd.md` (unchanged) and `pdd-teams.md` (team-aware variant). The alternative is keeping a single `pdd.md` and having `build_prompt()` conditionally append a team instructions section when `--teams` is passed.

**Duplication** trades drift risk for simplicity — two small (~166 line) files, clear separation.
**Conditional assembly** avoids drift but adds complexity to prompt construction.

Which approach do you prefer?

**A1:** Conditional assembly preferred, as long as it doesn't require significant changes to support. If it turns out to be complex, fall back to duplication.

### Q2: Non-Claude backend behavior

Agent Teams is Claude-only. When a user runs `ralph plan --teams "idea" --backend gemini`, should the CLI:

- **(a)** Print a warning and proceed without teams (graceful degradation)
- **(b)** Error out and refuse to run

Codebase research shows: `bail!()` is used for unrecoverable incompatibilities. `warn!()` + fallback is used for degradable features (e.g., interactive→autonomous when no TTY). Since `ralph plan` works fine without teams (just runs sequentially), the warn-and-continue pattern fits best.

**A2:** Warn-and-continue. Print a warning that `--teams` is ignored for non-Claude backends, then proceed with a normal sequential PDD session.

### Q3: DisallowedTools adjustment approach

`claude_interactive()` hardcodes `--disallowedTools=TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet`. When teams are enabled, `TaskCreate`, `TaskUpdate`, `TaskList`, `TaskGet` must be re-allowed (Agent Teams uses them internally for coordination). `TodoWrite` should stay disallowed.

Three options:

- **(a)** Add a new method `claude_interactive_teams()` that only disallows `TodoWrite` — matches the existing pattern of separate methods per variant
- **(b)** Parameterize `claude_interactive(agent_teams: bool)` to conditionally build the disallowed list
- **(c)** Adjust the args after construction in `run_sop()` / `spawn_interactive()`

Which do you prefer?

**A3:** Option (a) — new `claude_interactive_teams()` method. Matches existing codebase pattern of separate methods per variant.

### Q4: Team instructions content

The conditional assembly approach will append team-specific instructions to the prompt when `--teams` is passed. These instructions modify PDD Steps 4 (Research), 5 (Iteration Checkpoint), and 6 (Design).

How opinionated should the team instructions be about team structure? Options:

- **(a)** Prescriptive: Specify exact teammates to spawn (e.g., "spawn a codebase researcher, an external researcher, and a technology evaluator for research; spawn a design critic for design review")
- **(b)** Suggestive: Describe the goals and let Claude self-organize (e.g., "use Agent Teams to parallelize research across multiple teammates; use adversarial review during design")
- **(c)** Hybrid: Suggest a default team structure but explicitly say Claude can adapt based on the task

**A4:** Option (b) — suggestive. Describe goals and let Claude self-organize the team structure.

### Q5: Should `--teams` also apply to `ralph code-task`?

The SOP runner serves both `ralph plan` (PDD) and `ralph code-task` (code task generator). The issue only mentions `ralph plan`, but the plumbing changes to `SopRunConfig` and `spawn_interactive()` would make it trivial to support `--teams` for `ralph code-task` too.

Should we scope this strictly to `ralph plan` only, or wire it up for both commands?

**A5:** Both. Wire up `--teams` for `ralph plan` and `ralph code-task`.

### Q6: Team instructions file location

The conditional team instructions need to live somewhere. Options:

- **(a)** Inline in `build_prompt()` as a string literal — simplest, but mixes prompt content with Rust code
- **(b)** Separate file `crates/ralph-cli/sops/pdd-team-addendum.md` loaded via `include_str!` — keeps prompt content in markdown, easy to edit
- **(c)** Appended to the existing `sops::PDD` constant in `sop_runner.rs` module — new constant like `sops::PDD_TEAM_ADDENDUM`

(b) and (c) are essentially the same pattern, just naming. The question is really: inline string vs separate file?

**A6:** Option (b) — separate markdown file loaded via `include_str!`.
