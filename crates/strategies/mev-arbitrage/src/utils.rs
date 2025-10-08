//! Common Utility Functions
//!
//! Shared utilities to avoid code duplication across detectors and validators
//! Includes:
//! - TokenGraph: petgraph-based token relationship graph
//! - PoolManager: DashMap-based concurrent pool state management
//! - AMM math utilities
//! - Price and profit calculations

use alloy_primitives::{U256, Address, Bytes};
use alloy_sol_types::{sol, SolCall};
use alloy_provider::Provider;
use alloy_rpc_types_eth::TransactionRequest;
use std::sync::Arc;
use std::collections::HashMap;
use dashmap::DashMap;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Directed;
use anyhow::{Result, anyhow};
use tracing::{debug, info, warn};

/// AMM (Automated Market Maker) mathematical formulas
pub mod amm {
    use super::*;

    /// Calculate output amount for Uniswap V2 style constant product AMM
    ///
    /// Formula: amountOut = (amountIn * 997 * reserveOut) / (reserveIn * 1000 + amountIn * 997)
    /// Fee: 0.3% (997/1000)
    ///
    /// # Arguments
    /// * `amount_in` - Input token amount
    /// * `reserve_in` - Input token reserve in pool
    /// * `reserve_out` - Output token reserve in pool
    ///
    /// # Returns
    /// Output token amount after fee
    pub fn uniswap_v2_output(amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }

        let amount_in_with_fee = amount_in.saturating_mul(U256::from(997));
        let numerator = amount_in_with_fee.saturating_mul(reserve_out);
        let denominator = reserve_in
            .saturating_mul(U256::from(1000))
            .saturating_add(amount_in_with_fee);

        if denominator.is_zero() {
            return U256::ZERO;
        }

        numerator / denominator
    }

    /// Calculate input amount needed for desired output (Uniswap V2)
    ///
    /// Formula: amountIn = (reserveIn * amountOut * 1000) / ((reserveOut - amountOut) * 997) + 1
    pub fn uniswap_v2_input(amount_out: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_out.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return U256::ZERO;
        }

        if amount_out >= reserve_out {
            return U256::MAX; // Not enough liquidity
        }

        let numerator = reserve_in
            .saturating_mul(amount_out)
            .saturating_mul(U256::from(1000));
        let denominator = reserve_out
            .saturating_sub(amount_out)
            .saturating_mul(U256::from(997));

        if denominator.is_zero() {
            return U256::MAX;
        }

        numerator / denominator + U256::from(1)
    }

    /// Calculate output for Curve StableSwap (simplified)
    ///
    /// This is a simplified version. Full Curve math is more complex.
    pub fn curve_stable_swap_output(
        amount_in: U256,
        balance_in: U256,
        balance_out: U256,
        _amplification: u64,
    ) -> U256 {
        // Simplified implementation
        // Real Curve uses: y^2 + y * (sum' - (A*n^n - 1) * D / (A * n^n)) = D^(n+1) / (n^(2*n) * prod')

        // For now, use Uniswap-like formula with lower fee
        let fee = U256::from(4); // 0.04% fee for stablecoins
        let amount_in_with_fee = amount_in.saturating_mul(U256::from(10000).saturating_sub(fee)) / U256::from(10000);

        let numerator = amount_in_with_fee.saturating_mul(balance_out);
        let denominator = balance_in.saturating_add(amount_in_with_fee);

        if denominator.is_zero() {
            return U256::ZERO;
        }

        numerator / denominator
    }

    /// Calculate Uniswap V3 output for a single tick range
    ///
    /// V3 is more complex with concentrated liquidity, this is simplified
    pub fn uniswap_v3_output_single_tick(
        amount_in: U256,
        _sqrt_price_x96: U256,
        liquidity: u128,
        fee_tier: u32, // 500 = 0.05%, 3000 = 0.3%, 10000 = 1%
    ) -> U256 {
        // Simplified: Use similar calculation to V2 but with custom fee
        let fee_multiplier = U256::from(1_000_000 - fee_tier);
        let amount_in_with_fee = amount_in.saturating_mul(fee_multiplier) / U256::from(1_000_000);

        // Simplified price impact calculation
        // Real V3: calculate across multiple ticks
        let price_impact = amount_in_with_fee / U256::from(liquidity.max(1));
        amount_in_with_fee.saturating_sub(price_impact)
    }
}

/// Price calculation utilities
pub mod price {
    use super::*;

    /// Calculate effective price (amount_out / amount_in)
    pub fn effective_price(amount_in: U256, amount_out: U256) -> f64 {
        if amount_in.is_zero() {
            return 0.0;
        }

        let amount_in_f64 = amount_in.to::<u128>() as f64;
        let amount_out_f64 = amount_out.to::<u128>() as f64;

        amount_out_f64 / amount_in_f64
    }

    /// Calculate price impact percentage
    pub fn price_impact(amount_in: U256, reserve_in: U256) -> f64 {
        if reserve_in.is_zero() {
            return 100.0; // 100% impact if no liquidity
        }

        let amount_in_f64 = amount_in.to::<u128>() as f64;
        let reserve_in_f64 = reserve_in.to::<u128>() as f64;

        (amount_in_f64 / reserve_in_f64) * 100.0
    }

    /// Calculate slippage percentage
    pub fn slippage(expected_price: f64, actual_price: f64) -> f64 {
        if expected_price == 0.0 {
            return 100.0;
        }

        ((actual_price - expected_price) / expected_price).abs() * 100.0
    }
}

/// Profit calculation utilities
pub mod profit {
    use super::*;

    /// Calculate net profit after gas
    pub fn net_profit(gross_profit: U256, gas_used: u64, gas_price: U256) -> U256 {
        let gas_cost = U256::from(gas_used).saturating_mul(gas_price);
        gross_profit.saturating_sub(gas_cost)
    }

    /// Calculate profit percentage
    pub fn profit_percentage(initial: U256, final_amount: U256) -> f64 {
        if initial.is_zero() {
            return 0.0;
        }

        let initial_f64 = initial.to::<u128>() as f64;
        let final_f64 = final_amount.to::<u128>() as f64;

        ((final_f64 - initial_f64) / initial_f64) * 100.0
    }

    /// Check if profit meets minimum threshold
    pub fn meets_threshold(profit: U256, threshold: U256) -> bool {
        profit >= threshold
    }
}

/// Validation helper functions
pub mod validation {
    use super::*;

    /// Validate that reserves are sufficient for trade
    pub fn has_sufficient_liquidity(amount_in: U256, reserve: U256, max_impact: f64) -> bool {
        let impact = super::price::price_impact(amount_in, reserve);
        impact <= max_impact
    }

    /// Validate slippage is within tolerance
    pub fn slippage_acceptable(expected: U256, actual: U256, tolerance: f64) -> bool {
        let expected_price = super::price::effective_price(U256::from(1), expected);
        let actual_price = super::price::effective_price(U256::from(1), actual);
        let slippage = super::price::slippage(expected_price, actual_price);

        slippage <= tolerance
    }
}

/// Pool state information (supports both V2 and V3)
#[derive(Debug, Clone)]
pub struct PoolState {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub fee_bps: u64, // Fee in basis points (e.g., 30 for 0.3%)
    pub last_update_block: u64,

    // V3-specific fields (optional)
    pub pool_type: PoolType,
    pub sqrt_price_x96: Option<U256>,  // V3 price
    pub tick: Option<i32>,              // V3 current tick
    pub liquidity: Option<u128>,        // V3 liquidity
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolType {
    UniswapV2,
    UniswapV3,
    Sushiswap,
    Curve,
}

/// Edge in token graph representing a pool
#[derive(Debug, Clone)]
pub struct PoolEdge {
    pub pool_address: Address,
    pub target_token: Address,
    pub fee_bps: u64,
}

/// Token graph using petgraph for efficient path finding
pub struct TokenGraph {
    /// Directed graph: nodes are tokens, edges are pools
    graph: Graph<Address, PoolEdge, Directed>,
    /// Map from token address to node index
    token_to_node: HashMap<Address, NodeIndex>,
    /// Map from node index to token address
    node_to_token: HashMap<NodeIndex, Address>,
}

impl TokenGraph {
    /// Create an empty token graph
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            token_to_node: HashMap::new(),
            node_to_token: HashMap::new(),
        }
    }

    /// Build token graph from pool states
    pub fn from_pools(pools: &[PoolState]) -> Result<Self> {
        let mut graph = Graph::new();
        let mut token_to_node = HashMap::new();
        let mut node_to_token = HashMap::new();

        // First pass: create all nodes
        for pool in pools {
            // Get or create node for token0
            if !token_to_node.contains_key(&pool.token0) {
                let node = graph.add_node(pool.token0);
                token_to_node.insert(pool.token0, node);
                node_to_token.insert(node, pool.token0);
            }

            // Get or create node for token1
            if !token_to_node.contains_key(&pool.token1) {
                let node = graph.add_node(pool.token1);
                token_to_node.insert(pool.token1, node);
                node_to_token.insert(node, pool.token1);
            }
        }

        // Second pass: add edges
        for pool in pools {
            let node0 = *token_to_node.get(&pool.token0).unwrap();
            let node1 = *token_to_node.get(&pool.token1).unwrap();

            // Edge from token0 to token1
            graph.add_edge(
                node0,
                node1,
                PoolEdge {
                    pool_address: pool.address,
                    target_token: pool.token1,
                    fee_bps: pool.fee_bps,
                },
            );

            // Edge from token1 to token0
            graph.add_edge(
                node1,
                node0,
                PoolEdge {
                    pool_address: pool.address,
                    target_token: pool.token0,
                    fee_bps: pool.fee_bps,
                },
            );
        }

        Ok(Self {
            graph,
            token_to_node,
            node_to_token,
        })
    }

    /// Get node index for a token
    pub fn get_node(&self, token: Address) -> Option<NodeIndex> {
        self.token_to_node.get(&token).copied()
    }

    /// Get token address for a node
    pub fn get_token(&self, node: NodeIndex) -> Option<Address> {
        self.node_to_token.get(&node).copied()
    }

    /// Find all pools connecting two tokens
    pub fn find_pools(&self, token_a: Address, token_b: Address) -> Vec<&PoolEdge> {
        use petgraph::Direction;

        let Some(node_a) = self.get_node(token_a) else {
            return vec![];
        };
        let Some(node_b) = self.get_node(token_b) else {
            return vec![];
        };

        self.graph
            .edges_directed(node_a, Direction::Outgoing)
            .filter(|edge_ref| edge_ref.target() == node_b)
            .map(|edge_ref| edge_ref.weight())
            .collect()
    }

    /// Find all triangular arbitrage paths starting from a token
    pub fn find_triangular_paths(&self, start_token: Address) -> Vec<Vec<Address>> {
        use petgraph::Direction;

        let Some(start_node) = self.get_node(start_token) else {
            return vec![];
        };

        let mut paths = Vec::new();

        // Use Direction::Outgoing for edges
        for edge_ab in self.graph.edges_directed(start_node, Direction::Outgoing) {
            let node_b = edge_ab.target();
            let Some(token_b) = self.get_token(node_b) else {
                continue;
            };

            for edge_bc in self.graph.edges_directed(node_b, Direction::Outgoing) {
                let node_c = edge_bc.target();
                if node_c == start_node || node_c == node_b {
                    continue;
                }

                let Some(token_c) = self.get_token(node_c) else {
                    continue;
                };

                // Check if there's an edge from C back to A
                let has_return_edge = self.graph
                    .edges_directed(node_c, Direction::Outgoing)
                    .any(|e| e.target() == start_node);

                if has_return_edge {
                    paths.push(vec![start_token, token_b, token_c]);
                }
            }
        }

        paths
    }

    /// Get number of tokens in graph
    pub fn token_count(&self) -> usize {
        self.token_to_node.len()
    }

    /// Get number of edges (pools)
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }
}

// ============================================================================
// Smart Contract ABIs
// ============================================================================

sol! {
    /// Uniswap V2 Factory interface
    interface IUniswapV2Factory {
        function allPairsLength() external view returns (uint256);
        function allPairs(uint256) external view returns (address pair);
    }

    /// Uniswap V2 Pair interface
    interface IUniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        function token0() external view returns (address);
        function token1() external view returns (address);
    }

    /// Uniswap V3 Factory interface
    interface IUniswapV3Factory {
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
    }

    /// Uniswap V3 Pool interface
    interface IUniswapV3Pool {
        struct Slot0 {
            uint160 sqrtPriceX96;
            int24 tick;
            uint16 observationIndex;
            uint16 observationCardinality;
            uint16 observationCardinalityNext;
            uint8 feeProtocol;
            bool unlocked;
        }

        function slot0() external view returns (
            uint160 sqrtPriceX96,
            int24 tick,
            uint16 observationIndex,
            uint16 observationCardinality,
            uint16 observationCardinalityNext,
            uint8 feeProtocol,
            bool unlocked
        );

        function liquidity() external view returns (uint128);
        function token0() external view returns (address);
        function token1() external view returns (address);
        function fee() external view returns (uint24);
    }

    /// Multicall3 interface
    interface IMulticall3 {
        struct Call3 {
            address target;
            bool allowFailure;
            bytes callData;
        }

        struct Result {
            bool success;
            bytes returnData;
        }

        function aggregate3(Call3[] calldata calls) external payable returns (Result[] memory returnData);
    }
}

/// Known DEX factory addresses
pub mod dex_factories {
    use super::*;
    use std::str::FromStr;

    /// Get Uniswap V2 factory address
    pub fn uniswap_v2() -> Address {
        Address::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap()
    }

    /// Get Sushiswap factory address
    pub fn sushiswap() -> Address {
        Address::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac").unwrap()
    }

    /// Get Uniswap V3 factory address
    pub fn uniswap_v3() -> Address {
        Address::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984").unwrap()
    }

    /// Get Multicall3 address
    pub fn multicall3() -> Address {
        Address::from_str("0xcA11bde05977b3631167028862bE2a173976CA11").unwrap()
    }

    /// V3 fee tiers
    pub const V3_FEE_LOW: u32 = 500;      // 0.05%
    pub const V3_FEE_MEDIUM: u32 = 3000;  // 0.3%
    pub const V3_FEE_HIGH: u32 = 10000;   // 1%
}

/// Pool state manager using DashMap for concurrent access
pub struct PoolManager<P> {
    provider: Arc<P>,
    pools: Arc<DashMap<Address, PoolState>>,
    known_pools: Arc<DashMap<Address, bool>>, // Changed to DashMap for thread-safety
    multicall3: Address,
}

impl<P> PoolManager<P>
where
    P: Provider + Clone + 'static,
{
    pub fn new(provider: Arc<P>) -> Self {
        Self {
            provider,
            pools: Arc::new(DashMap::new()),
            known_pools: Arc::new(DashMap::new()),
            multicall3: dex_factories::multicall3(),
        }
    }

    /// Discover all pools from DEX factories
    pub async fn discover_pools(&self) -> Result<Vec<PoolState>> {
        info!("🔍 Starting pool discovery...");
        let mut all_pools = Vec::new();

        // Define factories to query
        let factories = vec![
            ("Uniswap V2", dex_factories::uniswap_v2()),
            ("Sushiswap", dex_factories::sushiswap()),
        ];

        for (name, factory_addr) in factories {
            match self.discover_pools_from_factory(name, factory_addr).await {
                Ok(pools) => {
                    info!("✅ Discovered {} pools from {}", pools.len(), name);
                    all_pools.extend(pools);
                }
                Err(e) => {
                    warn!("⚠️  Failed to discover pools from {}: {}", name, e);
                }
            }
        }

        info!("✅ Total pools discovered: {}", all_pools.len());
        Ok(all_pools)
    }

    /// Discover pools from a specific factory
    async fn discover_pools_from_factory(
        &self,
        factory_name: &str,
        factory_addr: Address,
    ) -> Result<Vec<PoolState>> {
        // 1. Get total pairs count
        let pair_count_call = IUniswapV2Factory::allPairsLengthCall {};
        let calldata = pair_count_call.abi_encode();

        let tx = TransactionRequest::default()
            .to(factory_addr)
            .input(calldata.into());

        let result = self.provider.call(tx).await?;
        let pair_count = U256::from_be_slice(&result);

        info!("📊 {} has {} pairs", factory_name, pair_count);

        // 2. Batch fetch pair addresses using Multicall3
        let batch_size = 100;
        let mut all_pool_addresses = Vec::new();

        for batch_start in (0..pair_count.to::<usize>()).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(pair_count.to::<usize>());

            let mut calls = Vec::new();
            for i in batch_start..batch_end {
                let call = IUniswapV2Factory::allPairsCall(U256::from(i));
                calls.push(IMulticall3::Call3 {
                    target: factory_addr,
                    allowFailure: true,
                    callData: call.abi_encode().into(),
                });
            }

            // Execute multicall
            let multicall_call = IMulticall3::aggregate3Call { calls };
            let tx = TransactionRequest::default()
                .to(self.multicall3)
                .input(multicall_call.abi_encode().into());

            let result = self.provider.call(tx).await?;
            let decoded = IMulticall3::aggregate3Call::abi_decode_returns(&result)?;

            for res in decoded {
                if res.success && res.returnData.len() >= 32 {
                    // Decode address from returnData
                    let pair_addr = Address::from_slice(&res.returnData[12..32]);
                    all_pool_addresses.push(pair_addr);
                }
            }

            debug!("📦 Batch {}-{}: {} addresses fetched",
                   batch_start, batch_end, all_pool_addresses.len());
        }

        // 3. Batch fetch pool details (reserves and tokens)
        let pool_states = self.fetch_pool_details(&all_pool_addresses).await?;

        // 4. Store in DashMap
        for pool in &pool_states {
            self.pools.insert(pool.address, pool.clone());
            self.known_pools.insert(pool.address, true);
        }

        Ok(pool_states)
    }

    /// Fetch pool details (reserves, tokens) in batch
    async fn fetch_pool_details(&self, addresses: &[Address]) -> Result<Vec<PoolState>> {
        let batch_size = 50;
        let mut all_pools = Vec::new();

        for chunk in addresses.chunks(batch_size) {
            let mut calls = Vec::new();

            for &addr in chunk {
                // Call getReserves()
                let reserves_call = IUniswapV2Pair::getReservesCall {};
                calls.push(IMulticall3::Call3 {
                    target: addr,
                    allowFailure: true,
                    callData: reserves_call.abi_encode().into(),
                });

                // Call token0()
                let token0_call = IUniswapV2Pair::token0Call {};
                calls.push(IMulticall3::Call3 {
                    target: addr,
                    allowFailure: true,
                    callData: token0_call.abi_encode().into(),
                });

                // Call token1()
                let token1_call = IUniswapV2Pair::token1Call {};
                calls.push(IMulticall3::Call3 {
                    target: addr,
                    allowFailure: true,
                    callData: token1_call.abi_encode().into(),
                });
            }

            // Execute multicall
            let multicall_call = IMulticall3::aggregate3Call { calls };
            let tx = TransactionRequest::default()
                .to(self.multicall3)
                .input(multicall_call.abi_encode().into());

            let result = self.provider.call(tx).await?;
            let decoded = IMulticall3::aggregate3Call::abi_decode_returns(&result)?;

            // Parse results (每个pool有3个调用: reserves, token0, token1)
            for (i, addr) in chunk.iter().enumerate() {
                let reserves_idx = i * 3;
                let token0_idx = i * 3 + 1;
                let token1_idx = i * 3 + 2;

                if reserves_idx + 2 < decoded.len() {
                    let reserves_result = &decoded[reserves_idx];
                    let token0_result = &decoded[token0_idx];
                    let token1_result = &decoded[token1_idx];

                    if reserves_result.success && token0_result.success && token1_result.success {
                        // Decode reserves
                        if let Ok(reserves_decoded) = IUniswapV2Pair::getReservesCall::abi_decode_returns(
                            &reserves_result.returnData
                        ) {
                            let reserve0 = U256::from(reserves_decoded.reserve0);
                            let reserve1 = U256::from(reserves_decoded.reserve1);

                            // Decode tokens
                            let token0 = Address::from_slice(&token0_result.returnData[12..32]);
                            let token1 = Address::from_slice(&token1_result.returnData[12..32]);

                            // Filter out low liquidity pools (< 0.01 ETH equivalent)
                            let min_liquidity = U256::from(10_000_000_000_000_000u64); // 0.01 ETH
                            if reserve0 > min_liquidity || reserve1 > min_liquidity {
                                all_pools.push(PoolState {
                                    address: *addr,
                                    token0,
                                    token1,
                                    reserve0,
                                    reserve1,
                                    fee_bps: 30, // 0.3% for Uniswap V2/Sushiswap
                                    last_update_block: 0, // TODO: get current block
                                    pool_type: PoolType::UniswapV2,
                                    sqrt_price_x96: None,
                                    tick: None,
                                    liquidity: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(all_pools)
    }

    /// Update all known pools using Multicall3
    pub async fn update_all_pools(&self) -> Result<()> {
        let pool_addresses: Vec<Address> = self.known_pools
            .iter()
            .map(|entry| *entry.key())
            .collect();

        if pool_addresses.is_empty() {
            return Ok(());
        }

        debug!("🔄 Updating {} pools...", pool_addresses.len());

        let batch_size = 100;
        for chunk in pool_addresses.chunks(batch_size) {
            self.update_pool_batch(chunk).await?;
        }

        Ok(())
    }

    /// Update a batch of pools
    async fn update_pool_batch(&self, addresses: &[Address]) -> Result<()> {
        let mut calls = Vec::new();

        for &addr in addresses {
            let reserves_call = IUniswapV2Pair::getReservesCall {};
            calls.push(IMulticall3::Call3 {
                target: addr,
                allowFailure: true,
                callData: reserves_call.abi_encode().into(),
            });
        }

        // Execute multicall
        let multicall_call = IMulticall3::aggregate3Call { calls };
        let tx = TransactionRequest::default()
            .to(self.multicall3)
            .input(multicall_call.abi_encode().into());

        let result = self.provider.call(tx).await?;
        let decoded = IMulticall3::aggregate3Call::abi_decode_returns(&result)?;

        // Update pools in DashMap
        for (i, &addr) in addresses.iter().enumerate() {
            if i < decoded.len() {
                let res = &decoded[i];
                if res.success {
                    if let Ok(reserves) = IUniswapV2Pair::getReservesCall::abi_decode_returns(
                        &res.returnData
                    ) {
                        // Update only reserves, keep other fields
                        if let Some(mut pool) = self.pools.get_mut(&addr) {
                            pool.reserve0 = U256::from(reserves.reserve0);
                            pool.reserve1 = U256::from(reserves.reserve1);
                            // pool.last_update_block = current_block; // TODO
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_pool_state(&self, address: Address) -> Option<PoolState> {
        self.pools.get(&address).map(|entry| entry.clone())
    }

    pub fn get_all_states(&self) -> Arc<DashMap<Address, PoolState>> {
        self.pools.clone()
    }

    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    /// Get all known pool addresses
    pub fn get_known_pools(&self) -> Vec<Address> {
        self.known_pools.iter().map(|entry| *entry.key()).collect()
    }

    /// Get snapshot of all pool states for checkpointing
    pub fn get_all_states_snapshot(&self) -> HashMap<Address, PoolState> {
        self.pools.iter().map(|entry| (*entry.key(), entry.value().clone())).collect()
    }

    /// Restore pool states from checkpoint
    pub fn restore_states(&self, states: HashMap<Address, PoolState>) -> Result<()> {
        for (addr, state) in states {
            self.pools.insert(addr, state);
            self.known_pools.insert(addr, true);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniswap_v2_output() {
        let amount_in = U256::from(1_000_000_000_000_000_000u64); // 1 ETH
        let reserve_in = U256::from(100_000_000_000_000_000_000u128); // 100 ETH
        let reserve_out = U256::from(200_000_000_000u64); // 200k USDC (6 decimals)

        let output = amm::uniswap_v2_output(amount_in, reserve_in, reserve_out);
        assert!(output > U256::from(1_970_000_000u64)); // > 1970 USDC
        assert!(output < U256::from(1_980_000_000u64)); // < 1980 USDC
    }

    #[test]
    fn test_token_graph_from_pools() {
        let weth = Address::repeat_byte(0x01);
        let usdc = Address::repeat_byte(0x02);
        let dai = Address::repeat_byte(0x03);

        let pools = vec![
            PoolState {
                address: Address::repeat_byte(0x10),
                token0: weth,
                token1: usdc,
                reserve0: U256::from(100_000),
                reserve1: U256::from(200_000),
                fee_bps: 30,
                last_update_block: 0,
                pool_type: PoolType::UniswapV2,
                sqrt_price_x96: None,
                tick: None,
                liquidity: None,
            },
            PoolState {
                address: Address::repeat_byte(0x11),
                token0: usdc,
                token1: dai,
                reserve0: U256::from(300_000),
                reserve1: U256::from(300_000),
                fee_bps: 30,
                last_update_block: 0,
                pool_type: PoolType::UniswapV2,
                sqrt_price_x96: None,
                tick: None,
                liquidity: None,
            },
        ];

        let graph = TokenGraph::from_pools(&pools).unwrap();
        assert_eq!(graph.token_count(), 3);
        assert_eq!(graph.edge_count(), 4);
    }
}