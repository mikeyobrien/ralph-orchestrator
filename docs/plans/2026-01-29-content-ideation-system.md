# Content Ideation System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Configure Ralph orchestrator to generate video content ideas through multi-agent collaboration (Planner → 3 Creators → 3 Reviewers loop).

**Architecture:** Use Ralph's existing hat system (no code changes). Create preset config with 7 hats coordinating via events. Inputs via `.ideation/input/` directory, outputs to `.ideation/output/ideas.yaml`.

**Tech Stack:** Ralph orchestrator (Rust), YAML config, Claude CLI backend

---

## Task 1: Create Directory Structure

**Files:**
- Create: `.ideation/input/.gitkeep`
- Create: `.ideation/output/.gitkeep`
- Create: `.ideation/avatars/.gitkeep`
- Create: `.ideation/templates/.gitkeep`
- Create: `.ideation/archive/.gitkeep`
- Create: `.gitignore` updates

**Step 1: Create .ideation directories**

```bash
mkdir -p .ideation/input
mkdir -p .ideation/output
mkdir -p .ideation/avatars
mkdir -p .ideation/templates
mkdir -p .ideation/archive
```

**Step 2: Add .gitkeep files**

```bash
touch .ideation/input/.gitkeep
touch .ideation/output/.gitkeep
touch .ideation/avatars/.gitkeep
touch .ideation/templates/.gitkeep
touch .ideation/archive/.gitkeep
```

**Step 3: Update .gitignore**

Add to `.gitignore`:
```
# Content ideation runtime files
.ideation/input/avatar.yaml
.ideation/input/prompt.md
.ideation/output/ideas.yaml
.ideation/PROMPT.md
```

**Step 4: Verify structure**

Run: `tree .ideation -a`
Expected: Directory tree with all folders and .gitkeep files

**Step 5: Commit**

```bash
git add .ideation/ .gitignore
git commit -m "feat: add content ideation directory structure"
```

---

## Task 2: Create Example Avatar Profile

**Files:**
- Create: `.ideation/avatars/myla.yaml`
- Create: `.ideation/avatars/README.md`

**Step 1: Write example avatar**

Create `.ideation/avatars/myla.yaml`:
```yaml
name: Myla
personality: |
  Calm, reflective insider who's been in electronic music for years.
  Speaks with warmth and understanding. Never pretentious.
  Values authenticity over hype.
expertise:
  - electronic music curation
  - melodic techno
  - festival culture
  - DJ philosophy
audience_relationship: peer and guide
constraints: |
  Avoid:
  - Generic "best tracks" lists
  - Overly technical DJ tutorials
  - Name-dropping without context
  - Pretentious music theory
```

**Step 2: Write avatar README**

Create `.ideation/avatars/README.md`:
```markdown
# Avatar Library

Avatar profiles define the personality, expertise, and voice of content creators.

## Schema

```yaml
name: string                     # Avatar name
personality: string              # Voice, tone, character description
expertise: [string]              # Areas of knowledge
audience_relationship: string    # How avatar relates to audience
constraints: string              # Things to avoid (optional)
examples: [string]               # Reference content URLs (optional)
```

## Usage

```bash
cp .ideation/avatars/myla.yaml .ideation/input/avatar.yaml
```

## Available Avatars

- **myla.yaml** - Electronic music curator, melodic techno expert
```

**Step 3: Verify files exist**

Run: `ls -la .ideation/avatars/`
Expected: myla.yaml and README.md present

**Step 4: Commit**

```bash
git add .ideation/avatars/
git commit -m "feat: add example avatar profile (Myla)"
```

---

## Task 3: Create Example Prompt Templates

**Files:**
- Create: `.ideation/templates/late-night-techno.md`
- Create: `.ideation/templates/trend-analysis.md`
- Create: `.ideation/templates/README.md`

**Step 1: Write late-night techno template**

Create `.ideation/templates/late-night-techno.md`:
```markdown
# Late-night melodic techno

Create content ideas about the late-night dancefloor experience.

## Focus areas
- Emotional journey after midnight
- Track selection philosophy for 2-6am sets
- Connection with the crowd when energy shifts
- The role of silence and space in late sets

## Format preferences
- Short-form video (60-90 seconds)
- Authentic, intimate tone
- Personal anecdotes welcome

## Avoid
- Generic "top tracks" recommendations
- Technical mixing tutorials
- Festival hype content
```

**Step 2: Write trend analysis template**

Create `.ideation/templates/trend-analysis.md`:
```markdown
# Current Trends Analysis

Analyze what's happening right now in [DOMAIN] and create content ideas that tap into current conversations.

## Instructions
1. Replace [DOMAIN] with your specific area (e.g., "melodic techno scene")
2. Add 2-3 specific trends you've noticed
3. Add any constraints or angles to explore

## Trends to explore
- [Add trend 1]
- [Add trend 2]
- [Add trend 3]

## Angles
- What's misunderstood about this trend?
- Who's doing it differently?
- What does this say about the audience?

## Format preferences
- [Specify format: story, reaction, commentary, etc.]
```

**Step 3: Write templates README**

Create `.ideation/templates/README.md`:
```markdown
# Prompt Templates

Reusable prompt templates for common content ideation scenarios.

## Usage

```bash
cp .ideation/templates/late-night-techno.md .ideation/input/prompt.md
$EDITOR .ideation/input/prompt.md  # Customize
```

## Available Templates

- **late-night-techno.md** - Late-night dancefloor experience content
- **trend-analysis.md** - Current trends and cultural moments
```

**Step 4: Verify files**

Run: `ls -la .ideation/templates/`
Expected: 3 files present

**Step 5: Commit**

```bash
git add .ideation/templates/
git commit -m "feat: add prompt templates for content ideation"
```

---

## Task 4: Create Content Ideation Preset (Part 1: Config)

**Files:**
- Create: `presets/content-ideation.yml`

**Step 1: Write event loop configuration**

Create `presets/content-ideation.yml`:
```yaml
# Content Ideation Preset
#
# Multi-agent content ideation pipeline using Ralph's hat system.
# Planner → Creators (Trend/Story/Contrarian) → Reviewers (Audience/Brand/Critic)
#
# Usage:
#   1. cp .ideation/avatars/myla.yaml .ideation/input/avatar.yaml
#   2. cp .ideation/templates/late-night-techno.md .ideation/input/prompt.md
#   3. ralph run -c presets/content-ideation.yml -p "Generate content ideas"
#
# Output: .ideation/output/ideas.yaml

event_loop:
  prompt_file: ".ideation/PROMPT.md"
  completion_promise: "IDEATION_COMPLETE"
  max_iterations: 25
  max_runtime_seconds: 3600
  checkpoint_interval: 5
  starting_event: "ideate.start"

cli:
  backend: "claude"

core:
  specs_dir: "./specs/"

# Tasks track ideation workflow
tasks:
  enabled: true

# Memories accumulate what works across runs
memories:
  enabled: true
  inject: auto
  budget: 2000
```

**Step 2: Verify YAML is valid**

Run: `yq eval . presets/content-ideation.yml`
Expected: YAML parses without errors

**Step 3: Commit**

```bash
git add presets/content-ideation.yml
git commit -m "feat: add content-ideation preset config"
```

---

## Task 5: Create Content Ideation Preset (Part 2: Planner Hat)

**Files:**
- Modify: `presets/content-ideation.yml` (append hats section)

**Step 1: Add planner hat**

Append to `presets/content-ideation.yml`:
```yaml
hats:
  planner:
    name: "Content Planner"
    description: "Analyzes avatar and prompt to create ideation tasks for creators"
    triggers: ["ideate.start"]
    publishes: ["ideate.create"]
    default_publishes: "ideate.create"
    instructions: |
      ## PLANNER PHASE

      You are the content strategist. Your job is to understand the avatar's voice,
      the prompt's intent, and create specific, actionable ideation tasks.

      ### Process

      1. Read the avatar profile: `.ideation/input/avatar.yaml`
         - Note personality, expertise, constraints

      2. Read the prompt: `.ideation/input/prompt.md`
         - Note theme, focus areas, format preferences

      3. Search for relevant memories about this avatar or theme:
         ```bash
         ralph tools memory search "avatar_name OR theme_keyword"
         ```

      4. Create ideation tasks for each creator hat:
         ```bash
         ralph tools task add "Trend Spotter: Find culturally relevant angles for [theme]" -p 1
         ralph tools task add "Storyteller: Find personal narrative hooks for [theme]" -p 1
         ralph tools task add "Contrarian: Find unexpected perspectives on [theme]" -p 1
         ```

      5. Publish event to trigger creators:
         ```bash
         ralph emit "ideate.create" "created 3 ideation tasks for: [theme]"
         ```

      ### Don't
      - Do not generate ideas yourself (that's the creators' job)
      - Do not output the completion promise
      - Do not skip reading input files
```

**Step 2: Verify YAML still valid**

Run: `yq eval '.hats.planner.name' presets/content-ideation.yml`
Expected: "Content Planner"

**Step 3: Commit**

```bash
git add presets/content-ideation.yml
git commit -m "feat: add planner hat to content-ideation preset"
```

---

## Task 6: Create Content Ideation Preset (Part 3: Creator Hats)

**Files:**
- Modify: `presets/content-ideation.yml` (append creator hats)

**Step 1: Add trend_spotter hat**

Append to `presets/content-ideation.yml`:
```yaml
  trend_spotter:
    name: "Trend Spotter"
    description: "Generates ideas based on cultural relevance and current zeitgeist"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    default_publishes: "ideas.generated"
    instructions: |
      ## TREND SPOTTER PHASE

      You identify culturally relevant angles and capitalize on current conversations.

      ### Process

      1. Read avatar: `.ideation/input/avatar.yaml`
      2. Read prompt: `.ideation/input/prompt.md`
      3. Search memories for patterns about trends:
         ```bash
         ralph tools memory search "trend OR zeitgeist" --tags pattern
         ```

      4. Generate 2-3 content ideas based on:
         - What conversations are happening NOW?
         - What's the zeitgeist in this space?
         - What would stop someone mid-scroll?

      5. For EACH idea, create YAML structure:
         ```yaml
         ideas:
           - id: "trend-001-[timestamp]"
             title: "[Punchy, specific title]"
             hook: "[First 5 seconds - what grabs attention]"
             angle: "[The unique perspective or take]"
             rationale: "[Why this works right now]"
             mood: [calm/hype/reflective/intimate/provocative]
             format: [monologue/story/tip/reaction/question]
             duration_seconds: 75
             creator: "trend_spotter"
             reviews: []
             avg_score: 0.0
             status: "pending"
         ```

      6. Append ideas to `.ideation/output/ideas.yaml` (create file if missing)
         - If file doesn't exist, create header:
           ```yaml
           run_id: "[timestamp]"
           timestamp: "[ISO datetime]"
           avatar: "[from avatar.yaml]"
           theme: "[from prompt.md]"
           rounds: 1
           ideas: []
           ```
         - Then append your ideas to the `ideas` array

      7. Publish event:
         ```bash
         ralph emit "ideas.generated" "trend_spotter: generated 3 ideas"
         ```

      ### Quality Bar
      - Title: Under 10 words, specific not generic
      - Hook: Must create curiosity in 2 seconds
      - Angle: Must be unique to this moment in time
      - Rationale: Explain the cultural context
```

**Step 2: Add storyteller hat**

Append to `presets/content-ideation.yml`:
```yaml
  storyteller:
    name: "Storyteller"
    description: "Generates ideas based on narrative and emotional resonance"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    default_publishes: "ideas.generated"
    instructions: |
      ## STORYTELLER PHASE

      You find the personal narratives and emotional moments that create connection.

      ### Process

      1. Read avatar: `.ideation/input/avatar.yaml`
      2. Read prompt: `.ideation/input/prompt.md`
      3. Search memories for storytelling patterns:
         ```bash
         ralph tools memory search "storytelling OR narrative" --tags pattern
         ```

      4. Generate 2-3 content ideas based on:
         - What personal experiences resonate?
         - What moments create emotional connection?
         - What stories need to be told?

      5. Create ideas with same YAML structure as trend_spotter:
         - Use id prefix: "story-001-[timestamp]"
         - Set creator: "storyteller"
         - Focus on narrative arc and emotional beats

      6. Append to `.ideation/output/ideas.yaml`

      7. Publish event:
         ```bash
         ralph emit "ideas.generated" "storyteller: generated 3 ideas"
         ```

      ### Quality Bar
      - Title: Should evoke emotion or curiosity
      - Hook: Must establish personal connection immediately
      - Angle: Ground abstract concepts in lived experience
      - Rationale: Explain the emotional resonance
```

**Step 3: Add contrarian hat**

Append to `presets/content-ideation.yml`:
```yaml
  contrarian:
    name: "Contrarian"
    description: "Generates ideas based on unexpected takes and challenging assumptions"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    default_publishes: "ideas.generated"
    instructions: |
      ## CONTRARIAN PHASE

      You challenge conventional wisdom and find uncomfortable truths.

      ### Process

      1. Read avatar: `.ideation/input/avatar.yaml`
      2. Read prompt: `.ideation/input/prompt.md`
      3. Search memories for contrarian patterns:
         ```bash
         ralph tools memory search "contrarian OR unexpected" --tags pattern
         ```

      4. Generate 2-3 content ideas based on:
         - What does everyone get wrong?
         - What's the uncomfortable truth?
         - What assumption can we challenge?

      5. Create ideas with same YAML structure:
         - Use id prefix: "contra-001-[timestamp]"
         - Set creator: "contrarian"
         - Focus on provocative but authentic angles

      6. Append to `.ideation/output/ideas.yaml`

      7. Publish event:
         ```bash
         ralph emit "ideas.generated" "contrarian: generated 3 ideas"
         ```

      ### Quality Bar
      - Title: Should challenge expectations
      - Hook: Lead with the surprising take
      - Angle: Must be contrarian but not trolling
      - Rationale: Explain why the conventional view is incomplete
```

**Step 4: Verify YAML still valid**

Run: `yq eval '.hats | keys' presets/content-ideation.yml`
Expected: planner, trend_spotter, storyteller, contrarian

**Step 5: Commit**

```bash
git add presets/content-ideation.yml
git commit -m "feat: add creator hats (trend/story/contrarian) to preset"
```

---

## Task 7: Create Content Ideation Preset (Part 4: Reviewer Hats)

**Files:**
- Modify: `presets/content-ideation.yml` (append reviewer hats)

**Step 1: Add audience_reviewer hat**

Append to `presets/content-ideation.yml`:
```yaml
  audience_reviewer:
    name: "Audience Reviewer"
    description: "Scores ideas from the target audience perspective"
    triggers: ["ideas.generated"]
    publishes: ["review.audience"]
    default_publishes: "review.audience"
    instructions: |
      ## AUDIENCE REVIEWER PHASE

      You are the target audience. Score ideas on immediate appeal.

      ### Process

      1. Read avatar: `.ideation/input/avatar.yaml`
      2. Read all ideas: `.ideation/output/ideas.yaml`
      3. For EACH idea, score 1-10 based on:
         - Would I stop scrolling for this? (3 points)
         - Does the hook grab me in 2 seconds? (4 points)
         - Is this relevant to me right now? (3 points)

      4. Update EACH idea's reviews array:
         ```yaml
         reviews:
           - reviewer: "audience"
             score: 7.5
             feedback: "Hook is strong but angle feels generic. Need more specificity."
         ```

      5. Write updated YAML back to `.ideation/output/ideas.yaml`

      6. Publish event with count:
         ```bash
         ralph emit "review.audience" "audience: reviewed [N] ideas"
         ```

      ### Scoring Guide
      - 8-10: Would definitely watch, share-worthy
      - 6-7: Interesting but not compelling
      - 4-5: Meh, keep scrolling
      - 1-3: Skip or cringe
```

**Step 2: Add brand_reviewer hat**

Append to `presets/content-ideation.yml`:
```yaml
  brand_reviewer:
    name: "Brand Guardian"
    description: "Scores ideas on brand authenticity and voice alignment"
    triggers: ["review.audience"]
    publishes: ["review.brand"]
    default_publishes: "review.brand"
    instructions: |
      ## BRAND GUARDIAN PHASE

      You protect the avatar's authentic voice and brand integrity.

      ### Process

      1. Read avatar: `.ideation/input/avatar.yaml`
         - Deep understanding of personality, expertise, constraints
      2. Read all ideas: `.ideation/output/ideas.yaml`
      3. For EACH idea, score 1-10 based on:
         - Does this fit the avatar's voice? (4 points)
         - Is this authentic to the persona? (3 points)
         - Would the avatar actually say this? (3 points)

      4. Update EACH idea's reviews array:
         ```yaml
         reviews:
           - reviewer: "brand"
             score: 6.0
             feedback: "Angle is good but tone is too hype for Myla's calm style."
         ```

      5. Write updated YAML back to `.ideation/output/ideas.yaml`

      6. Publish event:
         ```bash
         ralph emit "review.brand" "brand: reviewed [N] ideas"
         ```

      ### Watch For
      - Constraints violations (avatar.yaml)
      - Tone mismatches (too hype / too technical / too generic)
      - Expertise overreach (claiming knowledge outside domain)
```

**Step 3: Add critic_reviewer hat**

Append to `presets/content-ideation.yml`:
```yaml
  critic_reviewer:
    name: "Critical Reviewer"
    description: "Scores ideas with skepticism, finds weaknesses and clichés"
    triggers: ["review.brand"]
    publishes: ["review.complete"]
    default_publishes: "review.complete"
    instructions: |
      ## CRITICAL REVIEWER PHASE

      You are the harsh but fair critic. Find what's weak, derivative, or cliché.

      ### Process

      1. Read all ideas: `.ideation/output/ideas.yaml`
      2. For EACH idea, score 1-10 based on:
         - Is this original or derivative? (4 points)
         - Is this cringe or compelling? (3 points)
         - Would this actually work? (3 points)

      3. Update EACH idea's reviews array:
         ```yaml
         reviews:
           - reviewer: "critic"
             score: 5.5
             feedback: "Hook is cliché ('late-night magic'). Angle has been done before. Needs fresh take."
         ```

      4. Calculate average score for EACH idea:
         - avg_score = (audience + brand + critic) / 3
         - Round to 1 decimal place

      5. Set status for EACH idea:
         - status: "passing" if avg_score >= 7.0
         - status: "failing" if avg_score < 7.0

      6. Write updated YAML back to `.ideation/output/ideas.yaml`

      7. Count passing ideas and publish event:
         ```bash
         ralph emit "review.complete" "critic: reviewed [N] ideas, [M] passing (avg >= 7.0)"
         ```

      ### Be Ruthless
      - Call out generic language ("magic", "journey", "elevate")
      - Flag derivative angles (seen 100 times before)
      - Question vague rationales (explain specifics)
```

**Step 4: Verify YAML still valid**

Run: `yq eval '.hats | keys' presets/content-ideation.yml`
Expected: 7 hats total

**Step 5: Commit**

```bash
git add presets/content-ideation.yml
git commit -m "feat: add reviewer hats (audience/brand/critic) to preset"
```

---

## Task 8: Create Content Ideation Preset (Part 5: Completion Checker)

**Files:**
- Modify: `presets/content-ideation.yml` (append completion_checker hat)

**Step 1: Add completion_checker hat**

Append to `presets/content-ideation.yml`:
```yaml
  completion_checker:
    name: "Completion Checker"
    description: "Evaluates end conditions and decides to iterate or complete"
    triggers: ["review.complete"]
    publishes: []
    instructions: |
      ## COMPLETION CHECKER PHASE

      You decide whether we have enough quality ideas or need another round.

      ### Process

      1. Read all ideas: `.ideation/output/ideas.yaml`

      2. Count passing ideas:
         ```bash
         passing_count=$(yq eval '.ideas[] | select(.status == "passing") | .id' .ideation/output/ideas.yaml | wc -l)
         echo "Passing ideas: $passing_count"
         ```

      3. Check end conditions in order:

      **Condition 1: Quality threshold met**
      - If >= 3 ideas with status "passing" (avg_score >= 7.0):
        - Update rounds counter in ideas.yaml
        - Close all tasks: `ralph tools task list` and close each
        - Output completion promise: `IDEATION_COMPLETE`
        - DONE

      **Condition 2: Max rounds reached**
      - If rounds >= 5 in ideas.yaml:
        - Output completion promise: `IDEATION_COMPLETE`
        - DONE

      **Condition 3: Iterate**
      - Otherwise:
        - Increment rounds in ideas.yaml
        - Analyze why ideas failed (check review feedback)
        - Create memory with patterns:
          ```bash
          ralph tools memory add "pattern: [specific pattern from failed ideas]" -t pattern
          ```
        - Publish event to restart:
          ```bash
          ralph emit "ideate.create" "iterate: round [N], need [M] more passing ideas"
          ```

      ### Don't
      - Do not iterate more than 5 rounds total
      - Do not complete without closing tasks
      - Do not skip pattern analysis
```

**Step 2: Verify complete preset**

Run: `yq eval '.hats | keys' presets/content-ideation.yml`
Expected: 8 hats (planner, 3 creators, 3 reviewers, completion)

**Step 3: Verify event flow**

Run: `yq eval '.hats | to_entries | .[] | [.value.name, .value.triggers, .value.publishes]' presets/content-ideation.yml`
Expected: Event chain visible

**Step 4: Commit**

```bash
git add presets/content-ideation.yml
git commit -m "feat: add completion checker to content-ideation preset"
```

---

## Task 9: Create CLI Wrapper Script

**Files:**
- Create: `bin/ideate`
- Modify: `.gitignore`

**Step 1: Create bin directory**

```bash
mkdir -p bin
```

**Step 2: Write ideate wrapper script**

Create `bin/ideate`:
```bash
#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

case $1 in
  run)
    shift
    prompt="${1:-Generate content ideas}"

    # Verify inputs exist
    if [[ ! -f .ideation/input/avatar.yaml ]]; then
      echo "Error: .ideation/input/avatar.yaml not found"
      echo "Copy an avatar: cp .ideation/avatars/myla.yaml .ideation/input/avatar.yaml"
      exit 1
    fi

    if [[ ! -f .ideation/input/prompt.md ]]; then
      echo "Error: .ideation/input/prompt.md not found"
      echo "Copy a template: cp .ideation/templates/late-night-techno.md .ideation/input/prompt.md"
      exit 1
    fi

    # Run Ralph with content-ideation preset
    ralph run -c presets/content-ideation.yml -p "$prompt"
    ;;

  show)
    if [[ ! -f .ideation/output/ideas.yaml ]]; then
      echo "No ideas generated yet. Run: ideate run"
      exit 1
    fi

    # Show passing ideas with color
    echo "=== Passing Ideas (avg >= 7.0) ==="
    yq eval '.ideas[] | select(.status == "passing") |
      "---\nTitle: " + .title +
      "\nHook: " + .hook +
      "\nScore: " + (.avg_score | tostring) +
      "\nCreator: " + .creator +
      "\n"' .ideation/output/ideas.yaml
    ;;

  stats)
    if [[ ! -f .ideation/output/ideas.yaml ]]; then
      echo "No ideas generated yet. Run: ideate run"
      exit 1
    fi

    total=$(yq eval '.ideas | length' .ideation/output/ideas.yaml)
    passing=$(yq eval '.ideas[] | select(.status == "passing") | .id' .ideation/output/ideas.yaml | wc -l)
    failing=$((total - passing))
    rounds=$(yq eval '.rounds' .ideation/output/ideas.yaml)

    echo "=== Ideation Stats ==="
    echo "Rounds: $rounds"
    echo "Total ideas: $total"
    echo "Passing (≥7.0): $passing"
    echo "Failing (<7.0): $failing"
    ;;

  archive)
    if [[ ! -f .ideation/output/ideas.yaml ]]; then
      echo "No ideas to archive"
      exit 1
    fi

    timestamp=$(date +%Y%m%d-%H%M%S)
    avatar=$(yq eval '.avatar' .ideation/output/ideas.yaml)
    theme=$(yq eval '.theme' .ideation/output/ideas.yaml | tr ' ' '-' | tr '[:upper:]' '[:lower:]')

    archive_name="${timestamp}-${avatar}-${theme}.yaml"
    cp .ideation/output/ideas.yaml ".ideation/archive/${archive_name}"
    echo "Archived to: .ideation/archive/${archive_name}"
    ;;

  clean)
    rm -f .ideation/output/ideas.yaml
    rm -f .ideation/PROMPT.md
    echo "Cleared output files"
    ;;

  setup)
    avatar="${2:-myla}"
    template="${3:-late-night-techno}"

    if [[ ! -f ".ideation/avatars/${avatar}.yaml" ]]; then
      echo "Error: Avatar not found: .ideation/avatars/${avatar}.yaml"
      exit 1
    fi

    if [[ ! -f ".ideation/templates/${template}.md" ]]; then
      echo "Error: Template not found: .ideation/templates/${template}.md"
      exit 1
    fi

    cp ".ideation/avatars/${avatar}.yaml" .ideation/input/avatar.yaml
    cp ".ideation/templates/${template}.md" .ideation/input/prompt.md

    echo "Setup complete:"
    echo "  Avatar: ${avatar}"
    echo "  Template: ${template}"
    echo ""
    echo "Edit prompt: \$EDITOR .ideation/input/prompt.md"
    echo "Run ideation: ideate run"
    ;;

  *)
    echo "Usage: ideate [command]"
    echo ""
    echo "Commands:"
    echo "  setup [avatar] [template]  - Copy avatar and template to input/ (default: myla late-night-techno)"
    echo "  run [prompt]               - Run ideation loop (default prompt: 'Generate content ideas')"
    echo "  show                       - Show passing ideas"
    echo "  stats                      - Show ideation statistics"
    echo "  archive                    - Archive current ideas with timestamp"
    echo "  clean                      - Clear output files"
    echo ""
    echo "Examples:"
    echo "  ideate setup myla late-night-techno"
    echo "  ideate run"
    echo "  ideate show"
    echo "  ideate archive"
    exit 1
    ;;
esac
```

**Step 3: Make script executable**

```bash
chmod +x bin/ideate
```

**Step 4: Test help command**

Run: `./bin/ideate`
Expected: Usage message with all commands

**Step 5: Update .gitignore**

Add to `.gitignore`:
```
# Don't ignore bin/ directory or scripts
!bin/
!bin/*
```

**Step 6: Commit**

```bash
git add bin/ideate .gitignore
git commit -m "feat: add ideate CLI wrapper script"
```

---

## Task 10: Create README for Content Ideation

**Files:**
- Create: `.ideation/README.md`

**Step 1: Write README**

Create `.ideation/README.md`:
```markdown
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

**Multi-Agent Loop:**
1. **Planner** - Analyzes avatar + prompt, creates tasks
2. **Creators** - Generate ideas (Trend Spotter, Storyteller, Contrarian)
3. **Reviewers** - Score ideas (Audience, Brand, Critic)
4. **Checker** - Iterate or complete (need 3+ ideas with avg ≥ 7.0)

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
```

**Step 2: Verify README renders**

Run: `cat .ideation/README.md`
Expected: Full markdown content displays

**Step 3: Commit**

```bash
git add .ideation/README.md
git commit -m "docs: add content ideation system README"
```

---

## Task 11: Create End-to-End Test Setup

**Files:**
- Create: `.ideation/input/avatar.yaml` (test input)
- Create: `.ideation/input/prompt.md` (test input)

**Step 1: Copy test avatar**

```bash
cp .ideation/avatars/myla.yaml .ideation/input/avatar.yaml
```

**Step 2: Copy test prompt**

```bash
cp .ideation/templates/late-night-techno.md .ideation/input/prompt.md
```

**Step 3: Verify inputs**

Run: `./bin/ideate setup myla late-night-techno`
Expected: "Setup complete" message

**Step 4: Don't commit inputs (gitignored)**

Run: `git status`
Expected: .ideation/input/ files NOT staged (gitignored)

**Step 5: Document test in commit message**

```bash
git status  # Should show no changes since inputs are gitignored
echo "Test inputs created in .ideation/input/ (gitignored)"
```

---

## Task 12: Update Main README

**Files:**
- Modify: `README.md`

**Step 1: Add content ideation section**

Add after Quick Start section in `README.md`:

```markdown
## Content Ideation System

This repository includes a content ideation system built on Ralph's hat system.

**Quick Start:**

```bash
# Setup inputs
./bin/ideate setup myla late-night-techno

# Edit prompt
$EDITOR .ideation/input/prompt.md

# Run ideation
./bin/ideate run

# View results
./bin/ideate show
```

**See:** [.ideation/README.md](.ideation/README.md) for full documentation.

**Configuration:** [specs/constitution.md](specs/constitution.md)
```

**Step 2: Verify README still valid**

Run: `head -20 README.md`
Expected: Original content preserved

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: add content ideation section to main README"
```

---

## Task 13: Validation and Testing

**Files:**
- No file changes, pure validation

**Step 1: Validate preset YAML**

```bash
yq eval . presets/content-ideation.yml > /dev/null
echo "YAML validation: $?"
```

Expected: Exit code 0

**Step 2: Verify directory structure**

```bash
tree .ideation -L 2
```

Expected:
```
.ideation/
├── avatars/
│   ├── myla.yaml
│   └── README.md
├── templates/
│   ├── late-night-techno.md
│   ├── trend-analysis.md
│   └── README.md
├── input/
│   ├── avatar.yaml
│   └── prompt.md
├── output/
│   └── .gitkeep
├── archive/
│   └── .gitkeep
└── README.md
```

**Step 3: Test CLI wrapper commands**

```bash
./bin/ideate                    # Should show help
./bin/ideate setup myla late-night-techno  # Should succeed
./bin/ideate show 2>&1 | grep "No ideas"   # Should warn (no run yet)
./bin/ideate stats 2>&1 | grep "No ideas"  # Should warn (no run yet)
```

Expected: All commands work, appropriate messages

**Step 4: Verify hat event flow**

```bash
echo "=== Event Flow Validation ==="
echo "Planner: ideate.start -> ideate.create"
echo "Creators: ideate.create -> ideas.generated"
echo "Audience: ideas.generated -> review.audience"
echo "Brand: review.audience -> review.brand"
echo "Critic: review.brand -> review.complete"
echo "Checker: review.complete -> [complete or iterate]"

# Extract from preset
yq eval '.hats | to_entries | .[] | .key + ": " + (.value.triggers | join(",")) + " -> " + (.value.publishes | join(","))' presets/content-ideation.yml
```

Expected: Event chain matches expected flow

**Step 5: Document validation complete**

```bash
echo "✅ All validations passed"
```

---

## Task 14: Create Contribution Guide for Content Ideation

**Files:**
- Create: `.ideation/CONTRIBUTING.md`

**Step 1: Write contribution guide**

Create `.ideation/CONTRIBUTING.md`:
```markdown
# Contributing to Content Ideation

## Adding New Avatars

1. Create avatar file: `.ideation/avatars/your-avatar.yaml`
2. Follow schema in `avatars/README.md`
3. Test with: `./bin/ideate setup your-avatar late-night-techno`
4. Commit: `git add .ideation/avatars/your-avatar.yaml`

## Adding New Templates

1. Create template: `.ideation/templates/your-template.md`
2. Include theme, focus areas, constraints
3. Test with: `./bin/ideate setup myla your-template`
4. Update `templates/README.md` with template description
5. Commit both files

## Modifying the Preset

Edit: `presets/content-ideation.yml`

**Adding a new creator hat:**
```yaml
hats:
  your_creator:
    name: "Your Creator Name"
    triggers: ["ideate.create"]
    publishes: ["ideas.generated"]
    instructions: |
      [Your instructions]
```

**Adding a new reviewer hat:**
- Insert between existing reviewers and completion_checker
- Trigger on previous reviewer's event
- Publish next event in chain

**Modifying scoring:**
- Edit reviewer hat instructions
- Adjust scoring criteria (1-10 scale)
- Update completion threshold in completion_checker

## Testing Changes

```bash
# Validate YAML
yq eval . presets/content-ideation.yml

# Test with example inputs
./bin/ideate setup myla late-night-techno
./bin/ideate run

# Check diagnostics
RALPH_DIAGNOSTICS=1 ./bin/ideate run
```

## Sharing Patterns

Found a pattern that improves ideas?

```bash
ralph tools memory add "pattern: [your pattern]" -t pattern --tags ideation
```

Share via PR to update preset instructions.
```

**Step 2: Commit**

```bash
git add .ideation/CONTRIBUTING.md
git commit -m "docs: add contribution guide for content ideation"
```

---

## Final Verification

All tasks complete. Verify full system:

```bash
# 1. Structure exists
tree .ideation -L 2

# 2. Preset is valid
yq eval . presets/content-ideation.yml > /dev/null && echo "✅ Preset valid"

# 3. CLI works
./bin/ideate && echo "✅ CLI works"

# 4. Git status clean (except test inputs)
git status

# 5. Ready for execution
echo "✅ Content Ideation System ready"
echo ""
echo "Next: Run './bin/ideate setup myla late-night-techno && ./bin/ideate run' to test"
```

---

## Execution Notes

**What we built:**
- 7 hat configuration preset (no Rust code)
- Directory structure for inputs/outputs
- Reusable avatar and template libraries
- CLI wrapper for ergonomic UX
- Complete documentation

**What we didn't build:**
- No Rust code changes (uses existing Ralph)
- No API integration (uses existing Claude CLI)
- No custom file formats (standard YAML)
- No complex workflows (Ralph's event system)

**Testing the system:**
After implementation, run:
```bash
./bin/ideate setup myla late-night-techno
./bin/ideate run
./bin/ideate show
```

This validates the complete pipeline end-to-end.
