# ElevenLabs Integration: Claude vs GenFM Comparison

**Date**: 2026-01-03
**Purpose**: Compare two approaches for Code Story audio generation

## Executive Summary

Two distinct approaches for generating audio narratives from code repositories:

| Aspect | Approach A: Claude + ElevenLabs | Approach B: ElevenLabs GenFM/Studio |
|--------|-------------------------------|-------------------------------------|
| Script Generation | Claude Opus 4.5 agents | ElevenLabs GenFM AI |
| Voice Synthesis | ElevenLabs TTS/Dialogue API | ElevenLabs Studio API |
| Control | Full control over narrative | Limited to GenFM parameters |
| Customization | Highly customizable | Preset formats (conversation/bulletin) |
| Cost | Claude API + ElevenLabs credits | ElevenLabs credits only (LLM free) |
| Complexity | Higher (multi-agent pipeline) | Lower (single API call) |

## Approach A: Claude + ElevenLabs TTS

### Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Intent Agent   │───►│ Repo Analyzer   │───►│ Story Architect │───►│ Voice Director  │
│  (Claude Opus)  │    │  (Claude Opus)  │    │  (Claude Opus)  │    │  (ElevenLabs)   │
└─────────────────┘    └─────────────────┘    └─────────────────┘    └─────────────────┘
      │                       │                       │                       │
      ▼                       ▼                       ▼                       ▼
   User Intent           Code Analysis          Script JSON            Audio Files
   + Story Plan          + Dependencies         + Voice Tags           (MP3/WAV)
```

### Script Generation (Claude)

```python
# Story Architect Agent generates structured script
script = {
    "chapters": [
        {
            "title": "Architecture Overview",
            "segments": [
                {
                    "speaker": "host",
                    "voice_id": "host_voice_id",
                    "text": "[excited] Welcome to the code story for ralph-orchestrator!",
                    "emotion": "excited"
                },
                {
                    "speaker": "analyst",
                    "voice_id": "analyst_voice_id",
                    "text": "[thoughtful] This is a fascinating AI orchestration system...",
                    "emotion": "thoughtful"
                }
            ]
        }
    ]
}
```

### Voice Synthesis (ElevenLabs)

```python
from elevenlabs import ElevenLabs

client = ElevenLabs(api_key=os.environ["ELEVENLABS_API_KEY"])

# Option 1: Text to Dialogue API (multi-speaker, v3 only)
audio = client.text_to_dialogue.convert(
    model_id="eleven_v3",
    inputs=[
        {"text": segment["text"], "voice_id": segment["voice_id"]}
        for segment in chapter["segments"]
    ],
    settings={"stability": 0.4, "similarity_boost": 0.75}
)

# Option 2: Text to Speech API (per-segment, any model)
for segment in chapter["segments"]:
    audio = client.text_to_speech.convert(
        voice_id=segment["voice_id"],
        model_id="eleven_multilingual_v2",
        text=segment["text"],
        voice_settings={"stability": 0.5, "similarity_boost": 0.8}
    )
```

### Pros
- **Full narrative control** - Custom story structures, personas, pacing
- **Deep code understanding** - Claude analyzes architecture, patterns, dependencies
- **Custom voice direction** - Precise emotion tags per segment
- **Tailored to user intent** - Onboarding, review, debugging focus
- **Reproducible** - Same input = same script (seed parameter)

### Cons
- **Higher complexity** - 4-agent pipeline to maintain
- **Dual API costs** - Claude + ElevenLabs credits
- **Longer generation time** - Sequential agent processing
- **More integration code** - Script format mapping

---

## Approach B: ElevenLabs GenFM/Studio API

### Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Source Content │───►│  GenFM/Studio   │───►│   Audio Export  │
│  (URL/Text/Doc) │    │   (ElevenLabs)  │    │    (MP3/WAV)    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                              │
                              ▼
                       Script Generation
                       + Voice Synthesis
                       (Single API Call)
```

### Podcast Generation (GenFM)

```python
from elevenlabs import ElevenLabs

client = ElevenLabs(api_key=os.environ["ELEVENLABS_API_KEY"])

# Create podcast from GitHub repository
podcast = client.studio.podcasts.create(
    model_id="eleven_v3",
    mode={
        "type": "conversation",  # Two hosts discussing
        "host_voice_id": "JBFqnCBsd6RMkjVDRZzb",
        "guest_voice_id": "pNInz6obpgDQGcFmaJgB"
    },
    source={
        "type": "url",
        "url": "https://github.com/mikeyobrien/ralph-orchestrator"
    },
    duration_scale="default",  # short (<3min), default (3-7min), long (>7min)
    quality_preset="high",
    language="en",
    instructions_prompt="Focus on the AI orchestration architecture and validation system",
    highlights=["Multi-agent pipeline", "Validation gates", "Claude integration"],
    intro="Welcome to Code Story, exploring codebases through audio.",
    outro="Thanks for listening to this code exploration."
)
```

### Studio Project (Audiobook Style)

```python
# Create audiobook-style project from content
project = client.studio.projects.create(
    name="Ralph Orchestrator Code Story",
    default_model_id="eleven_v3",
    default_paragraph_voice_id="narrator_voice_id",
    quality_preset="high",
    from_url="https://github.com/mikeyobrien/ralph-orchestrator",
    auto_convert=True,
    auto_assign_voices=True,  # Auto-detect speakers
    volume_normalization=True,
    callback_url="https://yourapp.com/webhook/elevenlabs"
)
```

### Pros
- **Single API call** - Simpler integration
- **Lower cost** - LLM costs covered by ElevenLabs
- **Faster time-to-audio** - No multi-agent overhead
- **Built-in features** - Music, SFX, captions, video track
- **Auto voice assignment** - Detects characters automatically

### Cons
- **Limited narrative control** - Preset formats only
- **Generic code understanding** - Not specialized for code repos
- **Fixed personas** - Host/guest or single narrator only
- **Less customizable** - Duration scale, highlights only
- **No intent-driven focus** - Can't tailor to onboarding vs debugging

---

## Feature Comparison

| Feature | Claude + ElevenLabs | GenFM/Studio |
|---------|--------------------:|-------------:|
| Script customization | ⭐⭐⭐⭐⭐ | ⭐⭐ |
| Code understanding | ⭐⭐⭐⭐⭐ | ⭐⭐ |
| Voice control | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| Integration simplicity | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| Cost efficiency | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| Time to audio | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Multi-speaker dialogue | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| Music/SFX integration | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| User intent matching | ⭐⭐⭐⭐⭐ | ⭐⭐ |

## Recommendation

### Use Claude + ElevenLabs (Approach A) when:
- Deep code understanding required
- Custom narrative structure needed
- User intent varies (onboarding, debugging, review)
- Multi-persona storytelling desired
- Reproducibility important

### Use GenFM/Studio (Approach B) when:
- Quick prototype/MVP needed
- Generic overview sufficient
- Cost is primary concern
- Music/SFX/video features needed
- Single API integration preferred

## Hybrid Approach (Recommended for Code Story)

```python
# Best of both worlds: Claude for script, ElevenLabs Studio for production

# 1. Claude agents generate the script
script = story_architect_agent.generate_script(
    repo_url="https://github.com/mikeyobrien/ralph-orchestrator",
    user_intent="onboarding",
    narrative_style="conversational"
)

# 2. Convert Claude script to ElevenLabs Studio format
studio_content = convert_to_studio_format(script)

# 3. Create Studio project with Claude-generated script
project = client.studio.projects.create(
    name=script["title"],
    default_model_id="eleven_v3",
    from_content_json=json.dumps(studio_content),
    auto_convert=True,
    quality_preset="high"
)

# Benefits:
# - Full narrative control from Claude
# - Studio features (music, SFX, timeline editing)
# - Webhook callbacks for progress
# - Export to multiple formats
```

## API Endpoints Reference

### ElevenLabs Studio API
| Endpoint | Purpose |
|----------|---------|
| `POST /v1/studio/projects` | Create project (from URL, doc, or JSON) |
| `POST /v1/studio/podcasts` | Create GenFM podcast |
| `GET /v1/studio/projects` | List projects |
| `GET /v1/studio/projects/{id}` | Get project details |
| `POST /v1/studio/projects/{id}/convert` | Convert project to audio |

### ElevenLabs TTS API
| Endpoint | Purpose |
|----------|---------|
| `POST /v1/text-to-speech/{voice_id}` | Single voice TTS |
| `POST /v1/text-to-dialogue` | Multi-speaker dialogue (v3) |
| `POST /v1/text-to-speech/{voice_id}/stream` | Streaming TTS |

## Cost Analysis

| Component | Claude + ElevenLabs | GenFM/Studio |
|-----------|--------------------:|-------------:|
| Script generation | ~$0.15-0.50/story (Opus) | $0 (included) |
| Voice synthesis | ~$0.30/1000 chars | ~$0.30/1000 chars |
| **10 min story (~15k chars)** | **~$5.00 total** | **~$4.50 total** |

*Note: GenFM LLM costs currently covered by ElevenLabs, may change in future.*
