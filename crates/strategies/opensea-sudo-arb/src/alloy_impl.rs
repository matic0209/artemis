    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashMap};
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::time::Instant;

    use alloy_primitives::{keccak256, Address as AlloyAddress};
    use alloy_provider::Provider;
    use alloy_rpc_types_eth::{
        state::{AccountOverride, StateOverride},
        Filter,
    };
    use anyhow::{anyhow, Context, Result};
    use artemis_core::eth::{Address as H160, Hash as H256, U256};
    use artemis_core::executors::mempool_types::{GasBidInfo, SubmitTxToMempool};
    use artemis_core::types::Strategy;
    use async_trait::async_trait;
    use bindings::{
        bytecode::SUDO_PAIR_QUOTER_BYTECODE, LSSVMPairFactory, SellQuote, SudoOpenseaArb,
        SudoPairQuoter,
    };
    use futures::stream::{self, StreamExt};
    use lru::LruCache;
    use metrics::{counter, histogram};
    use opensea_stream::schema::Chain;
    use opensea_v2::client::OpenSeaV2Client;
    use opensea_v2::types::FulfillListingResponse;
    use parking_lot::Mutex;
    use tracing::info;

    use crate::constants::{LSSVM_PAIR_FACTORY_ADDRESS, POOL_EVENT_SIGNATURES};
    use crate::types::{
        fulfill_listing_response_to_basic_order_parameters, hash_to_fulfill_listing_request, Action,
        Config, Event, to_alloy_address, from_alloy_address, to_alloy_u256, from_alloy_u256,
    };

    const ORDER_CACHE_CAPACITY: usize = 256;
    const QUOTE_CONCURRENCY: usize = 4;
    const QUOTE_BATCH_SIZE: usize = 128;
    const RANGE_CHUNK_SIZE: u64 = 1_500;

    #[derive(Debug, Clone)]
    pub struct OpenseaSudoArb<P> {
        provider: Arc<P>,
        opensea_client: OpenSeaV2Client,
        arb_contract_address: H160,
        sudo_pools: HashMap<H160, BinaryHeap<PoolBidEntry>>,
        pool_bids: HashMap<H160, U256>,
        bid_percentage: u64,
        order_cache: Arc<Mutex<LruCache<H256, Arc<FulfillListingResponse>>>>,
        sudo_pair_quoter_bytecode: Vec<u8>,
    }

    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    struct PoolBidEntry {
        pool: H160,
        bid: U256,
    }

    impl Ord for PoolBidEntry {
        fn cmp(&self, other: &Self) -> Ordering {
            self.bid.cmp(&other.bid)
        }
    }

    impl PartialOrd for PoolBidEntry {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl<P> OpenseaSudoArb<P>
    where
        P: Provider + Clone + Send + Sync + 'static,
    {
        pub fn new(provider: Arc<P>, opensea_client: OpenSeaV2Client, config: Config) -> Self {
            let order_cache = Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(ORDER_CACHE_CAPACITY).unwrap(),
            )));

            let sudo_pair_quoter_bytecode = SUDO_PAIR_QUOTER_BYTECODE.to_vec();

            Self {
                provider,
                opensea_client,
                arb_contract_address: config.arb_contract_address,
                sudo_pools: HashMap::new(),
                pool_bids: HashMap::new(),
                bid_percentage: config.bid_percentage,
                order_cache,
                sudo_pair_quoter_bytecode,
            }
        }

        fn address_from_u64(value: u64) -> H160 {
            let mut bytes = [0u8; 20];
            bytes[12..].copy_from_slice(&value.to_be_bytes());
            H160::from_slice(&bytes)
        }

        fn best_bid_for(&mut self, nft_address: &H160) -> Option<(H160, U256)> {
            let heap = self.sudo_pools.get_mut(nft_address)?;
            while let Some(entry) = heap.peek() {
                let pool = entry.pool;
                let bid = entry.bid;
                match self.pool_bids.get(&pool) {
                    Some(current) if *current == bid => return Some((pool, bid)),
                    _ => {
                        heap.pop();
                    }
                }
            }
            None
        }

        async fn get_opensea_order(&self, order_hash: H256) -> Option<Arc<FulfillListingResponse>> {
            if let Some(order) = self.order_cache.lock().get(&order_hash).cloned() {
                counter!("artemis.strategy.opensea_sudo.order_cache_hits").increment(1);
                return Some(order);
            }

            counter!("artemis.strategy.opensea_sudo.order_cache_misses").increment(1);
            match self
                .opensea_client
                .fulfill_listing(hash_to_fulfill_listing_request(order_hash))
                .await
            {
                Ok(order) => {
                    let order = Arc::new(order);
                    self.order_cache.lock().put(order_hash, Arc::clone(&order));
                    Some(order)
                }
                Err(err) => {
                    info!("Error getting order from opensea: {}", err);
                    None
                }
            }
        }

        fn build_arb_tx(
            &self,
            order: &FulfillListingResponse,
            sudo_pool: H160,
            sudo_bid: U256,
        ) -> Option<Action> {
            let payment_value: U256 = U256::from(order.fulfillment_data.transaction.value);
            let total_profit = sudo_bid.checked_sub(payment_value)?;

            let basic_order = fulfill_listing_response_to_basic_order_parameters(order);
            let payment_value_alloy = to_alloy_u256(&payment_value);
            let pool_alloy = to_alloy_address(sudo_pool);
            let contract_address = to_alloy_address(self.arb_contract_address);

            let mut tx = SudoOpenseaArb::new(contract_address, self.provider.clone())
                .executeArb(basic_order, payment_value_alloy, pool_alloy)
                .into_transaction_request();

            tx.value = Some(payment_value_alloy);
            if tx.chain_id.is_none() {
                tx.chain_id = Some(order.fulfillment_data.transaction.chain);
            }

            Some(Action::SubmitTx(SubmitTxToMempool {
                tx,
                gas_bid_info: Some(GasBidInfo {
                    total_profit,
                    bid_percentage: self.bid_percentage,
                }),
            }))
        }

        async fn get_quotes_for_pools(&self, pools: Vec<H160>) -> Result<Vec<(H160, SellQuote)>> {
            if pools.is_empty() {
                return Ok(vec![]);
            }

            let override_address = to_alloy_address(Self::address_from_u64(0x5eed));
            let provider = self.provider.clone();
            let bytecode = Arc::new(self.sudo_pair_quoter_bytecode.clone());

            let chunks = pools
                .chunks(QUOTE_BATCH_SIZE)
                .map(|chunk| chunk.to_vec())
                .collect::<Vec<_>>();

            let mut stream = stream::iter(chunks.into_iter().map(move |chunk_vec| {
                let provider = provider.clone();
                let bytecode = bytecode.clone();
                async move {
                    let mut override_state = StateOverride::default();
                    override_state.insert(
                        override_address,
                        AccountOverride::default().with_code((*bytecode).clone()),
                    );

                    let alloy_addresses = chunk_vec
                        .iter()
                        .cloned()
                        .map(to_alloy_address)
                        .collect::<Vec<_>>();

                    let quotes = SudoPairQuoter::new(override_address, provider.clone())
                        .getMultipleSellQuotes(alloy_addresses)
                        .state(override_state)
                        .call()
                        .await
                        .map_err(|err| anyhow!("failed to fetch sudo quotes: {err}"))?;

                    Ok::<_, anyhow::Error>(chunk_vec.into_iter().zip(quotes).collect::<Vec<_>>())
                }
            }))
            .buffer_unordered(QUOTE_CONCURRENCY);

            let mut results = Vec::with_capacity(pools.len());
            while let Some(batch) = stream.next().await {
                match batch {
                    Ok(mut entries) => results.append(&mut entries),
                    Err(err) => return Err(err),
                }
            }

            Ok(results)
        }

        fn update_internal_pool_state(&mut self, pools_and_quotes: Vec<(H160, SellQuote)>) {
            let start = Instant::now();
            let mut quotes_available = 0;
            let mut quotes_removed = 0;
            
            for (pool_address, quote) in pools_and_quotes {
                if quote.quoteAvailable {
                    let price = from_alloy_u256(quote.price);
                    self.pool_bids.insert(pool_address, price);
                    self.sudo_pools
                        .entry(from_alloy_address(quote.nftAddress))
                        .or_insert_with(BinaryHeap::new)
                        .push(PoolBidEntry {
                            pool: pool_address,
                            bid: price,
                        });
                    quotes_available += 1;
                } else {
                    self.pool_bids.remove(&pool_address);
                    quotes_removed += 1;
                }
            }
            
            let elapsed = start.elapsed();
            histogram!("artemis.strategies.opensea.state_update_duration").record(elapsed.as_millis() as f64);
            counter!("artemis.strategies.opensea.quotes_available").increment(quotes_available);
            counter!("artemis.strategies.opensea.quotes_removed").increment(quotes_removed);
        }

        #[allow(dead_code)]
        async fn get_touched_pools(&self, from_block: u64, to_block: u64) -> Result<Vec<H160>> {
            if to_block < from_block {
                return Ok(Vec::new());
            }

            let addresses: Vec<AlloyAddress> = self
                .pool_bids
                .keys()
                .cloned()
                .map(to_alloy_address)
                .collect();
            if addresses.is_empty() {
                return Ok(Vec::new());
            }

            let topics: Vec<_> = POOL_EVENT_SIGNATURES.iter().copied().collect();

            let filter = Filter::new()
                .from_block(from_block)
                .to_block(to_block)
                .address(addresses)
                .events(topics);

            let logs = self
                .provider
                .get_logs(&filter)
                .await
                .map_err(|err| anyhow!("failed to fetch touched pools: {err}"))?;

            Ok(logs
                .into_iter()
                .map(|log| from_alloy_address(log.address()))
                .collect())
        }

        async fn get_new_pools(&self, from_block: u64, to_block: u64) -> Result<Vec<H160>> {
            if to_block < from_block {
                return Ok(Vec::new());
            }

            let mut ranges = Vec::new();
            let mut start = from_block;
            while start <= to_block {
                let end = (start + RANGE_CHUNK_SIZE).min(to_block);
                ranges.push((start, end));
                if end == to_block {
                    break;
                }
                start = end + 1;
            }

            let topic = keccak256(b"NewPair(address)");
            let factory_address = to_alloy_address(*LSSVM_PAIR_FACTORY_ADDRESS);
            let provider = self.provider.clone();

            let mut stream = stream::iter(ranges.into_iter().map(move |(start, end)| {
                let provider = provider.clone();
                async move {
                    let filter = Filter::new()
                        .from_block(start)
                        .to_block(end)
                        .address(vec![factory_address])
                        .event_signature(topic);

                    let logs = provider
                        .get_logs(&filter)
                        .await
                        .map_err(|err| anyhow!("failed to fetch new pools: {err}"))?;

                    let pools = logs
                        .into_iter()
                        .filter_map(|log| {
                            log.log_decode::<LSSVMPairFactory::NewPair>()
                                .ok()
                                .map(|decoded| from_alloy_address(decoded.inner.data.poolAddress))
                        })
                        .collect::<Vec<_>>();

                    Ok::<_, anyhow::Error>(pools)
                }
            }))
            .buffer_unordered(QUOTE_CONCURRENCY);

            let mut pools = Vec::new();
            while let Some(batch) = stream.next().await {
                match batch {
                    Ok(mut addresses) => pools.append(&mut addresses),
                    Err(err) => return Err(err),
                }
            }

            Ok(pools)
        }

    }
    #[async_trait]
    impl<P> Strategy<Event, Action> for OpenseaSudoArb<P>
    where
        P: Provider + Clone + Send + Sync + 'static,
    {
        async fn sync_state(&mut self) -> Result<()> {
            let start_block = crate::constants::FACTORY_DEPLOYMENT_BLOCK;
            let current_block = self
                .provider
                .get_block_number()
                .await
                .context("failed to fetch latest block number")?;
            let current_block = current_block.max(start_block);

            let pool_addresses = self.get_new_pools(start_block, current_block).await?;
            info!("found {} deployed sudo pools", pool_addresses.len());

            if !pool_addresses.is_empty() {
                let timer = Instant::now();
                let quotes = self.get_quotes_for_pools(pool_addresses).await?;
                histogram!("artemis.strategy.opensea_sudo.sync_quote_ms")
                    .record(timer.elapsed().as_secs_f64() * 1_000.0);
                self.update_internal_pool_state(quotes);
            }
            info!(
                "done syncing state, found available pools for {} collections",
                self.sudo_pools.len()
            );

            Ok(())
        }

        async fn process_event(&mut self, event: Event) -> Vec<Action> {
            match event {
                Event::OpenseaOrder(order) => {
                    let nft_address =
                        H160::from_slice(order.listing.context.item.nft_id.address.as_bytes());
                    info!("processing order event for address {}", nft_address);

                    if !matches!(order.listing.context.item.nft_id.network, Chain::Ethereum) {
                        return vec![];
                    }
                    let payment_token_address =
                        H160::from_slice(order.listing.payment_token.address.as_bytes());
                    if payment_token_address != H160::ZERO {
                        return vec![];
                    }

                    let (max_pool, max_bid) = match self.best_bid_for(&nft_address) {
                        Some(result) => result,
                        None => return vec![],
                    };

                    let mut price_bytes = [0u8; 32];
                    order.listing.base_price.to_big_endian(&mut price_bytes);
                    let base_price = U256::from_be_bytes(price_bytes);

                    if max_bid <= base_price {
                        return vec![];
                    }

                    let timer = Instant::now();
                    let order_hash = H256::from_slice(order.listing.order_hash.as_bytes());
                    let order_resp = match self.get_opensea_order(order_hash).await {
                        Some(resp) => resp,
                        None => return vec![],
                    };
                    histogram!("artemis.strategy.opensea_sudo.order_fetch_ms")
                        .record(timer.elapsed().as_secs_f64() * 1_000.0);

                    self.build_arb_tx(&order_resp, max_pool, max_bid)
                        .into_iter()
                        .collect()
                }
                Event::NewBlock(block) => {
                    info!("processing new block {}", block.number);
                    vec![]
                }
            }
        }
    }
