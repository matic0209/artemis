#![allow(clippy::too_many_arguments)]

#[cfg(all(feature = "sdk-ethers", feature = "sdk-alloy"))]
compile_error!("Enable only one of `sdk-ethers` or `sdk-alloy` for `mev-share-uni-arb`.");

#[cfg(feature = "sdk-ethers")]
pub use ethers_impl::{MevShareUniArb, V2PoolInfo};

#[cfg(feature = "sdk-alloy")]
pub use alloy_impl::{MevShareUniArb, V2PoolInfo};

#[cfg(feature = "sdk-ethers")]
mod ethers_impl {
    use std::collections::HashMap;
    use std::ops::Add;
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Result;
    use artemis_core::eth::{Address, Hash as H256};
    use artemis_core::eth::{Address as H160, Middleware, Signer, U256};
    use artemis_core::types::Strategy;
    use async_trait::async_trait;
    use mev_share::rpc::{BundleItem, Inclusion, SendBundleRequest};
    use tracing::info;

    use super::super::types::{Action, Event};
    use crate::types::V2V3PoolRecord;
    use mev_share_bindings::blind_arb::BlindArb;

    /// Information about a uniswap v2 pool.
    #[derive(Debug, Clone)]
    pub struct V2PoolInfo {
        /// Address of the v2 pool.
        pub v2_pool: H160,
        /// Whether the pool has weth as token0.
        pub is_weth_token0: bool,
    }

    #[derive(Debug, Clone)]
    pub struct MevShareUniArb<M, S> {
        /// Ethers client.
        client: Arc<M>,
        /// Maps uni v3 pool address to v2 pool information.
        pool_map: HashMap<H160, V2PoolInfo>,
        /// Signer for transactions.
        tx_signer: S,
        /// Arb contract.
        arb_contract: BlindArb<M>,
    }

    impl<M: Middleware + 'static, S: Signer> MevShareUniArb<M, S> {
        /// Create a new instance of the strategy.
        pub fn new(client: Arc<M>, signer: S, arb_contract_address: Address) -> Self {
            Self {
                client: client.clone(),
                pool_map: HashMap::new(),
                tx_signer: signer,
                arb_contract: BlindArb::new(arb_contract_address, client),
            }
        }
    }

    #[async_trait]
    impl<M: Middleware + 'static, S: Signer + 'static> Strategy<Event, Action>
        for MevShareUniArb<M, S>
    {
        /// Initialize the strategy. This is called once at startup, and loads
        /// pool information into memory.
        async fn sync_state(&mut self) -> Result<()> {
            // Read pool information from csv file.
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("resources/v3_v2_pools.csv");
            let mut reader = csv::Reader::from_path(path)?;

            for record in reader.deserialize() {
                // Parse records into PoolRecord struct.
                let record: V2V3PoolRecord = record?;
                self.pool_map.insert(
                    record.v3_pool,
                    V2PoolInfo {
                        v2_pool: record.v2_pool,
                        is_weth_token0: record.weth_token0,
                    },
                );
            }

            Ok(())
        }

        // Process incoming events, seeing if we can arb new orders.
        async fn process_event(&mut self, event: Event) -> Vec<Action> {
            match event {
                Event::MEVShareEvent(event) => {
                    info!("Received mev share event: {:?}", event);
                    // skip if event has no logs
                    if event.logs.is_empty() {
                        return vec![];
                    }
                    let address = event.logs[0].address;
                    // skip if address is not a v3 pool
                    if !self.pool_map.contains_key(&address) {
                        return vec![];
                    }
                    // if it's a v3 pool we care about, submit bundles
                    info!(
                        "Found a v3 pool match at address {:?}, submitting bundles",
                        address
                    );
                    self.generate_bundles(address, event.hash)
                        .await
                        .into_iter()
                        .map(Action::SubmitBundle)
                        .collect()
                }
            }
        }
    }

    impl<M: Middleware + 'static, S: Signer + 'static> MevShareUniArb<M, S> {
        /// Generate a series of bundles of varying sizes to submit to the matchmaker.
        pub async fn generate_bundles(
            &self,
            v3_address: H160,
            tx_hash: H256,
        ) -> Vec<SendBundleRequest> {
            let mut bundles = Vec::new();
            let v2_info = match self.pool_map.get(&v3_address) {
                Some(info) => info.clone(),
                None => return bundles,
            };

            // The sizes of the backruns we want to submit.
            let sizes = vec![
                U256::from(100000_u128),
                U256::from(1000000_u128),
                U256::from(10000000_u128),
                U256::from(100000000_u128),
                U256::from(1000000000_u128),
                U256::from(10000000000_u128),
                U256::from(100000000000_u128),
                U256::from(1000000000000_u128),
                U256::from(10000000000000_u128),
                U256::from(100000000000000_u128),
                U256::from(1000000000000000_u128),
                U256::from(10000000000000000_u128),
                U256::from(100000000000000000_u128),
                U256::from(1000000000000000000_u128),
            ];

            // Set parameters for the backruns.
            let payment_percentage = U256::from(0);
            let bid_gas_price = match self.client.get_gas_price().await {
                Ok(price) => price,
                Err(err) => {
                    tracing::warn!("failed to fetch gas price: {err}");
                    return bundles;
                }
            };
            let block_num = match self.client.get_block_number().await {
                Ok(num) => num,
                Err(err) => {
                    tracing::warn!("failed to fetch block number: {err}");
                    return bundles;
                }
            };

            for size in sizes {
                let arb_tx = {
                    let mut inner = if v2_info.is_weth_token0 {
                        self.arb_contract
                            .execute_arb_weth_token_0(
                                v2_info.v2_pool,
                                v3_address,
                                size,
                                payment_percentage,
                            )
                            .tx
                    } else {
                        self.arb_contract
                            .execute_arb_weth_token_1(
                                v2_info.v2_pool,
                                v3_address,
                                size,
                                payment_percentage,
                            )
                            .tx
                    };
                    inner.set_gas(400000);
                    inner.set_gas_price(bid_gas_price);

                    if let Err(err) = self.client.fill_transaction(&mut inner, None).await {
                        tracing::warn!("error filling tx for size {:?}: {err}", size);
                        continue;
                    }

                    inner
                };
                info!("generated arb tx: {:?}", arb_tx);

                let signature = match self.tx_signer.sign_transaction(&arb_tx).await {
                    Ok(sig) => sig,
                    Err(err) => {
                        tracing::warn!("error signing tx for size {:?}: {err}", size);
                        continue;
                    }
                };
                let bytes = arb_tx.rlp_signed(&signature);
                let bundle = SendBundleRequest {
                    bundle_body: vec![
                        BundleItem::Hash { hash: tx_hash },
                        BundleItem::Tx {
                            tx: bytes,
                            can_revert: false,
                        },
                    ],
                    inclusion: Inclusion {
                        block: block_num.add(1),
                        max_block: Some(block_num.add(30)),
                    },
                    ..Default::default()
                };
                info!("submitting bundle: {:?}", bundle);
                bundles.push(bundle);
            }

            bundles
        }
    }
}

#[cfg(feature = "sdk-alloy")]
mod alloy_impl {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    use alloy_consensus::TxEnvelope;
    use alloy_eips::Encodable2718;
    use alloy_network::{Ethereum, EthereumWallet, IntoWallet, NetworkWallet};
    use alloy_primitives::Address as AlloyAddress;
    use alloy_provider::Provider;
    use alloy_signer::Signer as _;
    use anyhow::Result;
    use artemis_core::eth::{Address as H160, LocalWallet, U256};
    use artemis_core::types::Strategy;
    use async_trait::async_trait;
    use futures::stream::{self, StreamExt};
    use mev_share::rpc::{BundleItem, Inclusion, SendBundleRequest};
    use primitive_types::{H160 as PrimitiveH160, H256 as PrimitiveH256};
    use tracing::{info, warn};

    use super::super::types::{Action, Event};
    use crate::types::V2V3PoolRecord;
    use mev_share_bindings::alloy_blind_arb::BlindArb;

    /// Information about a uniswap v2 pool.
    #[derive(Debug, Clone)]
    pub struct V2PoolInfo {
        /// Address of the v2 pool.
        pub v2_pool: PrimitiveH160,
        /// Whether the pool has weth as token0.
        pub is_weth_token0: bool,
    }

    #[derive(Debug, Clone)]
    pub struct MevShareUniArb<P> {
        /// Alloy provider.
        provider: Arc<P>,
        /// Maps uni v3 pool address to v2 pool information.
        pool_map: HashMap<PrimitiveH160, V2PoolInfo>,
        /// Signer for transactions.
        tx_signer: Arc<LocalWallet>,
        /// Arb contract.
        arb_contract: BlindArb::BlindArbInstance<Arc<P>>,
    }

    impl<P> MevShareUniArb<P>
    where
        P: Provider<Ethereum> + Send + Sync + Clone + 'static,
    {
        /// Create a new instance of the strategy.
        pub fn new(provider: Arc<P>, signer: LocalWallet, arb_contract_address: H160) -> Self {
            let arb_contract = BlindArb::new(arb_contract_address, provider.clone());
            Self {
                provider,
                pool_map: HashMap::new(),
                tx_signer: Arc::new(signer),
                arb_contract,
            }
        }
    }

    #[async_trait]
    impl<P> Strategy<Event, Action> for MevShareUniArb<P>
    where
        P: Provider<Ethereum> + Send + Sync + Clone + 'static,
    {
        async fn sync_state(&mut self) -> Result<()> {
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("resources/v3_v2_pools.csv");
            let mut reader = csv::Reader::from_path(path)?;

            for record in reader.deserialize() {
                let record: V2V3PoolRecord = record?;
                let v3_key = PrimitiveH160::from_slice(record.v3_pool.as_slice());
                let v2_pool = PrimitiveH160::from_slice(record.v2_pool.as_slice());
                self.pool_map.insert(
                    v3_key,
                    V2PoolInfo {
                        v2_pool,
                        is_weth_token0: record.weth_token0,
                    },
                );
            }

            Ok(())
        }

        async fn process_event(&mut self, event: Event) -> Vec<Action> {
            match event {
                Event::MEVShareEvent(event) => {
                    info!("Received mev share event: {:?}", event);
                    if event.logs.is_empty() {
                        return vec![];
                    }
                    let address = PrimitiveH160::from_slice(event.logs[0].address.as_ref());
                    if !self.pool_map.contains_key(&address) {
                        return vec![];
                    }
                    let tx_hash = PrimitiveH256::from_slice(event.hash.as_ref());
                    info!(
                        "Found a v3 pool match at address {:?}, submitting bundles",
                        address
                    );
                    self.generate_bundles(address, tx_hash)
                        .await
                        .into_iter()
                        .map(Action::SubmitBundle)
                        .collect()
                }
            }
        }
    }

    impl<P> MevShareUniArb<P>
    where
        P: Provider<Ethereum> + Send + Sync + Clone + 'static,
    {
        /// Generate a series of bundles of varying sizes to submit to the matchmaker.
        pub async fn generate_bundles(
            &self,
            v3_address: PrimitiveH160,
            tx_hash: PrimitiveH256,
        ) -> Vec<SendBundleRequest> {
            let v2_info = match self.pool_map.get(&v3_address) {
                Some(info) => info.clone(),
                None => return vec![],
            };

            let signer_address = self.tx_signer.address();

            let sizes = vec![
                U256::from(100000_u128),
                U256::from(1000000_u128),
                U256::from(10000000_u128),
                U256::from(100000000_u128),
                U256::from(1000000000_u128),
                U256::from(10000000000_u128),
                U256::from(100000000000_u128),
                U256::from(1000000000000_u128),
                U256::from(10000000000000_u128),
                U256::from(100000000000000_u128),
                U256::from(1000000000000000_u128),
                U256::from(10000000000000000_u128),
                U256::from(100000000000000000_u128),
                U256::from(1000000000000000000_u128),
            ];

            let payment_percentage = U256::ZERO;

            let gas_price = match self.provider.get_gas_price().await {
                Ok(price) => price,
                Err(err) => {
                    warn!("failed to fetch gas price: {err}");
                    return vec![];
                }
            };

            let block_number = match self.provider.get_block_number().await {
                Ok(num) => num,
                Err(err) => {
                    warn!("failed to fetch block number: {err}");
                    return vec![];
                }
            };

            let chain_id = match self.provider.get_chain_id().await {
                Ok(id) => id,
                Err(err) => {
                    warn!("failed to fetch chain id: {err}");
                    return vec![];
                }
            };

            let nonce = match self.provider.get_transaction_count(signer_address).await {
                Ok(nonce) => nonce,
                Err(err) => {
                    warn!("failed to fetch nonce: {err}");
                    return vec![];
                }
            };

            let mut signer = (*self.tx_signer).clone();
            signer.set_chain_id(Some(chain_id.into()));
            let wallet: Arc<EthereumWallet> =
                Arc::new(<LocalWallet as IntoWallet<Ethereum>>::into_wallet(signer));

            let v3_address_alloy = AlloyAddress::from_slice(v3_address.as_bytes());
            let block_start = block_number + 1;
            let block_end = block_number + 30;
            let arb_contract = self.arb_contract.clone();

            stream::iter(sizes.into_iter().map(move |size| {
                let wallet = wallet.clone();
                let v2_info = v2_info.clone();
                let arb_contract = arb_contract.clone();
                async move {
                    let v2_address_alloy = AlloyAddress::from_slice(v2_info.v2_pool.as_bytes());
                    let mut tx = if v2_info.is_weth_token0 {
                        arb_contract
                            .executeArb__WETH_token0(
                                v2_address_alloy,
                                v3_address_alloy,
                                size,
                                payment_percentage,
                            )
                            .into_transaction_request()
                    } else {
                        arb_contract
                            .executeArb__WETH_token1(
                                v2_address_alloy,
                                v3_address_alloy,
                                size,
                                payment_percentage,
                            )
                            .into_transaction_request()
                    };
                    tx.gas = Some(400_000);
                    tx.gas_price = Some(gas_price);
                    tx.from = Some(signer_address);
                    tx.nonce = Some(nonce);
                    tx.chain_id = Some(chain_id.into());

                    let envelope: TxEnvelope =
                        match NetworkWallet::<Ethereum>::sign_request(&*wallet, tx.clone()).await {
                            Ok(envelope) => envelope,
                            Err(err) => {
                                warn!("error signing tx for size {:?}: {err}", size);
                                return None;
                            }
                        };
                    let bytes: Vec<u8> = envelope.encoded_2718();

                    let bundle = SendBundleRequest {
                        bundle_body: vec![
                            BundleItem::Hash { hash: tx_hash },
                            BundleItem::Tx {
                                tx: bytes.into(),
                                can_revert: false,
                            },
                        ],
                        inclusion: Inclusion {
                            block: block_start.into(),
                            max_block: Some(block_end.into()),
                        },
                        ..Default::default()
                    };
                    info!("prepared bundle for size {:?}", size);
                    Some(bundle)
                }
            }))
            .buffer_unordered(4)
            .filter_map(|bundle| async move { bundle })
            .collect()
            .await
        }
    }
}
