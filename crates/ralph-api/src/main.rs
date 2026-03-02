use anyhow::Result;
use tracing_subscriber::EnvFilter;

use ralph_api::{ApiConfig, serve};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let config = ApiConfig::from_env()?;
    serve(config).await
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("ralph_api=info,tower_http=info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .compact()
        .init();
}
