//! Negative Cycle Arbitrage Algorithm Implementation
//! 
//! This module implements the ARB_NegativeCycle_Trading algorithm that uses
//! graph theory to detect arbitrage opportunities through negative cycles in
//! trading graphs.

use std::collections::{HashMap, HashSet, VecDeque};
use alloy_primitives::{Address, U256};
use tracing::{info, debug, warn, error};
use anyhow::Result;

use crate::{
    types::{AnalysisEvent, AnalysisAction, ArbitrageOpportunity, RiskLevel},
    error::{DeFiResult, DeFiAnalyzerError},
    evm_interpreter::{ExecutionPath, EVMExecutionState},
    abi_parser::ABIParser,
};

/// Negative Cycle Arbitrage Engine
pub struct NegativeCycleArbitrageEngine {
    /// Current on-chain state snapshot
    state_snapshot: StateSnapshot,
    /// Trading graph
    trading_graph: TradingGraph,
    /// Configuration
    config: NegativeCycleConfig,
    /// ABI parser for contract interactions
    abi_parser: ABIParser,
}

/// Configuration for negative cycle arbitrage
#[derive(Debug, Clone)]
pub struct NegativeCycleConfig {
    /// Minimum revenue threshold (including gas)
    pub target_revenue: U256,
    /// Maximum cycles to process per iteration
    pub max_cycles_per_iteration: usize,
    /// Maximum path length for arbitrage
    pub max_path_length: usize,
    /// Gas price for cost calculations
    pub gas_price: U256,
    /// Base gas cost per transaction
    pub base_gas_cost: u64,
    /// Supported DEX protocols
    pub supported_protocols: Vec<String>,
}

/// On-chain state snapshot
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    /// Block number
    pub block_number: u64,
    /// Token reserves in liquidity pools
    pub token_reserves: HashMap<PoolId, (U256, U256)>,
    /// Spot prices between token pairs
    pub spot_prices: HashMap<(TokenId, TokenId), f64>,
    /// Pool information
    pub pools: HashMap<PoolId, PoolInfo>,
    /// Token information
    pub tokens: HashMap<TokenId, TokenInfo>,
}

/// Trading graph representation
#[derive(Debug, Clone)]
pub struct TradingGraph {
    /// Nodes (tokens)
    pub nodes: HashSet<TokenId>,
    /// Edges (trading pairs)
    pub edges: HashMap<(TokenId, TokenId), EdgeWeight>,
    /// Adjacency list for efficient traversal
    pub adjacency_list: HashMap<TokenId, Vec<TokenId>>,
}

/// Pool identifier
pub type PoolId = String;

/// Token identifier
pub type TokenId = String;

/// Edge weight (negative log of spot price)
pub type EdgeWeight = f64;

/// Pool information
#[derive(Debug, Clone)]
pub struct PoolInfo {
    /// Pool address
    pub address: Address,
    /// Pool protocol (Uniswap V2, V3, SushiSwap, etc.)
    pub protocol: String,
    /// Token A
    pub token_a: TokenId,
    /// Token B
    pub token_b: TokenId,
    /// Fee tier
    pub fee: u32,
    /// Pool type specific data
    pub pool_data: PoolData,
}

/// Pool specific data
#[derive(Debug, Clone)]
pub enum PoolData {
    UniswapV2 {
        reserve_a: U256,
        reserve_b: U256,
    },
    UniswapV3 {
        liquidity: U256,
        sqrt_price: U256,
        tick: i32,
    },
    Curve {
        balances: Vec<U256>,
        amplification: U256,
    },
}

/// Token information
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// Token address
    pub address: Address,
    /// Token symbol
    pub symbol: String,
    /// Token decimals
    pub decimals: u8,
}

/// Arbitrage cycle
#[derive(Debug, Clone)]
pub struct ArbitrageCycle {
    /// Token path in the cycle
    pub path: Vec<TokenId>,
    /// Pool path for execution
    pub pools: Vec<PoolId>,
    /// Expected profit
    pub expected_profit: U256,
    /// Total weight (negative indicates arbitrage opportunity)
    pub total_weight: f64,
}

/// Arbitrage path with base asset bridge
#[derive(Debug, Clone)]
pub struct ArbitragePath {
    /// Complete path including base asset bridge
    pub full_path: Vec<TokenId>,
    /// Pool sequence for execution
    pub pool_sequence: Vec<PoolId>,
    /// Expected revenue
    pub expected_revenue: U256,
    /// Gas cost estimate
    pub gas_cost: U256,
    /// Net profit (revenue - gas)
    pub net_profit: U256,
}

impl Default for NegativeCycleConfig {
    fn default() -> Self {
        Self {
            target_revenue: U256::from(1_000_000_000_000_000u64), // 0.001 ETH
            max_cycles_per_iteration: 10,
            max_path_length: 5,
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            base_gas_cost: 150_000, // Base gas per swap
            supported_protocols: vec![
                "uniswap_v2".to_string(),
                "uniswap_v3".to_string(),
                "sushiswap".to_string(),
                "curve".to_string(),
            ],
        }
    }
}

impl NegativeCycleArbitrageEngine {
    /// Create a new negative cycle arbitrage engine
    pub fn new(config: NegativeCycleConfig) -> Self {
        Self {
            state_snapshot: StateSnapshot::new(),
            trading_graph: TradingGraph::new(),
            config,
            abi_parser: ABIParser::new(),
        }
    }

    /// Execute the main ARB_NegativeCycle_Trading algorithm
    pub async fn execute_arbitrage_algorithm(&mut self, initial_state: &StateSnapshot) -> DeFiResult<U256> {
        info!("Starting negative cycle arbitrage algorithm");
        
        let mut state = initial_state.clone();
        let mut graph = self.build_graph(&state)?;
        let mut revenue_total = U256::ZERO;
        let mut iteration = 0;
        
        while self.has_negative_cycle(&graph)? {
            iteration += 1;
            if iteration > 100 { // Prevent infinite loops
                warn!("Maximum iterations reached, stopping algorithm");
                break;
            }
            
            debug!("Iteration {} - searching for negative cycles", iteration);
            
            // Get negative cycle
            let cycle = self.get_negative_cycle(&graph)?;
            if cycle.is_none() {
                break;
            }
            
            let cycle = cycle.unwrap();
            debug!("Found negative cycle: {:?}", cycle.path);
            
            // Bridge base asset to cycle
            let arbitrage_path = self.bridge_base_asset_to_cycle(&cycle, &state)?;
            
            // Local parameter search to optimize investment
            let (revenue, new_state) = self.local_param_search(&arbitrage_path, &state).await?;
            
            if revenue > self.config.target_revenue {
                info!("Profitable arbitrage found: {} ETH", revenue);
                revenue_total = revenue_total.saturating_add(revenue);
                state = new_state;
            } else {
                debug!("Arbitrage below threshold: {} < {}", revenue, self.config.target_revenue);
            }
            
            // Rebuild graph with new state
            graph = self.build_graph(&state)?;
        }
        
        info!("Arbitrage algorithm completed. Total revenue: {} ETH", revenue_total);
        Ok(revenue_total)
    }

    /// Build trading graph from state snapshot
    /// Function BuildGraph(N, E, s)
    pub fn build_graph(&self, state: &StateSnapshot) -> DeFiResult<TradingGraph> {
        debug!("Building trading graph from state snapshot");
        
        let mut graph = TradingGraph::new();
        
        // Add all tokens as nodes
        for token_id in state.tokens.keys() {
            graph.nodes.insert(token_id.clone());
        }
        
        // Add edges with weights = -log(spot_price)
        for pool_info in state.pools.values() {
            let token_a = &pool_info.token_a;
            let token_b = &pool_info.token_b;
            
            // Calculate spot price for both directions
            let spot_price_a_to_b = self.query_spot_price((token_a.clone(), token_b.clone()), state)?;
            let spot_price_b_to_a = self.query_spot_price((token_b.clone(), token_a.clone()), state)?;
            
            if spot_price_a_to_b > 0.0 && spot_price_b_to_a > 0.0 {
                // Edge A -> B
                let weight_a_to_b = -spot_price_a_to_b.ln();
                graph.edges.insert((token_a.clone(), token_b.clone()), weight_a_to_b);
                graph.adjacency_list.entry(token_a.clone()).or_insert_with(Vec::new).push(token_b.clone());
                
                // Edge B -> A
                let weight_b_to_a = -spot_price_b_to_a.ln();
                graph.edges.insert((token_b.clone(), token_a.clone()), weight_b_to_a);
                graph.adjacency_list.entry(token_b.clone()).or_insert_with(Vec::new).push(token_a.clone());
            }
        }
        
        debug!("Built graph with {} nodes and {} edges", graph.nodes.len(), graph.edges.len());
        Ok(graph)
    }

    /// Check if graph has negative cycles using Bellman-Ford algorithm
    pub fn has_negative_cycle(&self, graph: &TradingGraph) -> DeFiResult<bool> {
        if graph.nodes.is_empty() {
            return Ok(false);
        }
        
        // Pick arbitrary source node
        let source = graph.nodes.iter().next().unwrap().clone();
        
        // Initialize distances
        let mut distances: HashMap<TokenId, f64> = HashMap::new();
        for node in &graph.nodes {
            distances.insert(node.clone(), f64::INFINITY);
        }
        distances.insert(source, 0.0);
        
        // Relax edges |V| - 1 times
        for _ in 0..(graph.nodes.len() - 1) {
            for ((u, v), weight) in &graph.edges {
                if distances[u] != f64::INFINITY {
                    let new_dist = distances[u] + weight;
                    if new_dist < distances[v] {
                        distances.insert(v.clone(), new_dist);
                    }
                }
            }
        }
        
        // Check for negative cycles
        for ((u, v), weight) in &graph.edges {
            if distances[u] != f64::INFINITY {
                let new_dist = distances[u] + weight;
                if new_dist < distances[v] {
                    return Ok(true); // Negative cycle detected
                }
            }
        }
        
        Ok(false)
    }

    /// Get a negative cycle using modified Bellman-Ford with cycle recovery
    pub fn get_negative_cycle(&self, graph: &TradingGraph) -> DeFiResult<Option<ArbitrageCycle>> {
        if graph.nodes.is_empty() {
            return Ok(None);
        }
        
        let source = graph.nodes.iter().next().unwrap().clone();
        
        // Track distances and predecessors
        let mut distances: HashMap<TokenId, f64> = HashMap::new();
        let mut predecessors: HashMap<TokenId, Option<TokenId>> = HashMap::new();
        
        // Initialize
        for node in &graph.nodes {
            distances.insert(node.clone(), f64::INFINITY);
            predecessors.insert(node.clone(), None);
        }
        distances.insert(source, 0.0);
        
        // Bellman-Ford relaxation
        let mut last_updated = None;
        for _ in 0..(graph.nodes.len() - 1) {
            for ((u, v), weight) in &graph.edges {
                if distances[u] != f64::INFINITY {
                    let new_dist = distances[u] + weight;
                    if new_dist < distances[v] {
                        distances.insert(v.clone(), new_dist);
                        predecessors.insert(v.clone(), Some(u.clone()));
                        last_updated = Some(v.clone());
                    }
                }
            }
        }
        
        // Check for negative cycle and extract it
        for ((u, v), weight) in &graph.edges {
            if distances[u] != f64::INFINITY {
                let new_dist = distances[u] + weight;
                if new_dist < distances[v] {
                    // Found negative cycle, extract it
                    return self.extract_cycle(v, &predecessors, graph);
                }
            }
        }
        
        Ok(None)
    }

    /// Extract cycle from predecessors
    fn extract_cycle(&self, start: &TokenId, predecessors: &HashMap<TokenId, Option<TokenId>>, graph: &TradingGraph) -> DeFiResult<Option<ArbitrageCycle>> {
        let mut visited = HashSet::new();
        let mut current = start.clone();
        let mut path = Vec::new();
        
        // Find a node in the cycle
        while !visited.contains(&current) {
            visited.insert(current.clone());
            if let Some(Some(pred)) = predecessors.get(&current) {
                current = pred.clone();
            } else {
                return Ok(None);
            }
        }
        
        // Extract the cycle
        let cycle_start = current.clone();
        path.push(current.clone());
        
        loop {
            if let Some(Some(pred)) = predecessors.get(&current) {
                current = pred.clone();
                if current == cycle_start {
                    break;
                }
                path.push(current.clone());
            } else {
                return Ok(None);
            }
        }
        
        path.reverse();
        
        // Calculate total weight
        let mut total_weight = 0.0;
        let mut pools = Vec::new();
        
        for i in 0..path.len() {
            let from = &path[i];
            let to = &path[(i + 1) % path.len()];
            
            if let Some(weight) = graph.edges.get(&(from.clone(), to.clone())) {
                total_weight += weight;
                
                // Find corresponding pool
                if let Some(pool) = self.find_pool_for_pair(from, to) {
                    pools.push(pool);
                }
            }
        }
        
        let expected_profit = if total_weight < 0.0 {
            // Estimate profit from negative weight
            U256::from(((-total_weight).exp() * 1000.0) as u64)
        } else {
            U256::ZERO
        };
        
        Ok(Some(ArbitrageCycle {
            path,
            pools,
            expected_profit,
            total_weight,
        }))
    }

    /// Bridge base asset to cycle to create executable arbitrage path
    pub fn bridge_base_asset_to_cycle(&self, cycle: &ArbitrageCycle, state: &StateSnapshot) -> DeFiResult<ArbitragePath> {
        // For simplicity, assume ETH/WETH as base asset
        let base_asset = "WETH".to_string();
        
        // If cycle already contains base asset, use it directly
        if cycle.path.contains(&base_asset) {
            let gas_cost = self.estimate_gas_cost(&cycle.pools);
            let net_profit = cycle.expected_profit.saturating_sub(gas_cost);
            
            return Ok(ArbitragePath {
                full_path: cycle.path.clone(),
                pool_sequence: cycle.pools.clone(),
                expected_revenue: cycle.expected_profit,
                gas_cost,
                net_profit,
            });
        }
        
        // Find shortest path from base asset to cycle
        let entry_token = &cycle.path[0];
        let bridge_path = self.find_shortest_path(&base_asset, entry_token, state)?;
        
        // Combine bridge path with cycle
        let mut full_path = bridge_path.clone();
        full_path.extend_from_slice(&cycle.path[1..]);
        full_path.push(entry_token.clone()); // Complete the cycle
        
        // Add bridge back to base asset
        let exit_token = cycle.path.last().unwrap();
        let return_path = self.find_shortest_path(exit_token, &base_asset, state)?;
        full_path.extend_from_slice(&return_path[1..]);
        
        // Find pool sequence
        let pool_sequence = self.build_pool_sequence(&full_path)?;
        
        let gas_cost = self.estimate_gas_cost(&pool_sequence);
        let net_profit = cycle.expected_profit.saturating_sub(gas_cost);
        
        Ok(ArbitragePath {
            full_path,
            pool_sequence,
            expected_revenue: cycle.expected_profit,
            gas_cost,
            net_profit,
        })
    }

    /// Local parameter search to optimize investment amount
    pub async fn local_param_search(&self, path: &ArbitragePath, state: &StateSnapshot) -> DeFiResult<(U256, StateSnapshot)> {
        let mut best_revenue = U256::ZERO;
        let mut best_state = state.clone();
        let mut investment = U256::from(1000000000000000u64); // Start with 0.001 ETH
        let max_investment = U256::from(10000000000000000000u64); // Max 10 ETH
        
        debug!("Starting parameter search for path: {:?}", path.full_path);
        
        while investment <= max_investment {
            // Simulate execution with current investment
            match self.simulate_arbitrage_execution(path, investment, state).await {
                Ok((revenue, new_state)) => {
                    if revenue > best_revenue {
                        best_revenue = revenue;
                        best_state = new_state;
                        // Increase investment for next iteration
                        investment = investment.saturating_mul(U256::from(2));
                    } else {
                        // Revenue is decreasing, stop optimization
                        break;
                    }
                },
                Err(_) => {
                    // Simulation failed, try smaller investment
                    investment = investment / U256::from(2);
                    if investment < U256::from(1000000000000000u64) {
                        break;
                    }
                }
            }
        }
        
        debug!("Parameter search completed. Best revenue: {}", best_revenue);
        Ok((best_revenue, best_state))
    }

    /// Query spot price between two tokens
    fn query_spot_price(&self, pair: (TokenId, TokenId), state: &StateSnapshot) -> DeFiResult<f64> {
        // Check direct price mapping
        if let Some(&price) = state.spot_prices.get(&pair) {
            return Ok(price);
        }
        
        // Calculate from pool reserves
        for pool_info in state.pools.values() {
            if (pool_info.token_a == pair.0 && pool_info.token_b == pair.1) ||
               (pool_info.token_a == pair.1 && pool_info.token_b == pair.0) {
                return self.calculate_spot_price_from_pool(pool_info, &pair);
            }
        }
        
        Err(DeFiAnalyzerError::InvalidInput(format!("No price found for pair: {:?}", pair)))
    }

    /// Calculate spot price from pool information
    fn calculate_spot_price_from_pool(&self, pool: &PoolInfo, pair: &(TokenId, TokenId)) -> DeFiResult<f64> {
        match &pool.pool_data {
            PoolData::UniswapV2 { reserve_a, reserve_b } => {
                if pool.token_a == pair.0 && pool.token_b == pair.1 {
                    let price = reserve_b.as_limbs()[0] as f64 / reserve_a.as_limbs()[0] as f64;
                    Ok(price)
                } else {
                    let price = reserve_a.as_limbs()[0] as f64 / reserve_b.as_limbs()[0] as f64;
                    Ok(price)
                }
            },
            PoolData::UniswapV3 { sqrt_price, .. } => {
                // Simplified V3 price calculation
                let price = (sqrt_price.as_limbs()[0] as f64 / (2.0_f64.powi(96))).powi(2);
                Ok(price)
            },
            PoolData::Curve { balances, .. } => {
                // Simplified Curve price calculation
                if balances.len() >= 2 {
                    let price = balances[1].as_limbs()[0] as f64 / balances[0].as_limbs()[0] as f64;
                    Ok(price)
                } else {
                    Ok(1.0)
                }
            }
        }
    }

    /// Find shortest path between two tokens
    fn find_shortest_path(&self, from: &TokenId, to: &TokenId, state: &StateSnapshot) -> DeFiResult<Vec<TokenId>> {
        // Simple BFS for shortest path
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent: HashMap<TokenId, TokenId> = HashMap::new();
        
        queue.push_back(from.clone());
        visited.insert(from.clone());
        
        while let Some(current) = queue.pop_front() {
            if current == *to {
                // Reconstruct path
                let mut path = Vec::new();
                let mut node = to.clone();
                path.push(node.clone());
                
                while let Some(p) = parent.get(&node) {
                    path.push(p.clone());
                    node = p.clone();
                }
                
                path.reverse();
                return Ok(path);
            }
            
            // Add neighbors
            for pool in state.pools.values() {
                let next = if pool.token_a == current {
                    Some(pool.token_b.clone())
                } else if pool.token_b == current {
                    Some(pool.token_a.clone())
                } else {
                    None
                };
                
                if let Some(next_token) = next {
                    if !visited.contains(&next_token) {
                        visited.insert(next_token.clone());
                        parent.insert(next_token.clone(), current.clone());
                        queue.push_back(next_token);
                    }
                }
            }
        }
        
        Err(DeFiAnalyzerError::InvalidInput(format!("No path found from {} to {}", from, to)))
    }

    /// Build pool sequence for token path
    fn build_pool_sequence(&self, path: &[TokenId]) -> DeFiResult<Vec<PoolId>> {
        let mut pools = Vec::new();
        
        for i in 0..path.len() - 1 {
            let from = &path[i];
            let to = &path[i + 1];
            
            if let Some(pool_id) = self.find_pool_for_pair(from, to) {
                pools.push(pool_id);
            } else {
                return Err(DeFiAnalyzerError::InvalidInput(
                    format!("No pool found for pair: {} -> {}", from, to)
                ));
            }
        }
        
        Ok(pools)
    }

    /// Find pool for token pair
    fn find_pool_for_pair(&self, from: &TokenId, to: &TokenId) -> Option<PoolId> {
        // This would search the state snapshot for appropriate pools
        // For now, return a synthetic pool ID
        Some(format!("{}_{}_pool", from, to))
    }

    /// Estimate gas cost for pool sequence
    fn estimate_gas_cost(&self, pools: &[PoolId]) -> U256 {
        let total_gas = self.config.base_gas_cost * pools.len() as u64;
        self.config.gas_price * U256::from(total_gas)
    }

    /// Simulate arbitrage execution
    async fn simulate_arbitrage_execution(&self, path: &ArbitragePath, investment: U256, state: &StateSnapshot) -> DeFiResult<(U256, StateSnapshot)> {
        // This would use the EVM interpreter to simulate the arbitrage execution
        // For now, return simplified simulation results
        
        let simulated_revenue = if path.net_profit > U256::ZERO {
            investment + path.net_profit
        } else {
            investment / U256::from(2) // Loss scenario
        };
        
        let mut new_state = state.clone();
        new_state.block_number += 1; // Simulate state change
        
        Ok((simulated_revenue, new_state))
    }
}

impl StateSnapshot {
    /// Create new empty state snapshot
    pub fn new() -> Self {
        Self {
            block_number: 0,
            token_reserves: HashMap::new(),
            spot_prices: HashMap::new(),
            pools: HashMap::new(),
            tokens: HashMap::new(),
        }
    }
    
    /// Create state snapshot from analysis event
    pub fn from_analysis_event(event: &AnalysisEvent) -> Self {
        let mut snapshot = Self::new();
        snapshot.block_number = event.block_number;
        
        // Parse event data to populate snapshot
        // This would extract pool and token information from the event
        
        snapshot
    }
}

impl TradingGraph {
    /// Create new empty trading graph
    pub fn new() -> Self {
        Self {
            nodes: HashSet::new(),
            edges: HashMap::new(),
            adjacency_list: HashMap::new(),
        }
    }
}
