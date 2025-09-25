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

    use crate::types::{Action, Event};
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
        pub provider: Arc<P>,
        /// Maps uni v3 pool address to v2 pool information.
        pub pool_map: HashMap<PrimitiveH160, V2PoolInfo>,
        /// Signer for transactions.
        pub tx_signer: Arc<LocalWallet>,
        /// Arb contract.
        pub arb_contract: BlindArb::BlindArbInstance<Arc<P>>,
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
