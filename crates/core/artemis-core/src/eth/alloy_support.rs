//! Alloy adapter scaffolding.
//!
//! We keep these helpers separate so downstream modules can start importing
//! Alloy primitives while we gradually migrate off ethers-rs.

pub use alloy_primitives::{Address, Bytes, B256 as Hash, U256};
pub use alloy_rpc_types_eth::{BlockId, BlockNumberOrTag, Filter, Log, Transaction};
pub use alloy_provider::RootProvider as Provider;
pub use alloy_signer_local::PrivateKeySigner as LocalWallet;

/// Convenience alias to mirror `ethers::types::U64` in examples/tests.
pub type U64 = alloy_primitives::U64;
pub type TxRequest = alloy_rpc_types_eth::transaction::TransactionRequest;

pub mod helpers {
    use super::*;
    use alloy_primitives::keccak256;
    use alloy_provider::Provider as ProviderTrait;
    use anyhow::{anyhow, Result};
    use std::str::FromStr;

    pub async fn create_ws_provider(url: &str) -> Result<Provider> {
        Provider::connect(url)
            .await
            .map_err(|err| anyhow!("failed to connect alloy ws provider: {err}"))
    }

    pub async fn create_http_provider(url: &str) -> Result<Provider> {
        Provider::connect(url)
            .await
            .map_err(|err| anyhow!("failed to connect alloy http provider: {err}"))
    }

    pub fn parse_local_wallet(pk: &str) -> Result<LocalWallet> {
        LocalWallet::from_str(pk).map_err(|err| anyhow!("failed to parse wallet: {err}"))
    }

    pub fn attach_signer(provider: Provider, signer: LocalWallet) -> (Provider, LocalWallet) {
        (provider, signer)
    }

    pub async fn get_block_number(provider: &Provider) -> Result<U64> {
        ProviderTrait::get_block_number(provider)
            .await
            .map(U64::from)
            .map_err(|err| anyhow!("failed to fetch block number: {err}"))
    }

    pub fn keccak256_hash(data: impl AsRef<[u8]>) -> Hash {
        keccak256(data.as_ref())
    }

    pub async fn get_chain_id(provider: &Provider) -> Result<u64> {
        Ok(ProviderTrait::get_chain_id(provider).await?)
    }

    pub async fn sign_transaction_alloy(_signer: &LocalWallet, _tx: &TxRequest) -> Result<String> {
        // TODO: Implement proper Alloy transaction signing
        // The signing API in Alloy is different from ethers
        Err(anyhow!("Alloy transaction signing not yet implemented"))
    }

    pub async fn send_transaction_alloy(provider: &Provider, tx: TxRequest) -> Result<()> {
        let _pending = ProviderTrait::send_transaction(provider, tx).await?;
        Ok(())
    }

    pub async fn estimate_gas_alloy(provider: &Provider, tx: &TxRequest) -> Result<U256> {
        let gas = ProviderTrait::estimate_gas(provider, tx.clone()).await?;
        Ok(U256::from(gas))
    }

    /// Get multiple balances concurrently for better performance
    pub async fn get_balances_batch(
        provider: &Provider,
        addresses: &[Address],
        block: Option<BlockNumberOrTag>,
    ) -> Result<Vec<U256>> {
        use futures::future::try_join_all;
        
        let balance_futures: Vec<_> = addresses.iter()
            .map(|&addr| async move {
                let balance = ProviderTrait::get_balance(provider, addr).await;
                balance.map_err(|e| anyhow!("failed to get balance: {e}"))
            })
            .collect();
            
        try_join_all(balance_futures).await
    }
}

// TODO: once collectors/executors are migrated, provide subscription helpers
// (blocks, logs, pending transactions, etc.) that mirror the ethers-based APIs.
