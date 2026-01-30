# Image-Based Content Ideation

Generate video content ideas where each idea is paired with specific images.

## Quick Start

```bash
# 1. Setup
./.ideation/ideate setup myla late-night-techno

# 2. Prepare images directory
mkdir -p ./my-images
# Add your images (jpg, png, gif, webp)

# 3. Generate image-based ideas
./.ideation/ideate run-with-images ./my-images "Create late-night DJ content"

# 4. View results
./.ideation/ideate show
```

## How It Works

### 1. Image Analysis Phase

The **Image Analyzer** hat:
- Reads each image from the directory
- Analyzes visual content (subjects, composition, colors, mood)
- Identifies content themes each image could support
- Creates `.ideation/output/image-analysis.yaml`

Example analysis:
```yaml
images:
  - file: "./my-images/dj-hands.jpg"
    description: "Close-up of DJ hands on mixer, blue lighting, focused concentration"
    mood: [intimate, focused]
    themes:
      - "The technical craft behind the music"
      - "Moment-to-moment decision making"
    potential_formats: [monologue, story]
    duration_fit: "60-75 seconds for intimate pacing"
```

### 2. Idea Generation Phase

**Trend Spotter** and **Storyteller** hats:
- Generate ideas based on image themes
- Pair each idea with 1-3 relevant images
- Specify HOW images will be used in the video

Example idea:
```yaml
- id: "trend-001-1738234800"
  title: "The moment the crowd connects with the track"
  hook: "You can feel it - that split second when everyone just gets it."
  image_files:
    - "./my-images/crowd-hands.jpg"
    - "./my-images/mixer-closeup.jpg"
  image_usage: "crowd-hands as opening (0-15s), mixer-closeup during 'and then I drop it' moment (30-35s)"
```

### 3. Review Phase

**Audience & Brand Reviewer** scores (out of 10):
- Audience Appeal (3 points)
- Brand Authenticity (3 points)
- **Visual Coherence (4 points)** ← New for image-based ideas
  - Do images enhance the idea?
  - Is usage purposeful (not decorative)?
  - Does visual mood match content mood?

**Critic** also evaluates:
- Are images used creatively or predictably?
- Does visual execution elevate the idea?
- Calls out clichéd choices (sunset silhouettes, generic hands, etc.)

## Output Structure

Each passing idea includes:

```yaml
- id: "story-002-1738234900"
  title: "When the silence hits harder than the drop"
  hook: "I cut the music completely. Three seconds of nothing."
  angle: "Power of negative space in late-night sets"
  rationale: "Challenges 'more is more' mentality in electronic music"
  mood: reflective
  format: story
  duration_seconds: 75
  creator: "storyteller"

  # IMAGE-SPECIFIC FIELDS
  image_files:
    - "./my-images/empty-dancefloor.jpg"
    - "./my-images/dj-listening.jpg"
  image_usage: "empty-dancefloor during silence description (10-25s), dj-listening as 'reading the room' moment (40-45s)"

  reviews:
    - reviewer: "audience_brand"
      score: 8.5
      feedback: "Images perfectly capture the tension. Empty dancefloor is bold choice."
    - reviewer: "critic"
      score: 8.0
      feedback: "Fresh angle. Image usage is specific and purposeful, not generic."
  avg_score: 8.25
  status: "passing"
```

## Tips

### Image Selection

✅ **Do:**
- Mix wide shots and close-ups
- Capture specific moments (not generic stock)
- Include emotional/atmospheric images
- Show authentic behind-the-scenes

❌ **Avoid:**
- Generic stock photos (coffee cups, sunset silhouettes)
- Too many similar compositions
- Images that need heavy explanation
- Unrelated or purely decorative visuals

### Directory Organization

```bash
./content-images/
├── wide/           # Establishing shots
├── closeup/        # Detail/emotion shots
├── crowd/          # Audience connection
└── bts/            # Behind-the-scenes
```

You can use subdirectories - the script finds all images recursively.

### Image Usage Descriptions

Be specific about timing and purpose:

✅ **Good:**
- "crowd-hands as opening hook (0-5s), mixer during technical explanation (30-40s)"
- "empty-room establishes mood (0-10s), lights-on as reveal moment (45s)"

❌ **Vague:**
- "background visuals"
- "for visual interest"
- "general atmosphere"

## Use Cases

### Music/DJ Content
```bash
# Show your creative process
./.ideation/ideate run-with-images ./studio-footage "Track selection philosophy"
```

### Tutorial Content
```bash
# Visual demonstrations
./.ideation/ideate run-with-images ./technique-photos "Mixing techniques explained"
```

### Storytelling Content
```bash
# Personal journey narrative
./.ideation/ideate run-with-images ./career-photos "My path to DJing"
```

### Behind-the-Scenes
```bash
# Show the reality
./.ideation/ideate run-with-images ./bts-photos "What nobody tells you about touring"
```

## Continue With More Ideas

After the initial run, generate more ideas using the same images:

```bash
# Generate 3 more ideas
./.ideation/ideate continue

# Generate 5 more ideas with different angles
./.ideation/ideate continue 5
```

The system will:
- Use the same image analysis
- Avoid duplicate ideas
- Focus on unexplored angles
- Maintain the same quality threshold (avg ≥ 8.0)

## Archive Structure

Archives include image references:

```
.ideation/archive/20260130-102000-myla-theme/
├── all-ideas.yaml           # All generated ideas
├── passing-ideas.yaml       # Ideas with avg ≥ 8.0 (with image paths)
└── failing-ideas.yaml       # Ideas below threshold
```

**Note:** Archives store image paths, not the images themselves. Keep your source images directory intact for video production.
