use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use artemis_core::types::Strategy;
use artemis_core::eth::{Address, LocalWallet};
use crate::types::{Action, Event};

/// Pool information for V2 pools.
#[derive(Debug, Clone)]
pub struct V2PoolInfo {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
}

/// Alloy-based MEV-Share Uniswap arbitrage strategy.
pub struct MevShareUniArb<P> {
    provider: Arc<P>,
    wallet: LocalWallet,
    arb_contract_address: Address,
}

impl<P> MevShareUniArb<P> {
    pub fn new(provider: Arc<P>, wallet: LocalWallet, arb_contract_address: Address) -> Self {
        Self {
            provider,
            wallet,
            arb_contract_address,
        }
    }
}

#[async_trait]
impl<P> Strategy<Event, Action> for MevShareUniArb<P>
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
