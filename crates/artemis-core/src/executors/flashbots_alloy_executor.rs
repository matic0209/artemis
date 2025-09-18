use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::types::Executor;

/// A bundle of raw signed transactions (0x-prefixed RLP hex) to send to block builders.
pub type FlashbotsAlloyBundle = Vec<String>;

/// An Alloy-based Flashbots executor using alloy-mev extension traits.
pub struct FlashbotsAlloyExecutor<P> {
    provider: Arc<P>,
}

impl<P> FlashbotsAlloyExecutor<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl<P> Executor<FlashbotsAlloyBundle> for FlashbotsAlloyExecutor<P>
where
    P: Send + Sync + 'static,
{
    async fn execute(&self, bundle: FlashbotsAlloyBundle) -> Result<()> {
        // TODO: wire alloy_mev extension methods once provider type and trait are fixed.
        let _ = bundle;
        Ok(())
    }
}


