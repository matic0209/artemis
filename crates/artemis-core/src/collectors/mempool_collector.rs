use async_trait::async_trait;

use crate::eth::{Middleware, PubsubClient, Transaction};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::types::{Collector, CollectorStream};
use anyhow::Result;

/// A collector that listens for new transactions in the mempool, and generates a stream of
/// [events](Transaction) which contain the transaction.
pub struct MempoolCollector<M> {
    provider: Arc<M>,
}

impl<M> MempoolCollector<M> {
    pub fn new(provider: Arc<M>) -> Self {
        Self { provider }
    }
}

/// Implementation of the [Collector](Collector) trait for the [MempoolCollector](MempoolCollector).
/// This implementation uses the [PubsubClient](PubsubClient) to subscribe to new transactions.
#[async_trait]
impl<M> Collector<Transaction> for MempoolCollector<M>
where
    M: Middleware + 'static,
    M::Provider: PubsubClient,
    M::Error: 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Transaction>> {
        let (tx, rx) = mpsc::channel::<Transaction>(2048);
        let provider = self.provider.clone();
        tokio::spawn(async move {
            if let Ok(stream) = provider.subscribe_pending_txs().await {
                let mut stream = stream.transactions_unordered(512);
                while let Some(res) = stream.next().await {
                    if let Ok(txn) = res { if tx.send(txn).await.is_err() { break; } }
                }
            }
        });
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
