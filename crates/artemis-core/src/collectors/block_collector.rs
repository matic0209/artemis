use crate::types::{Collector, CollectorStream};
use anyhow::Result;
use async_trait::async_trait;
use crate::eth::{Middleware, PubsubClient, U64};
use crate::eth::Hash as H256;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use std::sync::Arc;
use tokio_stream::StreamExt;

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
