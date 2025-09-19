use crate::types::{Collector, CollectorStream};
use anyhow::Result;
use async_trait::async_trait;
use opensea_stream::{
    client,
    schema::{self, ItemListedData},
    subscribe_to, Collection, Network,
};
use tokio::sync::mpsc;
use tokio_stream::{
    wrappers::{BroadcastStream, ReceiverStream},
    StreamExt,
};

/// A collector that listens for new orders on OpenSea, and generates a stream of
/// [events](OpenseaOrder) which contain the order.
#[derive(Default)]
pub struct OpenseaOrderCollector {
    api_key: String,
}

impl OpenseaOrderCollector {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

/// A new order event, containing the internal order.
#[derive(Debug, Clone)]
pub struct OpenseaOrder {
    pub listing: ItemListedData,
}

/// Implementation of the [Collector](Collector) trait for the [OpenseaOrderCollector](OpenseaOrderCollector).
#[async_trait]
impl Collector<OpenseaOrder> for OpenseaOrderCollector {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, OpenseaOrder>> {
        let (tx, rx) = mpsc::channel::<OpenseaOrder>(512);
        let api_key = self.api_key.clone();

        tokio::spawn(async move {
            let collection = Collection::All;
            let mut backoff_secs = 1u64;

            loop {
                let mut client = client(Network::Mainnet, &api_key).await;

                let subscription = match subscribe_to(&mut client, collection.clone()).await {
                    Ok((_, subscription)) => {
                        backoff_secs = 1;
                        subscription
                    }
                    Err(err) => {
                        let counter =
                            metrics::counter!("artemis.collectors.opensea.subscribe_errors");
                        counter.increment(1);
                        tracing::warn!("opensea subscribe error: {}", err);
                        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                        backoff_secs = (backoff_secs * 2).min(60);
                        continue;
                    }
                };

                let mut stream = BroadcastStream::new(subscription);

                while let Some(event) = stream.next().await {
                    match event {
                        Ok(event) => {
                            if let Some(payload) = event.into_custom_payload() {
                                if let schema::Payload::ItemListed(listing) = payload.payload {
                                    if tx.send(OpenseaOrder { listing }).await.is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            let counter =
                                metrics::counter!("artemis.collectors.opensea.stream_errors");
                            counter.increment(1);
                            tracing::warn!("opensea stream error: {}", err);
                            break;
                        }
                    }
                }

                // exponential backoff before reconnecting
                tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(60);
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }
}
