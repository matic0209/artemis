use async_trait::async_trait;
use crate::eth::{Transaction, Address};
use crate::types::{Collector, CollectorStream};
use anyhow::Result;
use futures::StreamExt;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// A collector that listens for new transactions in the mempool, and generates a stream of
/// [events](Transaction) which contain the transaction.
pub struct MempoolCollector<P> {
    provider: Arc<P>,
    /// Filter transactions to/from these addresses only (empty = no filter)
    address_filter: HashSet<Address>,
    /// Minimum transaction value to consider (in wei)
    min_value_wei: u128,
}

impl<P> MempoolCollector<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { 
            provider,
            address_filter: HashSet::new(),
            min_value_wei: 0,
        }
    }

    /// Add address filter to only process transactions involving these addresses
    pub fn with_address_filter(mut self, addresses: HashSet<Address>) -> Self {
        self.address_filter = addresses;
        self
    }

    /// Set minimum transaction value to consider (in wei)
    pub fn with_min_value(mut self, min_value_wei: u128) -> Self {
        self.min_value_wei = min_value_wei;
        self
    }

}

/// Implementation of the [Collector](Collector) trait for the [MempoolCollector](MempoolCollector).
/// This implementation uses Alloy provider to subscribe to new transactions.
#[async_trait]
impl<P> Collector<Transaction> for MempoolCollector<P>
where
    P: alloy_provider::Provider + Send + Sync + 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Transaction>> {
        let (tx, rx) = mpsc::channel::<Transaction>(2048);
        let provider = self.provider.clone();
        
        tokio::spawn(async move {
            if let Ok(stream) = provider.subscribe_pending_transactions().await {
                let mut stream = stream.into_stream();
                while let Some(tx_hash) = stream.next().await {
                    // Get full transaction details
                    if let Ok(Some(txn)) = provider.get_transaction_by_hash(tx_hash).await {
                        if tx.send(txn).await.is_err() {
                            break;
                        }
                        metrics::counter!("artemis.collectors.mempool.transactions_processed").increment(1);
                    }
                }
            }
        });
        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}