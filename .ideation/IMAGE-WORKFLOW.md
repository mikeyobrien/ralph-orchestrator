# Image-Based Content Ideation

Generate VIDEO SCRIPTS where images are used to create the video content.

**IMPORTANT:** This generates VIDEO SCRIPTS, not photo captions. The images will be used to GENERATE VIDEOS. You're creating narration/scripts that will be spoken OVER the video visuals.

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
- Reads each image from the directory (these will become VIDEO CONTENT)
- Analyzes visual content (subjects, composition, colors, mood)
- Identifies what VIDEO SCRIPTS could work with these visuals
- Creates `.ideation/output/image-analysis.yaml`

Example analysis:
```yaml
images:
  - file: "./my-images/dj-hands.jpg"
    description: "Close-up of DJ hands on mixer, blue lighting, focused concentration"
    mood: [intimate, focused]
    video_script_themes:
      - "VIDEO SCRIPT: Narrate the technical decisions happening in real-time"
      - "VIDEO SCRIPT: Tell a story about learning to trust your instincts"
    potential_formats: [monologue, story]
    duration_fit: "60-75 seconds for intimate pacing"
    note: "This will be video content - what would someone SAY while this plays?"
```

### 2. Video Script Generation Phase

**Trend Spotter** and **Storyteller** hats:
- Write VIDEO SCRIPTS (what the creator will say/narrate)
- Pair each script with 1-3 images that will play as video content
- Specify WHEN each image appears during the narration
- **Important:** Scripts are what they SAY, not what the images show

Example video script:
```yaml
- id: "trend-001-1738234800"
  title: "The moment the crowd connects with the track"
  hook: "You can feel it - that split second when everyone just gets it."
  angle: "Capturing the invisible connection between DJ and crowd"
  # ^ This is what the creator SAYS in the video
  image_files:
    - "./my-images/crowd-hands.jpg"
    - "./my-images/mixer-closeup.jpg"
  image_usage: "crowd-hands plays while I talk about the feeling (0-15s). Mixer-closeup plays during 'and then I drop it' line (30-35s). I'm NARRATING over these visuals."
  # ^ The script works as voiceover/narration, images are the video content
```

### 3. Video Script Review Phase

**Audience & Brand Reviewer** scores VIDEO SCRIPTS (out of 10):
- Audience Appeal (3 points) - Would this NARRATION stop scrolling?
- Brand Authenticity (3 points) - Would the avatar actually SAY this?
- **Video Visual Coherence (4 points)** ← Evaluates script + visuals
  - Do visuals enhance the SPOKEN narrative?
  - Is timing purposeful (not just "background")?
  - Does the script work WITH visuals (not describe them)?

**Critic** ruthlessly evaluates:
- **INSTANT FAIL:** Script describes the photos instead of telling a story
- **INSTANT FAIL:** Vague timing like "plays as background footage"
- Does the script work as voiceover/narration over video?
- Could this be a TikTok/Reel voiceover? (Good sign)
- Is this just a stretched photo caption? (Bad sign)
- Calls out clichéd visuals and lazy "look at this" scripts

## Output Structure

Each passing VIDEO SCRIPT includes:

### ✅ GOOD Example - Video Script with Visuals

```yaml
- id: "story-002-1738234900"
  title: "When the silence hits harder than the drop"
  hook: "I cut the music completely. Three seconds of nothing."
  # ^ What the creator SAYS to camera / in voiceover
  angle: "Power of negative space in late-night sets"
  rationale: "Challenges 'more is more' mentality in electronic music"
  mood: reflective
  format: story
  duration_seconds: 75
  creator: "storyteller"

  # VIDEO VISUAL FIELDS
  image_files:
    - "./my-images/empty-dancefloor.jpg"
    - "./my-images/dj-listening.jpg"
  image_usage: "empty-dancefloor plays while I narrate the silence moment (10-25s). dj-listening plays during 'you have to read the room' line (40-45s). I'm telling the STORY over these visuals."
  # ^ Specific timing, script works as narration OVER video

  reviews:
    - reviewer: "audience_brand"
      score: 8.5
      feedback: "Strong spoken hook. Visuals support the narrative without being described. Works as voiceover script."
    - reviewer: "critic"
      score: 8.0
      feedback: "Fresh narrative. Script tells a story OVER the visuals, doesn't describe them. Timing is purposeful."
  avg_score: 8.25
  status: "passing"
```

### ❌ BAD Example - Photo Caption, Not Video Script

```yaml
- id: "story-003-1738234901"
  title: "Look at this amazing dancefloor"
  hook: "Check out this incredible moment I captured."
  # ^ Describes the photo, not a standalone script
  angle: "Beautiful dancefloor photography"
  image_files:
    - "./my-images/empty-dancefloor.jpg"
  image_usage: "Shows the dancefloor as background"
  # ^ Vague, no timing, treats image as decoration

  reviews:
    - reviewer: "audience_brand"
      score: 4.0
      feedback: "Script just describes the photo. 'Check out' and 'look at' means they're pointing at the image, not telling a story."
    - reviewer: "critic"
      score: 3.0
      feedback: "INSTANT FAIL. This is a photo caption, not a video script. Vague timing ('background'). What would they actually SAY for 75 seconds? This dies after 5 seconds."
  avg_score: 3.5
  status: "failing"
```

## Tips

### Video Script vs Photo Content

**Remember: You're writing VIDEO SCRIPTS, not photo captions.**

✅ **Good Video Script:**
- "I cut the music. Three seconds of total silence." ← What you SAY
- Images play WHILE you narrate
- Script makes sense as audio-only
- Could work as podcast/voiceover
- Specific visual timing (not "background")

❌ **Bad Photo Caption:**
- "Look at this moment" ← Describes the photo
- "Check out this view" ← Points at the image
- "As you can see here..." ← References the visual
- Vague timing like "plays throughout"
- Wouldn't work as audio-only

### Test Your Script
- Cover the images. Does the script still work? ✅ Good
- Remove the audio. Does it make sense? ❌ Bad (should need the narration)
- Could this be a TikTok voiceover? ✅ Good
- Is this just a stretched Instagram caption? ❌ Bad

### Image Selection

✅ **Do:**
- Mix wide shots and close-ups
- Capture specific moments (not generic stock)
- Include emotional/atmospheric images
- Show authentic behind-the-scenes
- Think: "What would I TALK ABOUT while this plays?"

❌ **Avoid:**
- Generic stock photos (coffee cups, sunset silhouettes)
- Too many similar compositions
- Images that need heavy explanation in the script
- Unrelated or purely decorative visuals
- Images you'd have to describe or point to

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

Be specific about timing and HOW the narration works with the visual:

✅ **Good - Script Over Visuals:**
- "crowd-hands plays while I talk about the energy shift (0-15s). Mixer plays during my 'here's the trick' line (30-40s). I'm NARRATING over these."
- "empty-room plays as I describe the silence (0-10s). Lights-on during 'and then boom' moment (45s). Visuals support my STORY."
- "dj-hands while I explain the technique (20-35s). My voiceover does the teaching, visuals show the action."

❌ **Vague - Treats Images as Decoration:**
- "background visuals" ← What are you SAYING while they play?
- "for visual interest" ← Lazy, no timing, no narrative connection
- "general atmosphere" ← How does your script work with this?
- "shows the scene" ← What are you NARRATING?

### Script + Visual Sync Examples

**Story Format:**
```
Script (what I say): "I remember my first silent drop. Heart racing, palms sweating..."
Visuals: dj-hands-nervous.jpg (0-12s) plays WHILE I tell this story
NOT: "As you can see in this photo, my hands are sweating"
```

**Tip Format:**
```
Script (what I say): "The trick is reading the room, not the waveform."
Visuals: crowd-watching.jpg (15-25s) plays WHILE I explain this concept
NOT: "Look at this crowd - can you see how..."
```

**Monologue Format:**
```
Script (what I say): "Everyone thinks louder equals better. It doesn't."
Visuals: mixer-levels.jpg (0-8s) plays during this opening statement
NOT: "Here's a photo of my mixer showing the levels..."
```

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
