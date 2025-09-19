use crate::eth::{Filter, Log};
use crate::types::{Collector, CollectorStream};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
#[cfg(feature = "sdk-ethers")]
use tokio_stream::StreamExt;

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use crate::eth::{BlockNumberOrTag, Provider as AlloyProvider};
#[cfg(feature = "sdk-ethers")]
use crate::eth::{Middleware, PubsubClient};
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use alloy_provider::Provider as ProviderTrait;
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use tokio::time::{Duration, MissedTickBehavior};
#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
use tracing::warn;

/// A collector that listens for new blockchain event logs based on a [Filter](Filter),
/// and generates a stream of [events](Log).
pub struct LogCollector<M> {
    provider: Arc<M>,
    filter: Filter,
}

impl<M> LogCollector<M> {
    pub fn new(provider: Arc<M>, filter: Filter) -> Self {
        Self { provider, filter }
    }
}

/// Implementation of the [Collector](Collector) trait for the [LogCollector](LogCollector).
/// This implementation uses the [PubsubClient](PubsubClient) to subscribe to new logs.
#[async_trait]
#[cfg(feature = "sdk-ethers")]
impl<M> Collector<Log> for LogCollector<M>
where
    M: Middleware,
    M::Provider: PubsubClient,
    M::Error: 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Log>> {
        let stream = self.provider.subscribe_logs(&self.filter).await?;
        let stream = stream.filter_map(Some);
        Ok(Box::pin(stream))
    }
}

#[cfg(all(feature = "sdk-alloy", not(feature = "sdk-ethers")))]
#[async_trait]
impl Collector<Log> for LogCollector<AlloyProvider> {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Log>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Log>(1024);
        let provider = self.provider.clone();
        let base_filter = self.filter.clone();

        tokio::spawn(async move {
            let mut last_cursor: Option<(u64, u64)> = None;
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

            loop {
                interval.tick().await;

                let logs_res = provider.get_logs(&base_filter).await;
                let mut logs = match logs_res {
                    Ok(entries) => entries,
                    Err(err) => {
                        warn!("failed to poll logs via alloy provider: {err}");
                        continue;
                    }
                };

                logs.sort_by(|a, b| {
                    let a_block = a.block_number.unwrap_or(u64::MAX);
                    let b_block = b.block_number.unwrap_or(u64::MAX);
                    match a_block.cmp(&b_block) {
                        std::cmp::Ordering::Equal => {
                            let a_idx = a.log_index.unwrap_or(u64::MAX);
                            let b_idx = b.log_index.unwrap_or(u64::MAX);
                            a_idx.cmp(&b_idx)
                        }
                        other => other,
                    }
                });

                for log in logs {
                    let block_number = match log.block_number {
                        Some(num) => num,
                        None => continue,
                    };
                    let log_index = log.log_index.unwrap_or(0);

                    if let Some((last_block, last_idx)) = last_cursor {
                        if block_number < last_block
                            || (block_number == last_block && log_index <= last_idx)
                        {
                            continue;
                        }
                    }

                    last_cursor = Some((block_number, log_index));
                    if tx.send(log.clone()).await.is_err() {
                        return;
                    }
                }

                // When the filter has an explicit to_block set to a past height, break to avoid
                // spinning forever.
                if let Some(to_block) = base_filter.block_option.get_to_block() {
                    if let BlockNumberOrTag::Number(num) = to_block {
                        if last_cursor.map(|(b, _)| b).unwrap_or_default() >= *num {
                            return;
                        }
                    }
                }
            }
        });

        Ok(Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}
