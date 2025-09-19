use std::sync::Arc;

use alloy::network::Ethereum;
use alloy::{eips::Encodable2718, network::NetworkWallet, primitives::B256};
use alloy_mev::{Endpoints, EthMevProviderExt};
use alloy_provider::WalletProvider;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

use crate::eth::TxRequest;
use crate::types::Executor;

/// Configuration for an Alloy Flashbots bundle.
#[derive(Debug, Default, Clone)]
pub struct FlashbotsAlloyBundle {
    pub txs: Vec<TxRequest>,
    pub target_block: Option<u64>,
    pub min_timestamp: Option<u64>,
    pub max_timestamp: Option<u64>,
    pub replacement_uuid: Option<String>,
    pub reverting_hashes: Vec<B256>,
}

/// An Alloy-based Flashbots executor using alloy-mev extension traits.
pub struct FlashbotsAlloyExecutor<P> {
    provider: Arc<P>,
    endpoints: Endpoints,
    block_offset: u64,
}

impl<P> FlashbotsAlloyExecutor<P> {
    pub fn new(provider: Arc<P>, endpoints: Endpoints) -> Self {
        Self {
            provider,
            endpoints,
            block_offset: 1,
        }
    }

    pub fn with_block_offset(mut self, offset: u64) -> Self {
        self.block_offset = offset;
        self
    }
}

#[async_trait]
impl<P> Executor<FlashbotsAlloyBundle> for FlashbotsAlloyExecutor<P>
where
    P: EthMevProviderExt<Ethereum> + WalletProvider<Ethereum> + Send + Sync + 'static,
{
    async fn execute(&self, bundle: FlashbotsAlloyBundle) -> Result<()> {
        if bundle.txs.is_empty() {
            return Ok(());
        }

        let provider = self.provider.as_ref();

        let current_block = provider
            .get_block_number()
            .await
            .context("failed to fetch current block number")?;
        let target_block = bundle
            .target_block
            .unwrap_or_else(|| current_block + self.block_offset);

        let mut builder = provider.bundle_builder().on_block(target_block);

        if let Some(min_ts) = bundle.min_timestamp {
            builder = builder.with_min_timestamp(min_ts);
        }
        if let Some(max_ts) = bundle.max_timestamp {
            builder = builder.with_max_timestamp(max_ts);
        }
        if let Some(uuid) = bundle.replacement_uuid.clone() {
            builder = builder.with_replacement_uuid(uuid);
        }

        for hash in &bundle.reverting_hashes {
            builder = builder.add_reverting_tx(*hash);
        }

        for tx in &bundle.txs {
            let envelope = provider
                .wallet()
                .sign_request(tx.clone())
                .await
                .map_err(|err| anyhow!("failed to sign transaction: {err}"))?;
            let encoded = envelope.encoded_2718().into();
            builder = builder.add_signed_transaction(encoded);
        }

        let send_bundle = builder.build();
        let responses = provider.send_eth_bundle(send_bundle, &self.endpoints).await;

        for response in responses {
            if let Err(err) = response {
                tracing::warn!("failed to send bundle to endpoint: {err}");
            }
        }

        Ok(())
    }
}
