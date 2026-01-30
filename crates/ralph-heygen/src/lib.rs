//! HeyGen video generation integration for Ralph Orchestrator
//!
//! This crate provides a Rust client for the HeyGen API, enabling video generation
//! from images, scripts, and voices.
//!
//! # Examples
//!
//! ## Basic usage
//!
//! ```no_run
//! use ralph_heygen::{VideoGenerator, VideoGenerationParams};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create generator from environment variables
//!     let generator = VideoGenerator::from_env()?;
//!
//!     // Define video parameters
//!     let params = VideoGenerationParams {
//!         script: "Hello, this is a test video!".to_string(),
//!         voice_id: "your_voice_id".to_string(),
//!         image_path: "./image.jpg".to_string(),
//!         video_title: "Test Video".to_string(),
//!         video_orientation: Some("vertical".to_string()),
//!         fit: Some("contain".to_string()),
//!     };
//!
//!     // Generate video (download files)
//!     let result = generator.generate_video(params, true).await?;
//!
//!     // Save to disk
//!     generator.save_video(
//!         &result,
//!         std::path::Path::new("./output_video.mp4"),
//!         std::path::Path::new("./output_thumbnail.jpg"),
//!     ).await?;
//!
//!     println!("Video generated: {}", result.video_url);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Using the low-level API directly
//!
//! ```no_run
//! use ralph_heygen::HeygenApi;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let api = HeygenApi::from_env()?;
//!
//!     // List available voices
//!     let voices = api.list_voices().await?;
//!     for voice in voices {
//!         println!("Voice: {} ({})", voice.name, voice.id);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod api;
pub mod config;
pub mod error;
pub mod generator;
pub mod types;

// Re-export main types
pub use api::HeygenApi;
pub use config::HeygenConfig;
pub use error::{HeygenError, Result};
pub use generator::{VideoGenerationParams, VideoGenerationResult, VideoGenerator};
pub use types::{VideoStatus, Voice};
