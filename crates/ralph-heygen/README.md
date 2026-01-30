# ralph-heygen

HeyGen video generation integration for Ralph Orchestrator.

This crate provides a Rust client for the [HeyGen API](https://www.heygen.com/), enabling video generation from images, scripts, and AI voices (via ElevenLabs integration).

## Features

- **Voice Management**: List and enable ElevenLabs voices in HeyGen
- **Asset Upload**: Upload images to HeyGen
- **Video Generation**: Create videos from images, scripts, and voices
- **Polling**: Wait for video generation completion
- **File Download**: Download generated videos and thumbnails
- **High-level API**: Orchestrate the entire video generation flow with a single call

## Configuration

Create a `.env` file in your project root with the following variables:

```env
HEYGEN_API_KEY=your_heygen_api_key_here
HEYGEN_IMPORTED_ELEVENLABS_KEY_ID=your_elevenlabs_key_id_here

# Optional: Default video settings
HEYGEN_DEFAULT_VIDEO_ORIENTATION=vertical  # Options: vertical, horizontal, square
HEYGEN_DEFAULT_FIT=contain  # Options: contain, cover, crop
HEYGEN_POLLING_TIMEOUT_SECONDS=600  # 10 minutes
HEYGEN_POLLING_INTERVAL_SECONDS=3
```

## Usage

### High-level API (Recommended)

The `VideoGenerator` provides a simple interface that handles the entire video generation flow:

```rust
use ralph_heygen::{VideoGenerator, VideoGenerationParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create generator from environment variables
    let generator = VideoGenerator::from_env()?;

    // Define video parameters
    let params = VideoGenerationParams {
        script: "Hello, this is a test video!".to_string(),
        voice_id: "your_voice_id".to_string(),
        image_path: "./image.jpg".to_string(),
        video_title: "Test Video".to_string(),
        video_orientation: Some("vertical".to_string()),
        fit: Some("contain".to_string()),
    };

    // Generate video (download files)
    let result = generator.generate_video(params, true).await?;

    // Save to disk
    generator.save_video(
        &result,
        std::path::Path::new("./output_video.mp4"),
        std::path::Path::new("./output_thumbnail.jpg"),
    ).await?;

    println!("Video generated: {}", result.video_url);

    Ok(())
}
```

### Low-level API

For more control, use the `HeygenApi` directly:

```rust
use ralph_heygen::HeygenApi;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = HeygenApi::from_env()?;

    // List available voices
    let voices = api.list_voices().await?;
    for voice in voices {
        println!("Voice: {} ({})", voice.name, voice.id);
    }

    // Ensure voice is enabled
    let (voice_id, voice_name) = api.ensure_voice_enabled("voice_123").await?;
    println!("Voice {} enabled: {}", voice_name, voice_id);

    // Upload image
    let image_data = std::fs::read("./image.jpg")?;
    let image_key = api.upload_asset(image_data, "image/jpeg").await?;
    println!("Image uploaded: {}", image_key);

    // Create video
    let video_id = api.create_video(
        &image_key,
        "Hello, world!",
        &voice_id,
        "My Video",
        Some("vertical"),
        Some("contain"),
    ).await?;
    println!("Video creation started: {}", video_id);

    // Poll for completion
    let (video_url, thumbnail_url) = api.poll_for_completion(&video_id).await?;
    println!("Video URL: {}", video_url);
    println!("Thumbnail URL: {}", thumbnail_url);

    Ok(())
}
```

## Examples

The crate includes example programs demonstrating different use cases:

### Generate a Video

```bash
# Set up environment variables first
export HEYGEN_API_KEY="your_key"
export HEYGEN_IMPORTED_ELEVENLABS_KEY_ID="your_elevenlabs_key_id"

# Run the example
cargo run --example generate_video ./image.jpg voice_123 "Hello world" "My Video" vertical contain
```

### List Available Voices

```bash
cargo run --example list_voices
```

## Video Generation Flow

The `VideoGenerator` orchestrates the following steps:

1. **Enable Voice**: Ensures the specified ElevenLabs voice is enabled in HeyGen
2. **Upload Image**: Uploads the image asset to HeyGen
3. **Create Video**: Initiates video generation with the script and voice
4. **Poll for Completion**: Waits for the video to be generated (with timeout)
5. **Download Files**: Optionally downloads the generated video and thumbnail

## Error Handling

The crate uses a comprehensive error type that covers:

- HTTP errors
- API errors (invalid responses, failed operations)
- Configuration errors (missing environment variables)
- Voice not found errors
- Video generation failures
- Timeouts
- I/O errors

## Integration with Content Ideation System

This crate is designed to work with the Ralph orchestrator's content ideation system. After generating content ideas (see `.ideation/`), you can use this crate to create videos from those ideas:

```rust
use ralph_heygen::{VideoGenerator, VideoGenerationParams};
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load idea from ideas.yaml
    let ideas_yaml = fs::read_to_string(".ideation/output/ideas.yaml")?;
    // Parse YAML and extract idea...

    let generator = VideoGenerator::from_env()?;

    let params = VideoGenerationParams {
        script: idea.hook, // Use the hook as script
        voice_id: "voice_id_from_avatar".to_string(),
        image_path: idea.image_files[0].clone(),
        video_title: idea.title,
        video_orientation: Some("vertical".to_string()),
        fit: Some("contain".to_string()),
    };

    let result = generator.generate_video(params, true).await?;
    // Save video...

    Ok(())
}
```

## Testing

Run the test suite:

```bash
cargo test -p ralph-heygen
```

## License

MIT
