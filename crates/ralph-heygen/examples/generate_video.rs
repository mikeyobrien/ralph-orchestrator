use ralph_heygen::{VideoGenerationParams, VideoGenerator};
use std::path::Path;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!("Usage: {} <image_path> <voice_id> <script> <video_title> [orientation] [fit]", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} ./image.jpg voice_123 \"Hello world\" \"My Video\" vertical contain", args[0]);
        eprintln!("\nOrientation options: vertical, horizontal, square (default: vertical)");
        eprintln!("Fit options: contain, cover, crop (default: contain)");
        std::process::exit(1);
    }

    let image_path = &args[1];
    let voice_id = &args[2];
    let script = &args[3];
    let video_title = &args[4];
    let orientation = args.get(5).map(|s| s.to_string());
    let fit = args.get(6).map(|s| s.to_string());

    println!("=== HeyGen Video Generator ===");
    println!("Image: {}", image_path);
    println!("Voice ID: {}", voice_id);
    println!("Script: {}", script);
    println!("Title: {}", video_title);
    println!("Orientation: {:?}", orientation.as_deref().unwrap_or("vertical"));
    println!("Fit: {:?}", fit.as_deref().unwrap_or("contain"));
    println!();

    // Create video generator from environment variables
    let generator = VideoGenerator::from_env()?;

    // Define video parameters
    let params = VideoGenerationParams {
        script: script.to_string(),
        voice_id: voice_id.to_string(),
        image_path: image_path.to_string(),
        video_title: video_title.to_string(),
        video_orientation: orientation,
        fit,
    };

    // Generate video (download files)
    println!("Starting video generation...");
    let result = generator.generate_video(params, true).await?;

    // Save to disk
    let video_output = format!("{}_video.mp4", video_title.replace(' ', "_"));
    let thumbnail_output = format!("{}_thumbnail.jpg", video_title.replace(' ', "_"));

    generator
        .save_video(
            &result,
            Path::new(&video_output),
            Path::new(&thumbnail_output),
        )
        .await?;

    println!("\n=== Video Generation Complete ===");
    println!("Video ID: {}", result.video_id);
    println!("Video URL: {}", result.video_url);
    println!("Thumbnail URL: {}", result.thumbnail_url);
    println!("Video saved to: {}", video_output);
    println!("Thumbnail saved to: {}", thumbnail_output);

    Ok(())
}
