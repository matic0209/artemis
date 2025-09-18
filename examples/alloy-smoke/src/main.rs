use anyhow::Result;
use tracing::info;
use tracing_subscriber::prelude::*;

use alloy_provider::{Provider, ProviderBuilder};
use alloy_transport_http::reqwest::Url;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).init();

    let url = std::env::var("RPC").unwrap_or_else(|_| "https://ethereum.publicnode.com".to_string());
    let url = Url::parse(&url)?;
    let provider = ProviderBuilder::new().on_http(url);

    let block_number = provider.get_block_number().await?;
    info!(?block_number, "Alloy provider connected");

    Ok(())
}

