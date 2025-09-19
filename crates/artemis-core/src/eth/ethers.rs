#[cfg(feature = "sdk-ethers")]
pub use ethers::{
    prelude::{Middleware, MiddlewareBuilder},
    providers::{Http, Provider, PubsubClient, Ws},
    signers::{LocalWallet, Signer},
    types::{BlockId, Bytes, Filter, Log, Transaction, H160 as Address, H256 as Hash, U256, U64},
};

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

    pub async fn sign_transaction(
        signer: &LocalWallet,
        tx: &TxRequest,
    ) -> Result<ethers::types::Signature> {
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
