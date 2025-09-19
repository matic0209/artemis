//! Alloy adapter scaffolding.
//!
//! We keep these helpers separate so downstream modules can start importing
//! Alloy primitives while we gradually migrate off ethers-rs.

#[cfg(feature = "sdk-alloy")]
pub use alloy_primitives::{Address, Bytes, B256 as Hash, U256};

#[cfg(feature = "sdk-alloy")]
pub use alloy_rpc_types_eth::{BlockId, BlockNumberOrTag, Filter, Log, Transaction};

#[cfg(feature = "sdk-alloy")]
pub use alloy_provider::RootProvider as Provider;

#[cfg(feature = "sdk-alloy")]
pub use alloy_signer_local::PrivateKeySigner as LocalWallet;

/// Convenience alias to mirror `ethers::types::U64` in examples/tests.
#[cfg(feature = "sdk-alloy")]
pub type U64 = alloy_primitives::U64;

#[cfg(feature = "sdk-alloy")]
pub type TxRequest = alloy_rpc_types_eth::transaction::TransactionRequest;

#[cfg(feature = "sdk-alloy")]
pub mod helpers {
    use super::*;
    use alloy_primitives::keccak256;
    use alloy_provider::Provider as ProviderTrait;
    use anyhow::{anyhow, Result};
    use std::str::FromStr;

    pub async fn create_ws_provider(url: &str) -> Result<Provider> {
        super::Provider::connect(url)
            .await
            .map_err(|err| anyhow!("failed to connect alloy ws provider: {err}"))
    }

    pub async fn create_http_provider(url: &str) -> Result<Provider> {
        super::Provider::connect(url)
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
}

// TODO: once collectors/executors are migrated, provide subscription helpers
// (blocks, logs, pending transactions, etc.) that mirror the ethers-based APIs.
