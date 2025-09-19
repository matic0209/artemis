use std::sync::Arc;

use alloy::{network::Ethereum, rpc::types::mev::MevSendBundle, signers::Signer};
use alloy_mev::MevShareProviderExt;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tracing::info;

use crate::types::Executor;

/// Alloy-based executor that submits bundles to the Flashbots MEV-Share matchmaker.
pub struct MevshareAlloyExecutor<P, S> {
    provider: Arc<P>,
    signer: S,
}

impl<P, S> MevshareAlloyExecutor<P, S> {
    /// Creates a new MEV-Share executor backed by an Alloy provider and signer.
    pub fn new(provider: Arc<P>, signer: S) -> Self {
        Self { provider, signer }
    }
}

#[async_trait]
impl<P, S> Executor<MevSendBundle> for MevshareAlloyExecutor<P, S>
where
    P: MevShareProviderExt<Ethereum> + Send + Sync + 'static,
    S: Signer + Clone + Send + Sync + 'static,
{
    async fn execute(&self, bundle: MevSendBundle) -> Result<()> {
        self.provider
            .send_mev_bundle(bundle, self.signer.clone())
            .await
            .map(|hash| info!(?hash, "sent MEV-Share bundle"))
            .map_err(|err| anyhow!("failed to send MEV-Share bundle: {err}"))?;

        Ok(())
    }
}
