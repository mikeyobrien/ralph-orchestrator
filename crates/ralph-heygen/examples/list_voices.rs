use ralph_heygen::HeygenApi;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== HeyGen Voice Lister ===\n");

    // Create API client from environment variables
    let api = HeygenApi::from_env()?;

    // List available voices
    println!("Fetching available voices...\n");
    let voices = api.list_voices().await?;

    println!("Found {} voices:\n", voices.len());

    for voice in voices {
        println!("ID: {}", voice.id);
        println!("  Name: {}", voice.name);
        println!("  HeyGen Voice ID: {:?}", voice.voice_id);
        println!("  Enabled: {}", voice.enabled);
        println!();
    }

    Ok(())
}
