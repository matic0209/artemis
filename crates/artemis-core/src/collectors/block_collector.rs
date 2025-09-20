use crate::types::{Collector, CollectorStream};
use crate::eth::{Hash as H256, U64};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

/// A collector that listens for new blocks, and generates a stream of
/// [events](NewBlock) which contain the block number and hash.
pub struct BlockCollector<P> {
    provider: Arc<P>,
}

/// A new block event, containing the block number and hash.
#[derive(Debug, Clone)]
pub struct NewBlock {
    pub hash: H256,
    pub number: U64,
}

impl<P> BlockCollector<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }
}

/// Implementation of the [Collector](Collector) trait for the [BlockCollector](BlockCollector).
/// This implementation uses Alloy provider to subscribe to new blocks.
#[async_trait]
impl<P> Collector<NewBlock> for BlockCollector<P>
where
    P: alloy_provider::Provider + Send + Sync + 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, NewBlock>> {
        let (tx, rx) = mpsc::channel::<NewBlock>(1024);
        let provider = self.provider.clone();
        tokio::spawn(async move {
            if let Ok(stream) = provider.subscribe_blocks().await {
                let mut stream = stream.into_stream();
                while let Some(block) = stream.next().await {
                    if let (hash, number) = (block.hash, block.number) {
                        let new_block = NewBlock { 
                            hash, 
                            number: U64::from(number) 
                        };
                        if tx.send(new_block).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}