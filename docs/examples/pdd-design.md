# Automated PDD Design Example

!!! note "Example Only"
    `docs/examples/presets/auto-pdd.yml` is an example workflow, not a supported builtin preset.

## Overview

This example automates the front half of PDD.
Instead of pausing for a human to answer discovery questions, two hats simulate a requirements interview:

1. A `Requirements Interviewer` asks one sharp question at a time.
2. A `Requirements Owner` answers from the prompt, repo context, and explicit assumptions.

Once the interview is complete, a `PDD Author` writes the design package and a `Design Critic` adversarially reviews it. The loop stops only after that design package is approved.

## Output

The workflow writes a PDD-style package under `.agents/scratchpad/implementation/{spec_name}/`:

- `rough-idea.md`
- `idea-honing.md`
- `requirements.md`
- `design.md`

It does not generate implementation tasks or write code.

## Usage

```bash
ralph run --config docs/examples/presets/auto-pdd.yml --prompt "Design a resilient import pipeline for CSV uploads"
```

## Why Use It

Use this when you want:

- an automated design pass from a rough prompt,
- explicit assumptions instead of blocked human Q&A,
- and an adversarial design gate before implementation starts.

If you want the longer supported workflow that continues through implementation, use `builtin:pdd-to-code-assist` instead.
