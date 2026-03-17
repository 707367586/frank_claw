//! ClawX CLI — interactive command-line interface for the agent runtime.

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("ClawX CLI starting");

    // TODO: parse args, load config, initialize runtime
    Ok(())
}
