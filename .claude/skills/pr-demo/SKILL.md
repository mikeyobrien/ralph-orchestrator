---
name: pr-demo
description: Use when creating animated demos (GIFs) for pull requests or documentation. Covers terminal recording with asciinema and conversion to GIF/SVG for GitHub embedding.
---

# PR Demo Creation

## Overview

Create polished terminal demos for PRs using asciinema recordings converted to GIF. The workflow: **script → record → convert → embed**.

## Tool Selection

| Goal | Tool Chain | Output |
|------|------------|--------|
| CLI demo for GitHub PR | asciinema → agg | GIF (< 5MB) |
| Smaller file needed | asciinema → svg-term-cli | SVG (< 500KB) |
| TUI screenshot | tmux → freeze | SVG/PNG |

**Default choice:** asciinema + agg (best compatibility, GitHub renders GIFs natively)

## Prerequisites

```bash
# Install tools (macOS)
brew install asciinema
cargo install --git https://github.com/asciinema/agg
npm install -g svg-term-cli  # Optional: for SVG output
```

## Workflow

### 1. Script Your Demo (REQUIRED)

Before recording, write a brief script:

```markdown
## Demo: [feature name]
Duration: ~20-30 seconds

1. [0-3s] Show command being typed
2. [3-10s] Command executes, show key output
3. [10-25s] Highlight the "aha moment" - what makes this valuable
4. [25-30s] Clean exit or final state
```

**Keep it short.** 20-30 seconds max. Show ONE thing well.

### 2. Prepare Environment

```bash
# Clean terminal state
clear
export PS1='$ '                    # Simple prompt
export TERM=xterm-256color         # Consistent colors
# Hide sensitive info (API keys, paths with usernames)
```

Terminal size: **100x24** (readable when scaled down)

### 3. Record

```bash
# Record to .cast file
asciinema rec demo.cast --cols 100 --rows 24

# Execute your scripted demo
# Press Ctrl+D or type 'exit' when done
```

**Tips:**
- Type at readable speed (not too fast)
- Pause briefly after key moments
- If you make a mistake, start over (editing is harder than re-recording)

### 4. Convert to GIF

```bash
# Basic conversion (recommended)
agg demo.cast demo.gif

# With speed adjustment (1.5x faster)
agg --speed 1.5 demo.cast demo.gif

# With custom font size for readability
agg --font-size 14 demo.cast demo.gif
```

**Alternative - SVG (smaller files):**
```bash
svg-term --in demo.cast --out demo.svg --window
```

### 5. Validate (Self-Validation)

Claude can self-validate demos using three approaches:

#### A. Automated Checks (run these first)

```bash
# Check file size (must be < 5MB for GitHub)
ls -lh demo.gif

# Check recording duration from .cast metadata
head -1 demo.cast | jq '.duration // "check manually"'
```

#### B. Visual Validation (LLM-as-judge)

Extract a static frame for Claude to analyze:

```bash
# Option 1: Use svg-term to render a specific timestamp (e.g., 15 seconds in)
svg-term --in demo.cast --out demo-preview.svg --at 15000

# Option 2: Use asciinema cat + freeze for a snapshot
asciinema cat demo.cast | head -500 | freeze -o demo-preview.png

# Option 3: Just convert to GIF and use the file directly
# Claude can read GIF files with the Read tool
```

Then ask Claude to analyze using the Read tool on the image:

**Validation prompt:**
```
Analyze this terminal demo screenshot. Check:
1. Is the text readable (not too small/blurry)?
2. Is the command being demonstrated visible?
3. Is there any sensitive info (API keys, /Users/username paths)?
4. Does the terminal look clean (simple prompt, no clutter)?
5. Is the "aha moment" visible - what value does this demo show?

Rate: PASS or FAIL with specific issues.
```

#### C. Content Validation (parse .cast file)

The `.cast` file is JSON lines - validate the content programmatically:

```bash
# Check what commands were typed (input events)
grep '"i"' demo.cast | head -20

# Check recording duration
head -1 demo.cast | jq -r '.duration | floor'
# Should be 20-30 seconds

# Look for sensitive patterns
grep -iE '(api.?key|password|secret|/Users/[a-z])' demo.cast && echo "WARNING: Sensitive data found!"
```

#### D. Full Validation Checklist

After running the above, verify:

- [ ] File size < 5MB (automated)
- [ ] Duration 20-30 seconds (automated)
- [ ] No sensitive info in .cast (automated)
- [ ] Text readable in preview frame (visual/LLM)
- [ ] Demo shows feature clearly (visual/LLM)
- [ ] Clean terminal appearance (visual/LLM)

### 6. Embed in PR

```markdown
## Demo

![feature demo](./docs/demos/feature-demo.gif)

*Shows: [one-sentence description of what the demo shows]*
```

Store demos in `docs/demos/` or `assets/` directory.

## Quick Reference

| Setting | Recommended Value |
|---------|------------------|
| Duration | 20-30 seconds |
| Terminal size | 100x24 |
| Speed multiplier | 1.0-1.5x |
| Target file size | < 2MB ideal, < 5MB max |
| Font size (agg) | 14-16 |

## Common Mistakes

| Mistake | Fix |
|---------|-----|
| Demo too long | Script it first, show ONE thing |
| Text unreadable | Use --font-size 14+, terminal 100x24 |
| File too large | Use svg-term-cli instead, or increase speed |
| Cluttered terminal | Clean PS1, clear history, hide paths |
| No context in PR | Add one-line description below GIF |

## File Organization

```
docs/demos/
├── feature-name.gif      # The demo
├── feature-name.cast     # Source recording (optional, for re-rendering)
└── README.md             # Recording instructions for future maintainers
```
