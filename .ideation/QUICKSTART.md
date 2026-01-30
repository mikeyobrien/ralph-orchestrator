# Content Ideation - Quick Start

Generate AI-driven content ideas in 3 steps.

## Prerequisites

```bash
# Build Ralph (one-time setup)
cargo build --release
```

## Basic Usage

```bash
# 1. Setup inputs (avatar + prompt)
./.ideation/ideate setup myla late-night-techno

# 2. (Optional) Customize the prompt
$EDITOR .ideation/input/prompt.md

# 3. Generate ideas
./.ideation/ideate run

# 4. View results
./.ideation/ideate show     # See passing ideas (avg >= 7.0)
./.ideation/ideate stats    # View statistics
```

## What Happens

The system runs a streamlined 5-hat orchestration loop:

```
Planner → 2 Creators → 2 Reviewers → Completion Checker
```

**Creators:** Trend Spotter (zeitgeist) + Storyteller (narrative)
**Reviewers:** Audience+Brand (appeal + authenticity) + Critic (quality check)

**Output:** `.ideation/output/ideas.yaml` with scored content ideas

**Time:** ~2-3 minutes per run (50% faster than 8-hat version)

**Goal:** Generate ≥3 passing ideas (avg score ≥7.0)

## Key Commands

| Command | Purpose |
|---------|---------|
| `ideate setup [avatar] [template]` | Copy avatar and prompt to input/ |
| `ideate run [prompt]` | Generate ideas |
| `ideate show` | Display passing ideas |
| `ideate stats` | View run statistics |
| `ideate split` | Split ideas into passing/failing files |
| `ideate archive` | Save everything with timestamp |
| `ideate clean` | Clear output files |

## Available Avatars

- `myla` - Electronic music curator (default)

## Available Templates

- `late-night-techno` - Late-night dancefloor content (default)
- `trend-analysis` - Current trends and cultural moments

## Create Custom Inputs

**New Avatar:**
```bash
cp .ideation/templates/avatar/myla.yaml .ideation/templates/your-avatar.yaml
$EDITOR .ideation/templates/your-avatar.yaml
```

**New Template:**
```bash
cp .ideation/templates/trend-analysis.md .ideation/templates/your-template.md
$EDITOR .ideation/templates/your-template.md
```

## Example Workflow

```bash
# Generate ideas for different scenarios
./.ideation/ideate setup myla late-night-techno
./.ideation/ideate run

# Archive results
./.ideation/ideate archive

# Try different angle
./.ideation/ideate setup myla trend-analysis
$EDITOR .ideation/input/prompt.md  # Customize
./.ideation/ideate run
```

## Output Structure

```yaml
ideas:
  - id: "trend-001-1234567890"
    title: "Your Idea Title"
    hook: "Opening line that grabs attention"
    angle: "The unique perspective"
    rationale: "Why this works"
    mood: reflective
    format: story
    duration_seconds: 75
    creator: "trend_spotter"
    reviews:
      - reviewer: "audience"
        score: 8.5
        feedback: "Strong hook..."
      - reviewer: "brand"
        score: 9.0
        feedback: "Perfect voice..."
      - reviewer: "critic"
        score: 7.0
        feedback: "Good but..."
    avg_score: 8.2
    status: "passing"
```

## Troubleshooting

**"ralph: command not found"**
→ Run `cargo build --release` first

**"No ideas generated"**
→ Check inputs exist: `ls .ideation/input/`

**"All ideas failing"**
→ Check avatar constraints alignment or refine prompt

## Learn More

- **Full Docs:** [.ideation/README.md](.ideation/README.md)
- **Constitution:** [specs/constitution.md](../specs/constitution.md)
- **Contributing:** [.ideation/CONTRIBUTING.md](CONTRIBUTING.md)

## Memory System

Ralph learns patterns across runs:

```bash
# Add patterns manually
./target/release/ralph tools memory add "pattern: Questions score higher" -t pattern

# Search patterns
./target/release/ralph tools memory search "myla"
```

Memories are automatically injected into future runs to improve quality.

---

**Ready?** Run `./.ideation/ideate setup && ./.ideation/ideate run` 🚀
