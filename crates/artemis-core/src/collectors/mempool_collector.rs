use async_trait::async_trait;
#[cfg(feature = "sdk-ethers")]
use futures::stream::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
#[cfg(feature = "sdk-ethers")]
use tokio::sync::mpsc::error::TrySendError;
use tokio_stream::wrappers::ReceiverStream;

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use crate::eth::Provider as AlloyProvider;
use crate::eth::Transaction;
#[cfg(feature = "sdk-ethers")]
use crate::eth::{Middleware, PubsubClient};
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use alloy_provider::Provider as ProviderTrait;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use alloy_rpc_types_eth::FilterChanges;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use tracing::warn;

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
#[cfg(feature = "sdk-ethers")]
#[async_trait]
impl<M> Collector<Transaction> for MempoolCollector<M>
where
    M: Middleware + 'static,
    M::Provider: PubsubClient,
    M::Error: 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Transaction>> {
        let (sender, rx) = mpsc::channel::<Transaction>(2048);
        let provider = self.provider.clone();

        tokio::spawn(async move {
            let mut backoff_secs = 1u64;
            loop {
                let mut stream = match provider.subscribe_pending_txs().await {
                    Ok(stream) => {
                        backoff_secs = 1;
                        stream.transactions_unordered(512)
                    }
                    Err(err) => {
                        let counter =
                            metrics::counter!("artemis.collectors.mempool.subscribe_errors");
                        counter.increment(1);
                        tracing::warn!("mempool subscription failed: {}", err);
                        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                        backoff_secs = (backoff_secs * 2).min(64);
                        continue;
                    }
                };

                while let Some(res) = stream.next().await {
                    match res {
                        Ok(txn) => match sender.try_send(txn) {
                            Ok(_) => {}
                            Err(TrySendError::Full(txn)) => {
                                let counter = metrics::counter!(
                                    "artemis.collectors.mempool.backpressure_hits"
                                );
                                counter.increment(1);
                                if sender.send(txn).await.is_err() {
                                    return;
                                }
                            }
                            Err(TrySendError::Closed(_)) => return,
                        },
                        Err(err) => {
                            let counter =
                                metrics::counter!("artemis.collectors.mempool.stream_errors");
                            counter.increment(1);
                            tracing::warn!("error in mempool stream: {}", err);
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
impl Collector<Transaction> for MempoolCollector<AlloyProvider> {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Transaction>> {
        let (sender, rx) = mpsc::channel::<Transaction>(2048);
        let provider = self.provider.clone();

        tokio::spawn(async move {
            let mut backoff_secs = 1u64;
            let mut last_seen = std::collections::BTreeSet::<alloy_primitives::TxHash>::new();
            loop {
                match provider.new_pending_transactions_filter(true).await {
                    Ok(filter_id) => {
                        loop {
                            match provider.get_filter_changes_dyn(filter_id).await {
                                Ok(FilterChanges::Transactions(transactions)) => {
                                    for tx in transactions {
                                        let tx_hash = *tx.inner.tx_hash();
                                        if !last_seen.insert(tx_hash) {
                                            continue;
                                        }

                                        if last_seen.len() > 10_000 {
                                            if let Some(first) = last_seen.iter().next().cloned() {
                                                last_seen.remove(&first);
                                            }
                                        }

                                        if sender.send(tx).await.is_err() {
                                            let _ = provider.uninstall_filter(filter_id).await;
                                            return;
                                        }
                                    }
                                }
                                Ok(FilterChanges::Hashes(_)) => {}
                                Ok(FilterChanges::Logs(_)) => {}
                                Ok(FilterChanges::Empty) => {}
                                Err(err) => {
                                    warn!("error polling pending tx filter: {err}");
                                    break;
                                }
                            }

                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        }

                        if let Err(err) = provider.uninstall_filter(filter_id).await {
                            warn!("failed to uninstall pending tx filter: {err}");
                        }
                    }
                    Err(err) => {
                        warn!("failed to install pending tx filter: {err}");
                        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                        backoff_secs = (backoff_secs * 2).min(64);
                        continue;
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
