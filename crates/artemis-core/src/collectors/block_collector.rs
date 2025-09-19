use crate::eth::Hash as H256;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use crate::eth::{helpers as alloy_helpers, BlockNumberOrTag, Provider as AlloyProvider, U64};
#[cfg(feature = "sdk-ethers")]
use crate::eth::{Middleware, PubsubClient, U64};
use crate::types::{Collector, CollectorStream};
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use alloy_provider::Provider as ProviderTrait;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use tokio::time::{Duration, MissedTickBehavior};
use tokio_stream::wrappers::ReceiverStream;
#[cfg(feature = "sdk-ethers")]
use tokio_stream::StreamExt;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use tracing::warn;

/// A collector that listens for new blocks, and generates a stream of
/// [events](NewBlock) which contain the block number and hash.
pub struct BlockCollector<M> {
    provider: Arc<M>,
}

/// A new block event, containing the block number and hash.
#[derive(Debug, Clone)]
pub struct NewBlock {
    pub hash: H256,
    pub number: U64,
}

impl<M> BlockCollector<M> {
    pub fn new(provider: Arc<M>) -> Self {
        Self { provider }
    }
}

/// Implementation of the [Collector](Collector) trait for the [BlockCollector](BlockCollector).
/// This implementation uses the [PubsubClient](PubsubClient) to subscribe to new blocks.
#[cfg(feature = "sdk-ethers")]
#[async_trait]
impl<M> Collector<NewBlock> for BlockCollector<M>
where
    M: Middleware + 'static,
    M::Provider: PubsubClient,
    M::Error: 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, NewBlock>> {
        let (tx, rx) = mpsc::channel::<NewBlock>(1024);
        let provider = self.provider.clone();
        tokio::spawn(async move {
            if let Ok(mut stream) = provider.subscribe_blocks().await {
                while let Some(block) = stream.next().await {
                    if let (Some(hash), Some(number)) = (block.hash, block.number) {
                        if tx.send(NewBlock { hash, number }).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
#[async_trait]
impl Collector<NewBlock> for BlockCollector<AlloyProvider> {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, NewBlock>> {
        let (tx, rx) = mpsc::channel::<NewBlock>(1024);
        let provider = self.provider.clone();
        tokio::spawn(async move {
            let mut last_emitted: Option<u64> = None;
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                interval.tick().await;

                let current_number = match alloy_helpers::get_block_number(&provider).await {
                    Ok(val) => val.to::<u64>(),
                    Err(err) => {
                        warn!("failed to poll block number via alloy provider: {err}");
                        continue;
                    }
                };

                let start = match last_emitted {
                    Some(last) if current_number > last => last + 1,
                    Some(_) => continue,
                    None => current_number,
                };

                for number in start..=current_number {
                    let tag = BlockNumberOrTag::Number(number);
                    match provider.get_block_by_number(tag).await {
                        Ok(Some(block)) => {
                            let event = NewBlock {
                                hash: block.hash(),
                                number: crate::eth::U64::from(number),
                            };
                            last_emitted = Some(number);
                            if tx.send(event).await.is_err() {
                                return;
                            }
                        }
                        Ok(None) => {
                            last_emitted = Some(number);
                        }
                        Err(err) => {
                            warn!("failed to fetch block {number} via alloy provider: {err}");
                            break;
                        }
                    }
                }
            }
        });
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
