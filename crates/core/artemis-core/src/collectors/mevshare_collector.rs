use crate::types::{Collector, CollectorStream};
use anyhow::Result;
use async_trait::async_trait;
use mev_share::sse::{Event, EventClient};
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tracing::warn;

/// A collector that streams from MEV-Share SSE endpoint
/// and generates [events](Event), which return tx hash, logs, and bundled txs.
pub struct MevShareCollector {
    mevshare_sse_url: String,
}

impl MevShareCollector {
    pub fn new(mevshare_sse_url: String) -> Self {
        Self { mevshare_sse_url }
    }
}

/// Implementation of the [Collector](Collector) trait for the
/// [MevShareCollector](MevShareCollector).
#[async_trait]
impl Collector<Event> for MevShareCollector {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, Event>> {
        let (tx, rx) = mpsc::channel::<Event>(512);
        let url = self.mevshare_sse_url.clone();

        tokio::spawn(async move {
            let client = EventClient::default();
            let mut backoff_secs = 1u64;

            loop {
                match client.events(&url).await {
                    Ok(mut stream) => {
                        backoff_secs = 1;
                        while let Some(event) = stream.next().await {
                            match event {
                                Ok(evt) => {
                                    if tx.send(evt).await.is_err() {
                                        return;
                                    }
                                }
                                Err(err) => {
                                    let counter = metrics::counter!(
                                        "artemis.collectors.mevshare.stream_errors"
                                    );
                                    counter.increment(1);
                                    warn!("mev-share stream error: {}", err);
                                    break;
                                }
                            }
                        }
                    }
                    Err(err) => {
                        let counter =
                            metrics::counter!("artemis.collectors.mevshare.connect_errors");
                        counter.increment(1);
                        warn!("failed to connect mev-share stream: {}", err);
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(60);
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
