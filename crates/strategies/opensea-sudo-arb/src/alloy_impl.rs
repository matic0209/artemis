use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use artemis_core::types::Strategy;
use opensea_v2::client::OpenSeaV2Client;
use crate::types::{Action, Config, Event};

/// Alloy-based OpenSea Sudoswap arbitrage strategy.
pub struct OpenseaSudoArb<P> {
    provider: Arc<P>,
    opensea_client: OpenSeaV2Client,
    config: Config,
}

impl<P> OpenseaSudoArb<P> {
    pub fn new(provider: Arc<P>, opensea_client: OpenSeaV2Client, config: Config) -> Self {
        Self {
            provider,
            opensea_client,
            config,
        }
    }
}

#[async_trait]
impl<P> Strategy<Event, Action> for OpenseaSudoArb<P>
where
    P: Send + Sync + 'static,
{
    async fn sync_state(&mut self) -> Result<()> {
        // TODO: Implement Alloy-based state sync
        Ok(())
    }

    async fn process_event(&mut self, _event: Event) -> Vec<Action> {
        // TODO: Implement Alloy-based event processing
        vec![]
    }
}
