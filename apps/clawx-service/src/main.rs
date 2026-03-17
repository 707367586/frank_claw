//! ClawX Service — background daemon for agent process supervision.

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("ClawX Service starting");

    // TODO: load config, start event bus, initialize runtime, begin supervision
    Ok(())
}
