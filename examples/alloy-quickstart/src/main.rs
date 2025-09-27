
use anyhow::Result;
use artemis_core::eth::alloy_support::{helpers, Provider};
use std::env;

/// Minimal Alloy smoke test: connects to the configured WebSocket endpoint,
/// fetches the latest block number, and prints it to stdout.
#[tokio::main]
async fn main() -> Result<()> {
    let wss = env::var("ALLOY_WS_ENDPOINT").unwrap_or_else(|_| "ws://localhost:8546".into());
    let provider: Provider = helpers::create_ws_provider(&wss).await?;
    let block_number = helpers::get_block_number(&provider).await?;

    println!("Alloy provider ready. Latest block: {block_number}");
    Ok(())
}
