use crate::types::{Collector, CollectorStream};
use crate::eth::Filter;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio_stream::StreamExt;

/// A collector that listens for new blockchain event logs based on a [Filter](Filter),
/// and generates a stream of [events](alloy_rpc_types_eth::Log).
pub struct LogCollector<P> {
    provider: Arc<P>,
    filter: Filter,
}

impl<P> LogCollector<P> {
    pub fn new(provider: Arc<P>, filter: Filter) -> Self {
        Self { provider, filter }
    }
}

/// Implementation of the [Collector](Collector) trait for the [LogCollector](LogCollector).
/// This implementation uses Alloy provider to subscribe to new logs.
#[async_trait]
impl<P> Collector<alloy_rpc_types_eth::Log> for LogCollector<P>
where
    P: alloy_provider::Provider + Send + Sync + 'static,
{
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, alloy_rpc_types_eth::Log>> {
        let stream = self.provider.subscribe_logs(&self.filter).await?;
        let stream = stream.into_stream();
        Ok(Box::pin(stream))
    }
}