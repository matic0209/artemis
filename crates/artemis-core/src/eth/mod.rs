//! SDK adapter layer.
//!
//! Default feature `sdk-ethers` re-exports ethers types so downstream crates can
//! depend on this module instead of directly depending on a specific SDK.
//! This enables a future switch to Alloy behind the `sdk-alloy` feature with
//! minimal surface changes.

#[cfg(feature = "sdk-ethers")]
pub use ethers::{
    prelude::{Middleware, MiddlewareBuilder},
    providers::{Http, Provider, PubsubClient, Ws},
    signers::{LocalWallet, Signer},
    types::{Bytes, Filter, Log, Transaction, BlockId, H160 as Address, H256 as Hash, U256, U64},
};

// Placeholder re-exports for the future Alloy migration. Left intentionally
// minimal for now to avoid unused dependencies when the feature is disabled.
#[cfg(feature = "sdk-alloy")]
pub mod alloy_support {
    // Narrow alloy re-exports live under a namespaced module to avoid name collisions
    // when both `sdk-ethers` and `sdk-alloy` features are enabled simultaneously.
    pub use alloy_primitives::{Address, B256 as Hash, Bytes, U256};
    pub use alloy_provider::RootProvider as Provider;
    pub use alloy_signer_local::PrivateKeySigner as LocalWallet;
    // Convenience alias to mirror ethers::types::U64 where needed in examples.
    pub type U64 = u64;

    pub mod helpers {
        use super::*;
        use anyhow::Result;
        use std::str::FromStr;

        pub async fn create_ws_provider(url: &str) -> Result<Provider> {
            Ok(Provider::connect(url).await?)
        }

        pub async fn create_http_provider(url: &str) -> Result<Provider> {
            Ok(Provider::connect(url).await?)
        }

        pub fn parse_local_wallet(pk: &str) -> Result<LocalWallet> {
            Ok(LocalWallet::from_str(pk)?)
        }

        pub fn attach_signer(provider: Provider, signer: LocalWallet) -> (Provider, LocalWallet) {
            (provider, signer)
        }

        pub async fn get_block_number(provider: &Provider) -> Result<U64> {
            use alloy_provider::Provider as _;
            Ok(provider.get_block_number().await?)
        }
    }
}

#[cfg(feature = "sdk-ethers")]
pub mod helpers {
    use super::*;
    use anyhow::Result;

    pub async fn create_ws_provider(url: &str) -> Result<Provider<Ws>> {
        let ws = Ws::connect(url).await?;
        Ok(Provider::new(ws))
    }

    pub fn create_http_provider(url: &str) -> Result<Provider<Http>> {
        Ok(Provider::try_from(url)?)
    }

    pub fn parse_local_wallet(pk: &str) -> Result<LocalWallet> {
        Ok(pk.parse()?)
    }

    pub async fn get_chain_id<M: Middleware>(client: &M) -> Result<u64>
    where
        <M as ethers::providers::Middleware>::Error: 'static,
    {
        Ok(client.get_chainid().await?.as_u64())
    }

    pub async fn estimate_gas<'a, M: Middleware>(client: &M, tx: &'a TxRequest) -> Result<U256>
    where
        <M as ethers::providers::Middleware>::Error: 'static,
    {
        Ok(client.estimate_gas(tx, None).await?)
    }

    pub async fn get_gas_price<M: Middleware>(client: &M) -> Result<U256>
    where
        <M as ethers::providers::Middleware>::Error: 'static,
    {
        Ok(client.get_gas_price().await?)
    }

    pub async fn sign_transaction(signer: &LocalWallet, tx: &TxRequest) -> Result<ethers::types::Signature> {
        Ok(signer.sign_transaction(tx).await?)
    }

    pub async fn send_transaction<M: Middleware>(client: &M, tx: TxRequest) -> Result<()>
    where
        <M as ethers::providers::Middleware>::Error: 'static,
    {
        let _pending = client.send_transaction(tx, None).await?;
        Ok(())
    }
}

/// SDK-agnostic alias for a transaction request type used by executors.
#[cfg(feature = "sdk-ethers")]
pub type TxRequest = ethers::types::transaction::eip2718::TypedTransaction;

