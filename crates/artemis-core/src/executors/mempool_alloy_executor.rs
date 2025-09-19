use std::{
    ops::{Div, Mul},
    sync::Arc,
};

use alloy::{network::Ethereum, primitives::keccak256};
use alloy_primitives::B256;
use alloy_provider::{Provider, WalletProvider};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use dashmap::DashMap;

use crate::eth::{Address as H160, TxRequest, U256};
use crate::executors::mempool_types::SubmitTxToMempool;
use crate::types::Executor;

/// An executor that sends transactions to the mempool using Alloy provider.
pub struct MempoolAlloyExecutor<P> {
    provider: Arc<P>,
    gas_cache: DashMap<GasCacheKey, U256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GasCacheKey {
    to: Option<H160>,
    data_hash: Option<B256>,
    value: Option<U256>,
    chain_id: Option<u64>,
}

impl GasCacheKey {
    fn new(tx: &TxRequest) -> Option<Self> {
        let to = tx.to.as_ref().and_then(|kind| kind.to()).copied();
        let data_hash = tx
            .input
            .clone()
            .into_input()
            .map(|bytes| keccak256(bytes.as_ref()));
        let value = tx.value.clone();
        let chain_id = tx.chain_id;

        Some(Self {
            to,
            data_hash,
            value,
            chain_id,
        })
    }
}

impl<P> MempoolAlloyExecutor<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self {
            provider,
            gas_cache: DashMap::new(),
        }
    }

    async fn estimate_gas_cached(
        &self,
        tx: &TxRequest,
        cache_key: Option<GasCacheKey>,
    ) -> Result<U256>
    where
        P: Provider<Ethereum> + Send + Sync,
    {
        if let Some(key) = cache_key {
            if let Some(entry) = self.gas_cache.get(&key) {
                metrics::counter!("artemis.executors.mempool.gas_cache_hits").increment(1);
                return Ok(*entry.value());
            }

            metrics::counter!("artemis.executors.mempool.gas_cache_misses").increment(1);
            let gas_usage = self
                .provider
                .estimate_gas(tx.clone())
                .await
                .context("error estimating gas usage")?;
            let gas_usage_u256 = U256::from(gas_usage);
            self.gas_cache.insert(key, gas_usage_u256);
            metrics::gauge!("artemis.executors.mempool.gas_cache_size")
                .set(self.gas_cache.len() as f64);
            Ok(gas_usage_u256)
        } else {
            metrics::counter!("artemis.executors.mempool.gas_cache_bypassed").increment(1);
            self.provider
                .estimate_gas(tx.clone())
                .await
                .context("error estimating gas usage")
                .map(U256::from)
        }
    }
}

#[async_trait]
impl<P> Executor<SubmitTxToMempool> for MempoolAlloyExecutor<P>
where
    P: Provider<Ethereum> + WalletProvider<Ethereum> + Send + Sync + 'static,
{
    async fn execute(&self, mut action: SubmitTxToMempool) -> Result<()> {
        let cache_key = GasCacheKey::new(&action.tx);
        let gas_usage = self.estimate_gas_cached(&action.tx, cache_key).await?;

        let bid_gas_price_u128 = if let Some(gas_bid_info) = action.gas_bid_info {
            let breakeven_gas_price = gas_bid_info.total_profit / gas_usage;
            let bid_gas_price = breakeven_gas_price
                .mul(U256::from(gas_bid_info.bid_percentage))
                .div(U256::from(100u64));
            u128::try_from(bid_gas_price).map_err(|_| anyhow!("gas price exceeds u128"))?
        } else {
            self.provider
                .get_gas_price()
                .await
                .context("error getting gas price")?
        };

        action.tx.gas_price = Some(bid_gas_price_u128);

        let pending = self
            .provider
            .send_transaction(action.tx.clone())
            .await
            .context("error sending transaction")?;

        // Register the pending transaction but don't block on confirmation.
        let _ = pending.register().await;
        Ok(())
    }
}
