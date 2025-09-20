#![allow(clippy::too_many_arguments)]

#[cfg(all(feature = "sdk-ethers", feature = "sdk-alloy"))]
compile_error!("Enable only one of `sdk-ethers` or `sdk-alloy` for `opensea-sudo-arb`.");

#[cfg(feature = "sdk-ethers")]
pub use ethers_impl::OpenseaSudoArb;

#[cfg(feature = "sdk-alloy")]
pub use alloy_impl::OpenseaSudoArb;

#[cfg(feature = "sdk-ethers")]
mod ethers_impl {
    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashMap};
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::time::Instant;

    use anyhow::Result;
    use artemis_core::collectors::block_collector::NewBlock;
    use artemis_core::collectors::opensea_order_collector::OpenseaOrder;
    use artemis_core::eth::{Address as H160, Filter, Hash as H256, Middleware, U256};
    use artemis_core::executors::mempool_types::{GasBidInfo, SubmitTxToMempool};
    use artemis_core::types::Strategy;
    use artemis_core::utilities::state_override_middleware::StateOverrideMiddleware;
    use async_trait::async_trait;
    use bindings::lssvm_pair_factory::{LSSVMPairFactory, NewPairFilter};
    use bindings::sudo_opensea_arb::SudoOpenseaArb;
    use bindings::sudo_pair_quoter::{SellQuote, SudoPairQuoter, SUDOPAIRQUOTER_DEPLOYED_BYTECODE};
    use futures::stream::{self, StreamExt};
    use lru::LruCache;
    use opensea_stream::schema::Chain;
    use opensea_v2::client::OpenSeaV2Client;
    use opensea_v2::types::FulfillListingResponse;
    use parking_lot::Mutex;
    use tracing::info;

    use super::super::constants::{LSSVM_PAIR_FACTORY_ADDRESS, POOL_EVENT_SIGNATURES};
    use super::super::types::{
        fulfill_listing_response_to_basic_order_parameters, hash_to_fulfill_listing_request,
        Action, Config, Event,
    };

    const ORDER_CACHE_CAPACITY: usize = 256;
    const QUOTE_CONCURRENCY: usize = 4;
    const QUOTE_BATCH_SIZE: usize = 128;
    const RANGE_CHUNK_SIZE: u64 = 1_500;

    #[derive(Debug, Clone)]
    pub struct OpenseaSudoArb<M> {
        /// Ethers client.
        client: Arc<M>,
        /// Opensea V2 client
        opensea_client: OpenSeaV2Client,
        /// LSSVM pair factory contract for getting pair history.
        lssvm_pair_factory: Arc<LSSVMPairFactory<M>>,
        /// Quoter for batch reading pair state.
        quoter: SudoPairQuoter<StateOverrideMiddleware<Arc<M>>>,
        /// Arb contract.
        arb_contract: SudoOpenseaArb<M>,
        /// Map NFT addresses to a max-heap of Sudo pool bids.
        sudo_pools: HashMap<H160, BinaryHeap<PoolBidEntry>>,
        /// Map Sudo pool addresses to the current bid for that pool (in ETH).
        pool_bids: HashMap<H160, U256>,
        /// Amount of profits to bid in gas
        bid_percentage: u64,
        /// In-memory cache for fulfill listing responses to avoid spamming OpenSea API.
        order_cache: Arc<Mutex<LruCache<H256, Arc<FulfillListingResponse>>>>,
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

    impl<M: Middleware + 'static> OpenseaSudoArb<M> {
        pub fn new(client: Arc<M>, opensea_client: OpenSeaV2Client, config: Config) -> Self {
            // Set up LSSVM pair factory contract.
            let lssvm_pair_factory = Arc::new(LSSVMPairFactory::new(
                *LSSVM_PAIR_FACTORY_ADDRESS,
                client.clone(),
            ));
            // Set up Sudo pair quoter contract.
            let mut state_override = StateOverrideMiddleware::new(client.clone());
            let addr = state_override.add_code(SUDOPAIRQUOTER_DEPLOYED_BYTECODE.clone());
            let quoter = SudoPairQuoter::new(addr, Arc::new(state_override));
            // Set up arb contract.
            let arb_contract = SudoOpenseaArb::new(config.arb_contract_address, client.clone());
            let order_cache = Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(ORDER_CACHE_CAPACITY).unwrap(),
            )));

            Self {
                client,
                opensea_client,
                lssvm_pair_factory,
                quoter,
                arb_contract,
                sudo_pools: HashMap::new(),
                pool_bids: HashMap::new(),
                bid_percentage: config.bid_percentage,
                order_cache,
            }
        }
    }

    #[async_trait]
    impl<M: Middleware + 'static> Strategy<Event, Action> for OpenseaSudoArb<M> {
        async fn sync_state(&mut self) -> Result<()> {
            let start_block = super::super::constants::FACTORY_DEPLOYMENT_BLOCK;

            let current_block = self.client.get_block_number().await?.as_u64();

            let pool_addresses = self.get_new_pools(start_block, current_block).await?;
            info!("found {} deployed sudo pools", pool_addresses.len());

            if !pool_addresses.is_empty() {
                let timer = Instant::now();
                let quotes = self.get_quotes_for_pools(pool_addresses).await?;
                let histogram = metrics::histogram!("artemis.strategy.opensea_sudo.sync_quote_ms");
                histogram.record(timer.elapsed().as_secs_f64() * 1_000.0);
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
                Event::OpenseaOrder(order) => self
                    .process_order_event(*order)
                    .await
                    .map_or(vec![], |a| vec![a]),
                Event::NewBlock(block) => match self.process_new_block_event(block).await {
                    Ok(_) => vec![],
                    Err(e) => {
                        panic!("Strategy is out of sync {}", e);
                    }
                },
            }
        }
    }

    impl<M: Middleware + 'static> OpenseaSudoArb<M> {
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

        async fn process_order_event(&mut self, event: OpenseaOrder) -> Option<Action> {
            let nft_address = event.listing.context.item.nft_id.address;
            info!("processing order event for address {}", nft_address);

            match event.listing.context.item.nft_id.network {
                Chain::Ethereum => {}
                _ => return None,
            }
            if event.listing.payment_token.address != H160::zero() {
                return None;
            }

            let (max_pool, max_bid) = self.best_bid_for(&nft_address)?;

            if max_bid <= event.listing.base_price {
                return None;
            }

            let timer = Instant::now();
            let order = match self.get_opensea_order(event.listing.order_hash).await {
                Some(order) => order,
                None => return None,
            };
            let histogram = metrics::histogram!("artemis.strategy.opensea_sudo.order_fetch_ms");
            histogram.record(timer.elapsed().as_secs_f64() * 1_000.0);

            self.build_arb_tx(&order, max_pool, max_bid)
        }

        async fn process_new_block_event(&mut self, event: NewBlock) -> Result<()> {
            info!("processing new block {}", event.number);
            let new_pools = self
                .get_new_pools(event.number.as_u64(), event.number.as_u64())
                .await?;
            let touched_pools = self
                .get_touched_pools(event.number.as_u64(), event.number.as_u64())
                .await?;
            let quotes = self
                .get_quotes_for_pools([new_pools, touched_pools].concat())
                .await?;
            self.update_internal_pool_state(quotes);
            Ok(())
        }

        async fn get_opensea_order(
            &mut self,
            order_hash: H256,
        ) -> Option<Arc<FulfillListingResponse>> {
            if let Some(order) = self.order_cache.lock().get(&order_hash).cloned() {
                let counter = metrics::counter!("artemis.strategy.opensea_sudo.order_cache_hits");
                counter.increment(1);
                return Some(order);
            }

            let counter = metrics::counter!("artemis.strategy.opensea_sudo.order_cache_misses");
            counter.increment(1);
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
            let payment_value: U256 = order.fulfillment_data.transaction.value.into();
            let total_profit = sudo_bid.checked_sub(payment_value)?;

            let basic_order = fulfill_listing_response_to_basic_order_parameters(order);
            let mut call = self
                .arb_contract
                .execute_arb(basic_order, payment_value, sudo_pool);
            call.tx.set_value(payment_value);
            if call.tx.chain_id().is_none() {
                call.tx
                    .set_chain_id(order.fulfillment_data.transaction.chain);
            }

            Some(Action::SubmitTx(SubmitTxToMempool {
                tx: call.tx,
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

            let chunked: Vec<Vec<H160>> = pools
                .chunks(QUOTE_BATCH_SIZE)
                .map(|chunk| chunk.to_vec())
                .collect();

            let batches = chunked.into_iter().map(|chunk_vec| {
                let quoter = self.quoter.clone();
                async move {
                    let quotes = quoter.get_multiple_sell_quotes(chunk_vec.clone()).await?;
                    Ok::<_, anyhow::Error>(chunk_vec.into_iter().zip(quotes).collect::<Vec<_>>())
                }
            });

            let mut results = Vec::with_capacity(pools.len());
            let mut stream = stream::iter(batches).buffer_unordered(QUOTE_CONCURRENCY);
            while let Some(batch) = stream.next().await {
                match batch {
                    Ok(mut entries) => results.append(&mut entries),
                    Err(err) => return Err(err),
                }
            }
            Ok(results)
        }

        fn update_internal_pool_state(&mut self, pools_and_quotes: Vec<(H160, SellQuote)>) {
            for (pool_address, quote) in pools_and_quotes {
                if quote.quote_available {
                    self.pool_bids.insert(pool_address, quote.price);
                    self.sudo_pools
                        .entry(quote.nft_address)
                        .or_insert_with(BinaryHeap::new)
                        .push(PoolBidEntry {
                            pool: pool_address,
                            bid: quote.price,
                        });
                } else {
                    self.pool_bids.remove(&pool_address);
                }
            }
        }

        async fn get_touched_pools(&self, from_block: u64, to_block: u64) -> Result<Vec<H160>> {
            let address_list = self.pool_bids.keys().cloned().collect::<Vec<_>>();
            let filter = Filter::new()
                .from_block(from_block)
                .to_block(to_block)
                .address(address_list)
                .events(&*POOL_EVENT_SIGNATURES);

            let events = self.client.get_logs(&filter).await?;
            let touched_pools = events.iter().map(|event| event.address).collect::<Vec<_>>();
            Ok(touched_pools)
        }

        async fn get_new_pools(&self, from_block: u64, to_block: u64) -> Result<Vec<H160>> {
            if to_block < from_block {
                return Ok(vec![]);
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

            let lssvm = self.lssvm_pair_factory.clone();
            let total = ranges.len() as f64;

            let mut stream =
                stream::iter(ranges.into_iter().enumerate().map(|(idx, (start, end))| {
                    let factory = lssvm.clone();
                    async move {
                        let events = factory
                            .event::<NewPairFilter>()
                            .from_block(start)
                            .to_block(end)
                            .query()
                            .await?;
                        info!(
                            "found {} new pools in block range {}-{} (progress {:.0}%)",
                            events.len(),
                            start,
                            end,
                            ((idx as f64 + 1.0) / total) * 100.0
                        );
                        Ok::<_, anyhow::Error>(
                            events
                                .into_iter()
                                .map(|event| event.pool_address)
                                .collect::<Vec<_>>(),
                        )
                    }
                }))
                .buffer_unordered(QUOTE_CONCURRENCY);

            let mut pool_addresses = Vec::new();
            while let Some(batch) = stream.next().await {
                match batch {
                    Ok(mut pools) => pool_addresses.append(&mut pools),
                    Err(err) => return Err(err),
                }
            }
            Ok(pool_addresses)
        }
    }
}

#[cfg(feature = "sdk-alloy")]
mod alloy_impl {
    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashMap};
    use std::num::NonZeroUsize;
    use std::sync::Arc;
    use std::time::Instant;

    use alloy_primitives::{keccak256, Address as AlloyAddress};
    use alloy_provider::{Provider, WalletProvider};
    use alloy_rpc_types_eth::{
        state::{AccountOverride, StateOverride},
        Filter,
    };
    use anyhow::{anyhow, Context, Result};
    use artemis_core::collectors::block_collector::NewBlock;
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

    use super::super::constants::{LSSVM_PAIR_FACTORY_ADDRESS, POOL_EVENT_SIGNATURES};
    use super::super::types::{
        fulfill_listing_response_to_basic_order_parameters, hash_to_fulfill_listing_request,
        Action, Config, Event,
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
        P: Provider + WalletProvider + Clone + Send + Sync + 'static,
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

        async fn process_new_block_event(&mut self, event: NewBlock) -> Result<()> {
            let block_number = event.number.to::<u64>();
            info!("processing new block {}", block_number);

            let new_pools = self.get_new_pools(block_number, block_number).await?;
            let touched_pools = self.get_touched_pools(block_number, block_number).await?;
            let quotes = self
                .get_quotes_for_pools([new_pools, touched_pools].concat())
                .await?;
            self.update_internal_pool_state(quotes);

            Ok(())
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
            let mut tx = SudoOpenseaArb::new(self.arb_contract_address, self.provider.clone())
                .executeArb(basic_order, payment_value, sudo_pool)
                .into_transaction_request();

            tx.value = Some(payment_value);
            if tx.chain_id.is_none() {
                tx.chain_id = Some(order.fulfillment_data.transaction.chain);
            }
            if tx.from.is_none() {
                tx.from = Some(self.provider.default_signer_address());
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

            let override_address = Self::address_from_u64(0x5eed);
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

                    let quotes = SudoPairQuoter::new(override_address, provider.clone())
                        .getMultipleSellQuotes(chunk_vec.clone())
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
            for (pool_address, quote) in pools_and_quotes {
                if quote.quoteAvailable {
                    let price = quote.price;
                    self.pool_bids.insert(pool_address, price);
                    self.sudo_pools
                        .entry(quote.nftAddress)
                        .or_insert_with(BinaryHeap::new)
                        .push(PoolBidEntry {
                            pool: pool_address,
                            bid: price,
                        });
                } else {
                    self.pool_bids.remove(&pool_address);
                }
            }
        }

        async fn get_touched_pools(&self, from_block: u64, to_block: u64) -> Result<Vec<H160>> {
            if self.pool_bids.is_empty() {
                return Ok(Vec::new());
            }

            if to_block < from_block {
                return Ok(Vec::new());
            }

            let addresses: Vec<AlloyAddress> = self.pool_bids.keys().cloned().collect();
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
                .map(|log| log.address())
                .collect::<Vec<H160>>())
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
            let factory_address = *LSSVM_PAIR_FACTORY_ADDRESS;
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
                                .map(|decoded| decoded.inner.data.poolAddress)
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
        P: Provider + WalletProvider + Clone + Send + Sync + 'static,
    {
        async fn sync_state(&mut self) -> Result<()> {
            let start_block = super::super::constants::FACTORY_DEPLOYMENT_BLOCK;
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
                Event::NewBlock(block) => match self.process_new_block_event(block).await {
                    Ok(_) => vec![],
                    Err(err) => panic!("Strategy is out of sync {err}"),
                },
            }
        }
    }

    #[cfg(all(test, feature = "sdk-alloy", not(feature = "sdk-ethers")))]
    mod tests {
        use super::*;
        use crate::constants;
        use alloy_primitives::hex;
        use alloy_provider::ProviderBuilder;
        use alloy_rpc_types_eth::Log as RpcLog;
        use alloy_sol_types::SolValue;
        use alloy_transport::mock::Asserter;
        use artemis_core::collectors::block_collector::NewBlock;
        use artemis_core::eth::{Address as H160, Bytes, Hash as H256, LocalWallet, U256, U64};
        use futures::executor::block_on;
        use opensea_v2::client::{OpenSeaApiConfig, OpenSeaV2Client};
        use opensea_v2::types::{
            AdditionalRecipient as ApiAdditionalRecipient, FulfillListingResponse, FulfillmentData,
            InputData, Parameters, Transaction,
        };
        use serde_json::json;
        use std::sync::Arc;

        use bindings::SellQuote;

        fn h160_from_u64(value: u64) -> H160 {
            let mut bytes = [0u8; 20];
            bytes[12..].copy_from_slice(&value.to_be_bytes());
            H160::from_slice(&bytes)
        }

        fn sample_order(payment_value: u64, chain_id: u64) -> FulfillListingResponse {
            let consideration_amount = U256::from(1_000_u64);
            let offer_amount = U256::from(500_u64);
            let timestamp = U256::from(1_696_969_696_u64);
            let salt = U256::from(42_u64);
            let conduit_key = H256::from([7u8; 32]);

            let params = Parameters {
                consideration_token: h160_from_u64(0x1111),
                consideration_identifier: consideration_amount,
                consideration_amount,
                offerer: h160_from_u64(0x2222),
                zone: h160_from_u64(0x3333),
                offer_token: h160_from_u64(0x4444),
                offer_identifier: offer_amount,
                offer_amount,
                basic_order_type: 0,
                start_time: timestamp,
                end_time: timestamp,
                zone_hash: conduit_key,
                salt,
                offerer_conduit_key: conduit_key,
                fulfiller_conduit_key: conduit_key,
                total_original_additional_recipients: U256::ZERO,
                additional_recipients: vec![ApiAdditionalRecipient {
                    amount: U256::from(10u64),
                    recipient: h160_from_u64(0x5555),
                }],
                signature: Bytes::from(vec![0x11; 65]),
            };

            FulfillListingResponse {
                protocol: "seaport1.5".to_string(),
                fulfillment_data: FulfillmentData {
                    transaction: Transaction {
                        function: "executeArb".to_string(),
                        chain: chain_id,
                        to: "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
                        value: payment_value,
                        input_data: InputData { parameters: params },
                    },
                },
            }
        }

        #[test]
        fn build_arb_tx_sets_expected_fields() {
            let mock = Asserter::new();
            let provider = Arc::new(
                ProviderBuilder::new()
                    .wallet(LocalWallet::from_slice(&[1u8; 32]).expect("wallet"))
                    .connect_mocked_client(mock),
            );
            let default_signer = provider.default_signer_address();

            let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {
                api_key: "test-key".to_string(),
            });

            let config = Config {
                arb_contract_address: h160_from_u64(0xa11ce),
                bid_percentage: 50,
            };

            let strategy = OpenseaSudoArb::new(Arc::clone(&provider), opensea_client, config);

            let payment_value = 1_000_u64;
            let chain_id = 1u64;
            let order = sample_order(payment_value, chain_id);

            let sudo_pool = h160_from_u64(0xabcdef);
            let sudo_bid = U256::from(payment_value * 2);

            let action = strategy
                .build_arb_tx(&order, sudo_pool, sudo_bid)
                .expect("tx should be built");

            let Action::SubmitTx(SubmitTxToMempool { tx, gas_bid_info }) = action;

            let info = gas_bid_info.expect("gas bid info should be present");
            assert_eq!(info.total_profit, U256::from(payment_value));
            assert_eq!(info.bid_percentage, 50);

            let to = tx
                .to
                .as_ref()
                .and_then(|kind| kind.to())
                .expect("call target");
            let expected_to = h160_from_u64(0xa11ce);
            assert_eq!(to.clone(), expected_to);
            assert_eq!(tx.chain_id, Some(chain_id));
            assert_eq!(tx.value, Some(U256::from(payment_value)));
            assert_eq!(tx.from, Some(default_signer));

            let calldata = tx
                .input
                .clone()
                .into_input()
                .expect("encoded calldata should exist");
            assert!(
                calldata.len() > 4,
                "calldata should contain selector and args"
            );

            let selector = &calldata[..4];
            let expected_selector = &super::keccak256(b"executeArb((address,uint256,uint256,address,address,address,uint256,uint256,uint8,uint256,uint256,bytes32,uint256,bytes32,bytes32,uint256,(uint256,address)[],bytes),uint256,address)")[..4];
            assert_eq!(selector, expected_selector);
        }

        #[test]
        fn update_internal_pool_state_tracks_quotes() {
            let mock = Asserter::new();
            let provider = Arc::new(
                ProviderBuilder::new()
                    .wallet(LocalWallet::from_slice(&[2u8; 32]).expect("wallet"))
                    .connect_mocked_client(mock),
            );

            let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {
                api_key: "test-key".to_string(),
            });

            let config = Config {
                arb_contract_address: h160_from_u64(0xbeef),
                bid_percentage: 10,
            };

            let mut strategy = OpenseaSudoArb::new(provider, opensea_client, config);

            let nft = h160_from_u64(0xdead);
            let pool = h160_from_u64(0xaaaa);
            let price = U256::from(1_000_u64);
            let quote = SellQuote {
                quoteAvailable: true,
                nftAddress: nft,
                price,
            };

            strategy.update_internal_pool_state(vec![(pool, quote)]);

            let (bid_pool, bid_price) = strategy
                .best_bid_for(&nft)
                .expect("quote should populate state");
            assert_eq!(bid_pool, pool);
            assert_eq!(bid_price, price);

            // Feeding an unavailable quote should clear the map entry.
            let cleared_quote = SellQuote {
                quoteAvailable: false,
                nftAddress: nft,
                price,
            };
            strategy.update_internal_pool_state(vec![(pool, cleared_quote)]);
            assert!(strategy.best_bid_for(&nft).is_none());
        }

        #[test]
        fn get_new_pools_decodes_factory_logs() {
            let mock = Asserter::new();
            let provider = Arc::new(
                ProviderBuilder::new()
                    .wallet(LocalWallet::from_slice(&[3u8; 32]).expect("wallet"))
                    .connect_mocked_client(mock.clone()),
            );

            let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {
                api_key: "test-key".to_string(),
            });

            let config = Config {
                arb_contract_address: h160_from_u64(0xface),
                bid_percentage: 20,
            };

            let strategy = OpenseaSudoArb::new(Arc::clone(&provider), opensea_client, config);

            let pool_address = h160_from_u64(0x900d);
            let mut encoded = [0u8; 32];
            encoded[12..].copy_from_slice(pool_address.as_slice());
            let topic = super::keccak256(b"NewPair(address)");
            let inner = alloy_primitives::Log::new(
                *LSSVM_PAIR_FACTORY_ADDRESS,
                vec![topic],
                Bytes::from(encoded.to_vec()),
            )
            .expect("valid log");

            mock.push_success(&vec![RpcLog {
                inner,
                block_hash: None,
                block_number: Some(1),
                block_timestamp: None,
                transaction_hash: None,
                transaction_index: None,
                log_index: None,
                removed: false,
            }]);

            let pools = block_on(strategy.get_new_pools(1, 1)).expect("log decode succeeds");
            assert_eq!(pools, vec![pool_address]);
        }

        #[test]
        fn get_touched_pools_returns_active_pools() {
            let mock = Asserter::new();
            let provider = Arc::new(
                ProviderBuilder::new()
                    .wallet(LocalWallet::from_slice(&[4u8; 32]).expect("wallet"))
                    .connect_mocked_client(mock.clone()),
            );

            let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {
                api_key: "test-key".to_string(),
            });

            let config = Config {
                arb_contract_address: h160_from_u64(0xbead),
                bid_percentage: 15,
            };

            let mut strategy = OpenseaSudoArb::new(provider, opensea_client, config);

            let pool = h160_from_u64(0xabcd);
            strategy.pool_bids.insert(pool, U256::from(42u64));

            let topic = *POOL_EVENT_SIGNATURES.iter().next().expect("static topics");
            let inner =
                alloy_primitives::Log::new(pool, vec![topic], Bytes::from(Vec::<u8>::new()))
                    .expect("valid touch log");

            mock.push_success(&vec![RpcLog {
                inner,
                block_hash: None,
                block_number: Some(2),
                block_timestamp: None,
                transaction_hash: None,
                transaction_index: None,
                log_index: None,
                removed: false,
            }]);

            let touched = block_on(strategy.get_touched_pools(2, 2)).expect("fetch succeeds");
            assert_eq!(touched, vec![pool]);
        }

        #[test]
        fn sync_state_populates_pool_state_from_factory_logs() {
            let mock = Asserter::new();
            let provider = Arc::new(
                ProviderBuilder::new()
                    .wallet(LocalWallet::from_slice(&[5u8; 32]).expect("wallet"))
                    .connect_mocked_client(mock.clone()),
            );

            let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {
                api_key: "test-key".to_string(),
            });

            let config = Config {
                arb_contract_address: h160_from_u64(0xffee),
                bid_percentage: 25,
            };

            let mut strategy = OpenseaSudoArb::new(Arc::clone(&provider), opensea_client, config);

            let pool = h160_from_u64(0xcafe);
            let nft = h160_from_u64(0x1234);
            let price = U256::from(4_200_u64);

            let block_number_hex = format!("0x{:x}", constants::FACTORY_DEPLOYMENT_BLOCK);
            mock.push_success(&block_number_hex);

            let mut encoded_address = [0u8; 32];
            encoded_address[12..].copy_from_slice(pool.as_slice());
            let topic = super::keccak256(b"NewPair(address)");
            let inner = alloy_primitives::Log::new(
                *LSSVM_PAIR_FACTORY_ADDRESS,
                vec![topic],
                Bytes::from(encoded_address.to_vec()),
            )
            .expect("valid new pair log");
            let new_pool_logs = vec![RpcLog {
                inner,
                block_hash: None,
                block_number: Some(constants::FACTORY_DEPLOYMENT_BLOCK),
                block_timestamp: None,
                transaction_hash: None,
                transaction_index: None,
                log_index: None,
                removed: false,
            }];
            mock.push_success(&new_pool_logs);

            let sell_quotes = vec![SellQuote {
                quoteAvailable: true,
                nftAddress: nft,
                price,
            }];
            let encoded = sell_quotes.abi_encode();
            let encoded_hex = format!("0x{}", hex::encode(encoded));
            mock.push_success(&json!(encoded_hex));

            block_on(strategy.sync_state()).expect("sync state should succeed");

            let (best_pool, best_bid) = strategy.best_bid_for(&nft).expect("pool populated");
            assert_eq!(best_pool, pool);
            assert_eq!(best_bid, price);
        }

        #[test]
        fn process_new_block_event_refreshes_quotes() {
            let mock = Asserter::new();
            let provider = Arc::new(
                ProviderBuilder::new()
                    .wallet(LocalWallet::from_slice(&[6u8; 32]).expect("wallet"))
                    .connect_mocked_client(mock.clone()),
            );

            let opensea_client = OpenSeaV2Client::new(OpenSeaApiConfig {
                api_key: "test-key".to_string(),
            });

            let config = Config {
                arb_contract_address: h160_from_u64(0xfeed),
                bid_percentage: 30,
            };

            let mut strategy = OpenseaSudoArb::new(Arc::clone(&provider), opensea_client, config);

            let pool = h160_from_u64(0xaaaa);
            let nft = h160_from_u64(0xbbbb);
            let initial_price = U256::from(1_000_u64);

            let block_number_hex = format!("0x{:x}", constants::FACTORY_DEPLOYMENT_BLOCK);
            mock.push_success(&block_number_hex);

            let mut encoded_address = [0u8; 32];
            encoded_address[12..].copy_from_slice(pool.as_slice());
            let topic = super::keccak256(b"NewPair(address)");
            let inner = alloy_primitives::Log::new(
                *LSSVM_PAIR_FACTORY_ADDRESS,
                vec![topic],
                Bytes::from(encoded_address.to_vec()),
            )
            .expect("valid new pair log");
            let new_pool_logs = vec![RpcLog {
                inner,
                block_hash: None,
                block_number: Some(constants::FACTORY_DEPLOYMENT_BLOCK),
                block_timestamp: None,
                transaction_hash: None,
                transaction_index: None,
                log_index: None,
                removed: false,
            }];
            mock.push_success(&new_pool_logs);

            let initial_quotes = vec![SellQuote {
                quoteAvailable: true,
                nftAddress: nft,
                price: initial_price,
            }];
            let encoded_initial = initial_quotes.abi_encode();
            let encoded_initial_hex = format!("0x{}", hex::encode(encoded_initial));
            mock.push_success(&json!(encoded_initial_hex));

            block_on(strategy.sync_state()).expect("sync state should succeed");

            let (_, best_bid) = strategy.best_bid_for(&nft).expect("pool populated");
            assert_eq!(best_bid, initial_price);

            let empty_logs = json!([]);
            mock.push_success(&empty_logs);

            let touch_topic = *POOL_EVENT_SIGNATURES.iter().next().expect("static topics");
            let touch_log =
                alloy_primitives::Log::new(pool, vec![touch_topic], Bytes::from(Vec::<u8>::new()))
                    .expect("valid touch log");
            let touched_logs = vec![RpcLog {
                inner: touch_log,
                block_hash: None,
                block_number: Some(constants::FACTORY_DEPLOYMENT_BLOCK + 1),
                block_timestamp: None,
                transaction_hash: None,
                transaction_index: None,
                log_index: None,
                removed: false,
            }];
            mock.push_success(&touched_logs);

            let refreshed_price = U256::from(2_000_u64);
            let refreshed_quotes = vec![SellQuote {
                quoteAvailable: true,
                nftAddress: nft,
                price: refreshed_price,
            }];
            let encoded_refresh = refreshed_quotes.abi_encode();
            let encoded_refresh_hex = format!("0x{}", hex::encode(encoded_refresh));
            mock.push_success(&json!(encoded_refresh_hex));

            let block_number = constants::FACTORY_DEPLOYMENT_BLOCK + 1;
            let event = NewBlock {
                hash: H256::ZERO,
                number: U64::from(block_number),
            };

            block_on(strategy.process_new_block_event(event))
                .expect("processing new block should succeed");

            let (_, updated_bid) = strategy.best_bid_for(&nft).expect("pool tracked");
            assert_eq!(updated_bid, refreshed_price);
        }
    }
}
