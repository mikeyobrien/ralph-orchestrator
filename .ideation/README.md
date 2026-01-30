# Content Ideation System

AI-driven content idea generator using Ralph orchestrator's multi-agent pipeline.

## What It Does

Generates scored content ideas using a 5-agent pipeline:

1. **Planner** - Reads your avatar + prompt, creates tasks
2. **Trend Spotter** - Generates culturally relevant ideas
3. **Storyteller** - Generates narrative-driven ideas
4. **Audience & Brand Reviewer** - Scores on appeal + authenticity
5. **Critic** - Ruthlessly scores on originality + quality

**Completion:** Loop runs until you have 3+ passing ideas (avg score ≥ 8.0).

**Output:** `.ideation/output/ideas.yaml` with scored ideas + reviews

## Quick Start

```bash
# 1. Setup (copies avatar + prompt templates)
./.ideation/ideate setup myla late-night-techno

# 2. Edit prompt with your specifics
nano .ideation/input/prompt.md

# 3. Generate ideas (auto-archives on completion)
./.ideation/ideate run

# 4. View results
./.ideation/ideate show
./.ideation/ideate stats
```

## Commands

| Command | Description |
|---------|-------------|
| `setup [avatar] [template]` | Copy avatar + prompt to input/ (default: myla late-night-techno) |
| `run [prompt]` | Generate ideas (auto-archives on success) |
| `continue [N]` | Generate N MORE ideas (default: 3) on top of existing ones |
| `show` | Display passing ideas (≥8.0) |
| `stats` | Show round count, passing/failing breakdown |
| `archive` | Manually archive current run (auto-called after runs) |
| `clean` | Clear output files |

## Typical Workflow

```bash
# Initial run
./.ideation/ideate setup myla late-night-techno
nano .ideation/input/prompt.md
./.ideation/ideate run

# View results
./.ideation/ideate show

# Need more ideas?
./.ideation/ideate continue      # Generate 3 more
./.ideation/ideate continue 5    # Generate 5 more
```

## Directory Structure

```
.ideation/
├── ideate              # Main executable
├── preset.yml          # Ralph configuration
├── templates/          # Avatar profiles + prompt templates
│   ├── myla.yaml
│   ├── avatar-schema.md
│   ├── late-night-techno.md
│   └── trend-analysis.md
├── input/              # Working directory (your edits)
│   ├── avatar.yaml
│   └── prompt.md
├── output/
│   └── ideas.yaml      # Generated ideas
└── archive/            # Timestamped archives
    └── 20260130-102000-myla-theme/
        ├── all-ideas.yaml
        ├── passing-ideas.yaml
        └── failing-ideas.yaml
```

## Customization

### Create New Avatar

```bash
cp .ideation/templates/myla.yaml .ideation/templates/your-avatar.yaml
nano .ideation/templates/your-avatar.yaml
./.ideation/ideate setup your-avatar late-night-techno
```

See `templates/avatar-schema.md` for schema.

### Create New Prompt Template

```bash
cp .ideation/templates/trend-analysis.md .ideation/templates/your-template.md
nano .ideation/templates/your-template.md
./.ideation/ideate setup myla your-template
```

### Adjust Scoring

Edit `.ideation/preset.yml`:
- Change passing threshold (line 269, 276, 307): `avg_score >= 8.0`
- Modify reviewer scoring criteria
- Adjust completion threshold (default: 3 passing ideas)

## Output Format

```yaml
run_id: "20260130-102000"
timestamp: "2026-01-30T10:20:00Z"
avatar: "Myla"
theme: "How to become an influencer"
rounds: 2

ideas:
  - id: "trend-001-1738234800"
    title: "Your niche isn't a genre - it's a perspective"
    hook: "Stop trying to be 'the melodic techno person.' That's not a niche."
    angle: "Reframe niche as unique perspective, not category"
    rationale: "Addresses common misconception, actionable reframe"
    mood: provocative
    format: tip
    duration_seconds: 75
    creator: "trend_spotter"
    reviews:
      - reviewer: "audience_brand"
        score: 7.5
        feedback: "Strong hook, slightly generic angle"
      - reviewer: "critic"
        score: 8.5
        feedback: "Fresh perspective on tired advice"
    avg_score: 8.0
    status: "passing"
```
