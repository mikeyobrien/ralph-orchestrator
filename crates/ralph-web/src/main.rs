//! Ralph Web Dashboard - Binary entry point

use ralph_web::{Config, serve};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ralph_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config {
        port: 3000,
        static_dir: Some(PathBuf::from("frontend/dist")),
        diagnostics_dir: Some(PathBuf::from(".ralph/diagnostics")),
    };

    tracing::info!(
        "Starting Ralph Web Dashboard on http://localhost:{}",
        config.port
    );

    serve(config).await?;

    Ok(())
}
