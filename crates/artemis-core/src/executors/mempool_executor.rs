use std::{
    ops::{Div, Mul},
    sync::Arc,
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use ethers::utils::keccak256;

use crate::eth::{Address as H160, Middleware, TxRequest, U256};
use crate::executors::mempool_types::SubmitTxToMempool;
use crate::types::Executor;

/// An executor that sends transactions to the mempool.
pub struct MempoolExecutor<M> {
    client: Arc<M>,
    gas_cache: DashMap<GasCacheKey, U256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GasCacheKey {
    to: Option<H160>,
    data_hash: Option<[u8; 32]>,
    value: Option<U256>,
    chain_id: Option<u64>,
}

impl GasCacheKey {
    fn new(tx: &TxRequest) -> Option<Self> {
        let to = tx.to().and_then(|to| to.as_address()).copied();
        let data_hash = tx.data().map(|bytes| keccak256(bytes.as_ref()));
        let value = tx.value().cloned();
        let chain_id = tx.chain_id().map(|id| id.as_u64());

        Some(Self {
            to,
            data_hash,
            value,
            chain_id,
        })
    }
}

impl<M: Middleware> MempoolExecutor<M> {
    pub fn new(client: Arc<M>) -> Self {
        Self {
            client,
            gas_cache: DashMap::new(),
        }
    }

    async fn estimate_gas_cached(
        &self,
        tx: &TxRequest,
        cache_key: Option<GasCacheKey>,
    ) -> Result<U256>
    where
        M::Error: 'static,
    {
        if let Some(key) = cache_key {
            if let Some(entry) = self.gas_cache.get(&key) {
                let counter = metrics::counter!("artemis.executors.mempool.gas_cache_hits");
                counter.increment(1);
                return Ok(*entry.value());
            }
            let counter = metrics::counter!("artemis.executors.mempool.gas_cache_misses");
            counter.increment(1);
            let gas_usage = self
                .client
                .estimate_gas(tx, None)
                .await
                .context("Error estimating gas usage: {}")?;
            self.gas_cache.insert(key, gas_usage);
            let gauge = metrics::gauge!("artemis.executors.mempool.gas_cache_size");
            gauge.set(self.gas_cache.len() as f64);
            Ok(gas_usage)
        } else {
            let counter = metrics::counter!("artemis.executors.mempool.gas_cache_bypassed");
            counter.increment(1);
            self.client
                .estimate_gas(tx, None)
                .await
                .context("Error estimating gas usage: {}")
        }
    }
}

#[async_trait]
impl<M> Executor<SubmitTxToMempool> for MempoolExecutor<M>
where
    M: Middleware,
    M::Error: 'static,
{
    /// Send a transaction to the mempool.
    async fn execute(&self, mut action: SubmitTxToMempool) -> Result<()> {
        let cache_key = GasCacheKey::new(&action.tx);
        let gas_usage = self.estimate_gas_cached(&action.tx, cache_key).await?;

        let bid_gas_price;
        if let Some(gas_bid_info) = action.gas_bid_info {
            // gas price at which we'd break even, meaning 100% of profit goes to validator
            let breakeven_gas_price = gas_bid_info.total_profit / gas_usage;
            // gas price corresponding to bid percentage
            bid_gas_price = breakeven_gas_price
                .mul(gas_bid_info.bid_percentage)
                .div(100);
        } else {
            bid_gas_price = self
                .client
                .get_gas_price()
                .await
                .context("Error getting gas price: {}")?;
        }
        action.tx.set_gas_price(bid_gas_price);
        self.client.send_transaction(action.tx, None).await?;
        Ok(())
    }
}
