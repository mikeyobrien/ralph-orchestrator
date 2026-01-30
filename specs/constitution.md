# Content Ideation System - Constitution v2

> Ralph-orchestrator configured for AI-driven content ideation

---

## 1. Purpose

Generate high-quality video content ideas through iterative AI collaboration, using Claude subscription (no API costs).

**Core pattern:** Planner → Creators → Reviewers → [iterate until quality threshold]

---

## 2. Architecture

### Use Ralph's Hat System

This is NOT a fork. This is **Ralph configured for content ideation**.

| Component | Approach |
|-----------|----------|
| Multi-agent coordination | Ralph's hat system (events trigger hats) |
| Claude authentication | Ralph's existing Claude CLI backend |
| Input | `.ideation/input/` directory |
| Output | `.ideation/output/ideas.yaml` |
| Workflow | Hat preset in `presets/content-ideation.yml` |

---

## 3. Directory Structure

```
content-generator/
├── .ideation/
│   ├── input/                    # Per-run inputs
│   │   ├── avatar.yaml          # WHO: Personality, voice, expertise
│   │   └── prompt.md            # WHAT: Theme, angles, constraints
│   │
│   └── output/                   # Generated outputs
│       └── ideas.yaml           # Scored content ideas
│
├── presets/
│   └── content-ideation.yml     # Hat configuration for ideation workflow
│
└── [standard Ralph structure]   # crates/, backend/, frontend/, etc.
```

---

## 4. Input Schemas

### 4.1 Avatar Profile (`.ideation/input/avatar.yaml`)

```yaml
name: string                     # Avatar name
personality: string              # Voice, tone, character
expertise: [string]              # Areas of knowledge
audience_relationship: string    # How avatar relates to audience

# Optional
constraints: string              # Things to avoid
examples: [string]               # Reference content URLs
```

**Example:**

```yaml
name: Myla
personality: |
  Calm, reflective insider who's been in electronic music for years.
  Speaks with warmth and understanding. Never pretentious.
expertise:
  - electronic music curation
  - melodic techno
  - festival culture
audience_relationship: peer and guide
constraints: |
  Avoid generic "best tracks" lists and overly technical DJ content.
```

### 4.2 Prompt (`.ideation/input/prompt.md`)

Freeform markdown containing:
- Theme or topic
- Specific angles to explore (optional)
- Format preferences (optional)
- Context or inspiration (optional)

**Example:**

```markdown
# Late-night melodic techno

Create content ideas about the late-night dancefloor experience.

## Focus areas
- Emotional journey after midnight
- Track selection philosophy
- Connection with the crowd

## Avoid
- Generic recommendations
- Technical DJ tutorials
```

---

## 5. Output Schema (`.ideation/output/ideas.yaml`)

```yaml
run_id: string
timestamp: datetime
avatar: string
theme: string
rounds: integer

ideas:
  - id: string
    title: string
    hook: string                 # Opening line (attention grabber)
    angle: string                # Perspective
    rationale: string            # Why it works

    # Metadata
    mood: [calm, hype, reflective, intimate, provocative]
    format: [monologue, story, tip, reaction, question]
    duration_seconds: integer
    creator: string              # Which hat generated this

    # Scoring
    reviews:
      - reviewer: string
        score: float (1-10)
        feedback: string

    avg_score: float
    status: [passing, failing]
```

---

## 6. Hat Configuration (`presets/content-ideation.yml`)

```yaml
event_loop:
  prompt_file: ".ideation/PROMPT.md"       # Generated from inputs
  completion_promise: "IDEATION_COMPLETE"
  max_iterations: 25
  starting_event: "ideate.start"

hats:
  planner:
    name: "Content Planner"
    triggers: ["ideate.start"]
    publishes: ["ideate.create"]
    instructions: |
      Read `.ideation/input/avatar.yaml` and `.ideation/input/prompt.md`.

      Analyze what content would resonate with this avatar's audience.
      Consider current trends, emotional hooks, and the avatar's unique voice.

      Create specific ideation tasks for each creator:

      ```bash
      ralph tools task add "Trend Spotter: Find culturally relevant angles" -p 1
      ralph tools task add "Storyteller: Find personal narrative hooks" -p 1
      ralph tools task add "Contrarian: Find unexpected perspectives" -p 1
      ```

      Then publish: `ralph emit "ideate.create" "tasks created"`

  trend_spotter:
    name: "Trend Spotter"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    instructions: |
      Generate 2-3 content ideas based on cultural relevance:
      - What conversations are happening now?
      - What's the zeitgeist in this space?
      - What would stop someone mid-scroll?

      For each idea, provide:
      - Title (punchy, specific)
      - Hook (first 5 seconds)
      - Angle (the unique perspective)
      - Rationale (why this works)
      - Suggested mood and format

      Append ideas to `.ideation/output/ideas.yaml` (create if missing).
      Then: `ralph emit "ideas.generated" "trend_spotter: 3 ideas"`

  storyteller:
    name: "Storyteller"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    instructions: |
      Generate 2-3 content ideas based on narrative and emotion:
      - What personal experiences resonate?
      - What moments create connection?
      - What stories need to be told?

      Same output format as trend_spotter.
      Append to `.ideation/output/ideas.yaml`.
      Then: `ralph emit "ideas.generated" "storyteller: 3 ideas"`

  contrarian:
    name: "Contrarian"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    instructions: |
      Generate 2-3 content ideas based on unexpected takes:
      - What does everyone get wrong?
      - What's the uncomfortable truth?
      - What assumption can we challenge?

      Same output format as trend_spotter.
      Append to `.ideation/output/ideas.yaml`.
      Then: `ralph emit "ideas.generated" "contrarian: 3 ideas"`

  audience_reviewer:
    name: "Audience Reviewer"
    triggers: ["ideas.generated"]
    publishes: ["review.done"]
    instructions: |
      Review ALL ideas in `.ideation/output/ideas.yaml` from audience POV:

      For each idea, score 1-10:
      - Would I stop scrolling for this?
      - Does the hook grab me in 2 seconds?
      - Is this relevant to me right now?

      Update each idea's `reviews` array with your score + feedback.
      Then: `ralph emit "review.done" "audience: reviewed [N] ideas"`

  brand_reviewer:
    name: "Brand Guardian"
    triggers: ["review.done"]
    publishes: ["review.done"]
    instructions: |
      Review ALL ideas in `.ideation/output/ideas.yaml` from brand POV:

      For each idea, score 1-10:
      - Does this fit the avatar's voice?
      - Is this authentic to the persona?
      - Would the avatar actually say this?

      Update each idea's `reviews` array with your score + feedback.
      Then: `ralph emit "review.done" "brand: reviewed [N] ideas"`

  critic_reviewer:
    name: "Critical Reviewer"
    triggers: ["review.done"]
    publishes: ["review.complete"]
    instructions: |
      Review ALL ideas in `.ideation/output/ideas.yaml` with skepticism:

      For each idea, score 1-10:
      - What's weak or cliché?
      - Has this been done before?
      - Is this cringe or compelling?

      Update each idea's `reviews` array with your score + feedback.
      Calculate avg_score for each idea and set status (passing if >= 7.0).
      Then: `ralph emit "review.complete" "[N] ideas reviewed, [M] passing"`

  completion_checker:
    name: "Completion Checker"
    triggers: ["review.complete"]
    publishes: []
    instructions: |
      Read `.ideation/output/ideas.yaml`.

      Check end conditions:
      1. Are there >= 3 ideas with avg_score >= 7.0? ✅ DONE
      2. Have we run 5+ rounds? ✅ DONE
      3. Otherwise: `ralph emit "ideate.create" "iterate: round [N]"`

      If done: Output completion promise: `IDEATION_COMPLETE`
```

---

## 7. UX Without UI

### Basic Workflow

```bash
# 1. Prepare input
mkdir -p .ideation/input
cat > .ideation/input/avatar.yaml <<EOF
name: Myla
personality: "Calm electronic music insider..."
expertise: [electronic music, melodic techno]
audience_relationship: peer and guide
EOF

cat > .ideation/input/prompt.md <<EOF
# Late-night melodic techno
Create ideas about the late-night dancefloor experience.
EOF

# 2. Run ideation
ralph run -c .ideation/preset.yml -p "Generate content ideas"

# 3. Review output
cat .ideation/output/ideas.yaml
```

### Advanced Patterns

**Iterate on specific ideas:**

```bash
# Extract passing ideas
yq '.ideas[] | select(.status == "passing")' .ideation/output/ideas.yaml

# Refine a specific idea
cat > .ideation/input/prompt.md <<EOF
# Refine idea: "The 3am moment"
Take the existing idea and develop it further with:
- Specific examples
- Concrete storytelling beats
- Visual suggestions
EOF

ralph run -c .ideation/preset.yml -p "Refine idea"
```

**Batch generation:**

```bash
# Multiple avatars
for avatar in myla techno_dad festival_sage; do
  cp .ideation/templates/${avatar}.yaml .ideation/input/avatar.yaml
  ralph run -c .ideation/preset.yml -p "Generate ideas"
  mv .ideation/output/ideas.yaml outputs/${avatar}-ideas-$(date +%s).yaml
done
```

**Quality filtering:**

```bash
# Extract only high-scoring ideas
yq '.ideas[] | select(.avg_score >= 8.0)' .ideation/output/ideas.yaml > top-ideas.yaml
```

### Directory-Based UX

Store reusable configs:

```
.ideation/
├── templates/            # Reusable templates (avatars + prompts)
│   ├── myla.yaml              # Avatar profiles
│   ├── techno-dad.yaml
│   ├── festival-sage.yaml
│   ├── avatar-schema.md       # Avatar schema docs
│   ├── trend-analysis.md      # Prompt templates
│   ├── seasonal-content.md
│   └── reaction-videos.md
│
└── archive/              # Historical outputs
    └── 2026-01-29-myla-late-night.yaml
```

**Usage:**

```bash
# Copy from library
cp .ideation/templates/avatar/myla.yaml .ideation/input/avatar.yaml
cp .ideation/templates/seasonal-content.md .ideation/input/prompt.md

# Edit prompt with specifics
$EDITOR .ideation/input/prompt.md

# Run
ralph run -c .ideation/preset.yml -p "Generate ideas"

# Archive
cp .ideation/output/ideas.yaml .ideation/archive/$(date +%s)-ideas.yaml
```

---

## 8. CLI Integration

Make it feel native:

```bash
# Wrapper script: bin/ideate
#!/bin/bash
set -e

case $1 in
  run)
    ralph run -c .ideation/preset.yml -p "${2:-Generate content ideas}"
    ;;

  show)
    yq -C '.ideas[] | select(.status == "passing")' .ideation/output/ideas.yaml | less
    ;;

  archive)
    timestamp=$(date +%s)
    cp .ideation/output/ideas.yaml .ideation/archive/${timestamp}-ideas.yaml
    echo "Archived to: .ideation/archive/${timestamp}-ideas.yaml"
    ;;

  clean)
    rm -f .ideation/output/ideas.yaml
    echo "Cleared output"
    ;;

  *)
    echo "Usage: ideate [run|show|archive|clean]"
    exit 1
    ;;
esac
```

---

## 9. Quality Gates (Backpressure)

Ralph principle: "Use gates that reject bad work"

### Option 1: LLM-as-Judge

Create a validation hat:

```yaml
validator:
  name: "Quality Validator"
  triggers: ["review.complete"]
  instructions: |
    For each passing idea, validate:
    1. Hook is < 10 words and creates curiosity
    2. Angle is specific, not generic
    3. Rationale explains WHY not just WHAT

    If validation fails, republish "ideate.create" with critique.
    Otherwise publish "validation.passed".
```

### Option 2: Structured Checks

```yaml
validator:
  instructions: |
    Run validation script:
    ```bash
    python scripts/validate_ideas.py .ideation/output/ideas.yaml
    ```

    If exit code != 0, iterate. Otherwise complete.
```

---

## 10. Memory Integration

Use Ralph's memory system to improve over time:

```yaml
# In hat instructions
ralph tools memory add "pattern: Hooks with questions score +1.2 higher" -t pattern
ralph tools memory add "pattern: Avoid 'late-night magic' - too generic" -t pattern
ralph tools memory add "decision: Myla avatar responds well to introspective angles" -t context
```

Memories persist across runs and get injected into context automatically.

---

## 11. End Conditions

Loop terminates when:

| Condition | Default | Configurable |
|-----------|---------|--------------|
| Min passing ideas | 3 ideas ≥ 7.0 avg | In completion_checker |
| Max iterations | 25 | `event_loop.max_iterations` |
| Max runtime | 1 hour | `event_loop.max_runtime_seconds` |
| User interrupt | Ctrl+C | - |

---

## 12. Extensibility

### Add New Creator Hats

```yaml
hats:
  data_analyst:
    name: "Data Analyst"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    instructions: |
      Generate ideas based on audience data and trends...
```

### Add New Input Types

```yaml
# .ideation/input/context.yaml
recent_videos:
  - url: "..."
    views: 120000
    top_comment: "..."

competitor_analysis:
  - creator: "..."
    successful_themes: [...]
```

Update planner to read `context.yaml` alongside avatar and prompt.

### Custom Scoring Models

```yaml
critic_reviewer:
  instructions: |
    Use custom scoring model:
    ```bash
    python scripts/score_idea.py --idea "$idea_json" --model gpt-4
    ```
```

---

## 13. MVP Acceptance Criteria

- [ ] Create `.ideation/input/` structure
- [ ] Create `presets/content-ideation.yml` with 6 hats
- [ ] Planner reads avatar.yaml + prompt.md
- [ ] Creators generate ideas (append to YAML)
- [ ] Reviewers score ideas
- [ ] Completion checker evaluates end conditions
- [ ] Output valid `ideas.yaml` with scores
- [ ] Works with Claude subscription (no API)

---

## 15. Success Metrics

**System works when:**
- User drops in avatar.yaml + prompt.md → gets scored ideas in < 5 minutes
- 70%+ of generated ideas are "passing" (avg ≥ 7.0)
- No manual intervention needed during loop
- Output is immediately usable (valid YAML, clear structure)

**System learns when:**
- Memories accumulate patterns about what works
- Subsequent runs with same avatar produce higher scores
- User feedback ("this idea worked great") gets captured as memory

---

## 16. References

- [Ralph Orchestrator Docs](https://mikeyobrien.github.io/ralph-orchestrator/)
- [Hat System Guide](https://mikeyobrien.github.io/ralph-orchestrator/concepts/hats-and-events/)
- [Presets Collection](https://mikeyobrien.github.io/ralph-orchestrator/guide/presets/)
- [Ralph Wiggum Technique](https://ghuntley.com/ralph/)
