# Content Ideation System

AI-driven content idea generation using Ralph orchestrator's multi-agent collaboration.

## Quick Start

```bash
# 1. Setup inputs (copies avatar + template)
./bin/ideate setup myla late-night-techno

# 2. Edit prompt with specifics
$EDITOR .ideation/input/prompt.md

# 3. Run ideation
./bin/ideate run

# 4. View results
./bin/ideate show
./bin/ideate stats

# 5. Archive good runs
./bin/ideate archive
```

## How It Works

**Streamlined Multi-Agent Loop (5 hats):**
1. **Planner** - Analyzes avatar + prompt, creates tasks
2. **Creators** - Generate ideas (Trend Spotter, Storyteller)
3. **Reviewers** - Score ideas (Audience+Brand, Critic)
4. **Checker** - Iterate or complete (need 3+ ideas with avg ≥ 7.0)

**Optimization:** Reduced from 8 to 5 hats for 50% faster iterations while maintaining quality.

**Inputs:**
- `input/avatar.yaml` - WHO is creating (personality, voice, expertise)
- `input/prompt.md` - WHAT to create (theme, angles, constraints)

**Outputs:**
- `output/ideas.yaml` - Scored content ideas with reviews

## Directory Structure

```
.ideation/
├── input/           # Per-run inputs
│   ├── avatar.yaml  # Copy from avatars/
│   └── prompt.md    # Copy from templates/ or write custom
│
├── output/
│   └── ideas.yaml   # Generated ideas with scores
│
├── avatars/         # Reusable avatar profiles
│   ├── myla.yaml
│   └── README.md
│
├── templates/       # Reusable prompt templates
│   ├── late-night-techno.md
│   ├── trend-analysis.md
│   └── README.md
│
└── archive/         # Historical runs
    └── 20260129-120000-myla-late-night.yaml
```

## Using the CLI Wrapper

```bash
# Setup with defaults (myla + late-night-techno)
./bin/ideate setup

# Setup with specific avatar and template
./bin/ideate setup myla trend-analysis

# Run with custom prompt
./bin/ideate run "Generate festival season content ideas"

# View only passing ideas
./bin/ideate show

# View statistics
./bin/ideate stats

# Archive results
./bin/ideate archive

# Clear outputs
./bin/ideate clean
```

## Using Ralph Directly

```bash
# More control over Ralph configuration
ralph run -c presets/content-ideation.yml -p "Generate ideas" --max-iterations 30

# View diagnostics
RALPH_DIAGNOSTICS=1 ralph run -c presets/content-ideation.yml -p "Generate ideas"

# Use memories from previous runs
ralph tools memory search "myla OR techno"
```

## Creating Custom Avatars

Copy and edit:
```bash
cp .ideation/avatars/myla.yaml .ideation/avatars/custom.yaml
$EDITOR .ideation/avatars/custom.yaml
```

Schema: see `avatars/README.md`

## Creating Custom Prompts

Copy and edit:
```bash
cp .ideation/templates/trend-analysis.md .ideation/templates/custom.md
$EDITOR .ideation/templates/custom.md
```

## Output Schema

```yaml
run_id: "20260129-120000"
timestamp: "2026-01-29T12:00:00Z"
avatar: "Myla"
theme: "Late-night melodic techno"
rounds: 2

ideas:
  - id: "trend-001-1738152000"
    title: "The 3am Shift"
    hook: "Everyone talks about peak time. Nobody talks about what happens at 3am."
    angle: "The dancefloor transforms after midnight - energy drops but connection deepens"
    rationale: "Taps into insider knowledge and emotional authenticity"

    mood: reflective
    format: monologue
    duration_seconds: 75
    creator: "trend_spotter"

    reviews:
      - reviewer: "audience"
        score: 8.5
        feedback: "Hook is strong, immediately creates curiosity"
      - reviewer: "brand"
        score: 9.0
        feedback: "Perfect Myla voice - calm, insider, authentic"
      - reviewer: "critic"
        score: 7.0
        feedback: "Good but angle could be more specific"

    avg_score: 8.2
    status: "passing"
```

## Memory System

Ralph remembers patterns across runs:

```bash
# Add patterns manually
ralph tools memory add "pattern: Hooks with questions score +1.5 higher" -t pattern

# Search patterns
ralph tools memory search "myla" --tags pattern

# View all memories
ralph tools memory list
```

Memories are injected automatically into agent context.

## Troubleshooting

**No ideas generated:**
- Check inputs exist: `ls -la .ideation/input/`
- Check preset loads: `yq eval . presets/content-ideation.yml`
- Run with diagnostics: `RALPH_DIAGNOSTICS=1 ./bin/ideate run`

**Low scores:**
- Check avatar constraints alignment
- Search memories for patterns: `ralph tools memory search "low score"`
- Iterate with more specific prompt

**Loop doesn't complete:**
- Check max_iterations: `yq eval '.event_loop.max_iterations' presets/content-ideation.yml`
- Monitor progress: `ralph tools task list`
- Check completion threshold: need 3+ ideas with avg ≥ 7.0

## Configuration

Edit `presets/content-ideation.yml`:

- `max_iterations: 25` - Maximum loop iterations
- `max_runtime_seconds: 3600` - Maximum runtime (1 hour)
- Completion threshold in completion_checker hat instructions
- Creator/reviewer scoring logic in hat instructions

## References

- [Constitution](../specs/constitution.md) - Full system design
- [Ralph Docs](https://mikeyobrien.github.io/ralph-orchestrator/)
- [Hat System](https://mikeyobrien.github.io/ralph-orchestrator/concepts/hats-and-events/)
