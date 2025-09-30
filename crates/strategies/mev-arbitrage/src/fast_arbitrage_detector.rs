//! Fast Arbitrage Detection Implementation
//!
//! Optimized detection for 2-3 hop arbitrage opportunities with millisecond latency.

use std::collections::{HashMap, HashSet};
use alloy_primitives::U256;
use tracing::{debug, info};
use anyhow::Result;

use mev_arbitrage_graph::{StateSnapshot, TokenId, PoolId};

/// Fast arbitrage detector optimized for simple 2-3 hop opportunities
pub struct FastArbitrageDetector {
    /// Pre-computed triangle paths for major tokens
    triangle_paths: TrianglePathMap,
    /// Hot token pairs with high update frequency
    hot_pairs: HotPairTracker,
    /// Price deviation tracker
    price_deviation: PriceDeviationTracker,
    /// Two-hop price cache
    two_hop_cache: TwoHopPriceCache,
    /// Configuration
    config: FastDetectorConfig,
}

/// Configuration for fast detector
#[derive(Debug, Clone)]
pub struct FastDetectorConfig {
    /// Minimum profit threshold for 2-hop (wei)
    pub min_two_hop_profit: U256,
    /// Minimum profit threshold for 3-hop (wei)
    pub min_three_hop_profit: U256,
    /// Maximum allowed slippage for fast detection
    pub max_slippage: f64,
    /// Cache size for price data
    pub cache_size: usize,
    /// Hot pairs threshold (updates per minute)
    pub hot_pair_threshold: u32,
    /// Price deviation threshold for opportunity detection
    pub price_deviation_threshold: f64,
}

impl Default for FastDetectorConfig {
    fn default() -> Self {
        Self {
            min_two_hop_profit: U256::from(500_000_000_000_000u64), // 0.0005 ETH
            min_three_hop_profit: U256::from(1_000_000_000_000_000u64), // 0.001 ETH
            max_slippage: 0.02, // 2%
            cache_size: 1000,
            hot_pair_threshold: 10,
            price_deviation_threshold: 0.005, // 0.5%
        }
    }
}

/// Triangle path mapping for efficient lookup
pub struct TrianglePathMap {
    /// Base token -> intermediate -> target mappings
    triangles: HashMap<TokenId, HashMap<TokenId, Vec<TokenId>>>,
    /// Path quality scores
    path_scores: HashMap<(TokenId, TokenId, TokenId), f64>,
    /// Last update timestamp
    last_update: std::time::Instant,
}

/// Hot pair tracker for frequently updated pairs
pub struct HotPairTracker {
    /// Update counts per pair
    update_counts: HashMap<(TokenId, TokenId), u32>,
    /// Last update times
    last_updates: HashMap<(TokenId, TokenId), std::time::Instant>,
    /// Current hot pairs
    hot_pairs: HashSet<(TokenId, TokenId)>,
}

/// Price deviation tracker for detecting arbitrage opportunities
pub struct PriceDeviationTracker {
    /// Reference prices (baseline)
    reference_prices: HashMap<(TokenId, TokenId), f64>,
    /// Current price deviations
    current_deviations: HashMap<(TokenId, TokenId), f64>,
    /// Deviation history for trend analysis
    deviation_history: HashMap<(TokenId, TokenId), Vec<(std::time::Instant, f64)>>,
}

/// Two-hop price cache for fast lookups
pub struct TwoHopPriceCache {
    /// Direct prices (token A -> token B)
    direct_prices: HashMap<(TokenId, TokenId), PriceCacheEntry>,
    /// Two-hop prices (token A -> intermediate -> token B)
    two_hop_prices: HashMap<(TokenId, TokenId), Vec<TwoHopPath>>,
    /// Cache metadata
    cache_stats: CacheStats,
}

/// Cached price entry
#[derive(Debug, Clone)]
pub struct PriceCacheEntry {
    pub price: f64,
    pub liquidity: U256,
    pub timestamp: std::time::Instant,
    pub pool_id: PoolId,
    pub fee: f64,
}

/// Two-hop arbitrage path
#[derive(Debug, Clone)]
pub struct TwoHopPath {
    pub intermediate: TokenId,
    pub price: f64,
    pub combined_liquidity: U256,
    pub total_fee: f64,
    pub pools: Vec<PoolId>,
    pub estimated_gas: u64,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub updates: u64,
    pub evictions: u64,
}

/// Fast arbitrage opportunity
#[derive(Debug, Clone)]
pub struct FastArbitrageOpportunity {
    /// Opportunity type
    pub opportunity_type: FastOpportunityType,
    /// Token path
    pub path: Vec<TokenId>,
    /// Pool path
    pub pools: Vec<PoolId>,
    /// Expected profit (wei)
    pub expected_profit: U256,
    /// Required input amount (wei)
    pub input_amount: U256,
    /// Expected output amount (wei)
    pub output_amount: U256,
    /// Estimated gas cost
    pub gas_cost: u64,
    /// Slippage estimate
    pub slippage_estimate: f64,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Detection timestamp
    pub detected_at: std::time::Instant,
}

/// Fast opportunity types
#[derive(Debug, Clone)]
pub enum FastOpportunityType {
    /// Two-hop arbitrage (A -> B -> A)
    TwoHop {
        intermediate: TokenId,
        price_difference: f64,
    },
    /// Triangle arbitrage (A -> B -> C -> A)
    Triangle {
        intermediate1: TokenId,
        intermediate2: TokenId,
        total_rate: f64,
    },
    /// Price deviation opportunity
    PriceDeviation {
        deviation_percent: f64,
        reference_price: f64,
        current_price: f64,
    },
}

impl FastArbitrageDetector {
    /// Create new fast arbitrage detector
    pub fn new(config: FastDetectorConfig) -> Self {
        Self {
            triangle_paths: TrianglePathMap::new(),
            hot_pairs: HotPairTracker::new(),
            price_deviation: PriceDeviationTracker::new(),
            two_hop_cache: TwoHopPriceCache::new(config.cache_size),
            config,
        }
    }

    /// Detect fast arbitrage opportunities
    pub async fn detect_fast_arbitrage(
        &mut self,
        state: &StateSnapshot,
    ) -> Result<Vec<FastArbitrageOpportunity>> {
        debug!("Starting fast arbitrage detection");
        let start_time = std::time::Instant::now();

        let mut opportunities = Vec::new();

        // Update caches and trackers
        self.update_price_caches(state)?;
        self.update_hot_pairs(state)?;
        self.update_price_deviations(state)?;

        // 1. Fast two-hop detection
        let two_hop_opportunities = self.detect_two_hop_arbitrage(state)?;
        opportunities.extend(two_hop_opportunities);

        // 2. Triangle arbitrage detection (for hot pairs only)
        let triangle_opportunities = self.detect_triangle_arbitrage(state)?;
        opportunities.extend(triangle_opportunities);

        // 3. Price deviation opportunities
        let deviation_opportunities = self.detect_price_deviation_opportunities(state)?;
        opportunities.extend(deviation_opportunities);

        // Filter and sort by profitability
        opportunities.retain(|op| op.expected_profit > self.config.min_two_hop_profit);
        opportunities.sort_by(|a, b| b.expected_profit.cmp(&a.expected_profit));

        let detection_time = start_time.elapsed();
        info!("Fast detection completed in {:?}, found {} opportunities",
              detection_time, opportunities.len());

        Ok(opportunities)
    }

    /// Detect two-hop arbitrage opportunities (A -> B -> A)
    fn detect_two_hop_arbitrage(&mut self, state: &StateSnapshot) -> Result<Vec<FastArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // Check all token pairs in hot pairs first
        for (token_a, token_b) in &self.hot_pairs.hot_pairs {
            if let Some(opportunity) = self.check_two_hop_arbitrage(token_a, token_b, state)? {
                opportunities.push(opportunity);
            }
        }

        // Check other major pairs if we have capacity
        if opportunities.len() < 10 {
            for token_a in state.tokens.keys() {
                for token_b in state.tokens.keys() {
                    if token_a != token_b && !self.hot_pairs.hot_pairs.contains(&(token_a.clone(), token_b.clone())) {
                        if let Some(opportunity) = self.check_two_hop_arbitrage(token_a, token_b, state)? {
                            opportunities.push(opportunity);
                        }
                    }
                }
            }
        }

        Ok(opportunities)
    }

    /// Check two-hop arbitrage for a specific pair
    fn check_two_hop_arbitrage(
        &self,
        token_a: &TokenId,
        token_b: &TokenId,
        state: &StateSnapshot,
    ) -> Result<Option<FastArbitrageOpportunity>> {
        // Get cached two-hop paths
        if let Some(paths) = self.two_hop_cache.two_hop_prices.get(&(token_a.clone(), token_b.clone())) {
            for path in paths {
                // Calculate arbitrage potential
                let direct_price = self.get_direct_price(token_a, token_b, state)?;
                let two_hop_price = path.price;

                let price_difference = (two_hop_price - direct_price) / direct_price;

                if price_difference.abs() > self.config.price_deviation_threshold {
                    // Calculate potential profit
                    let input_amount = U256::from(1_000_000_000_000_000_000u64); // 1 ETH test amount
                    let direct_output = self.calculate_output_amount(input_amount, direct_price)?;
                    let two_hop_output = self.calculate_two_hop_output(input_amount, path)?;

                    if two_hop_output > direct_output {
                        let profit = two_hop_output - direct_output;
                        let gas_cost = path.estimated_gas;
                        let gas_cost_wei = U256::from(gas_cost * 20_000_000_000u64); // 20 gwei

                        if profit > gas_cost_wei && profit > self.config.min_two_hop_profit {
                            return Ok(Some(FastArbitrageOpportunity {
                                opportunity_type: FastOpportunityType::TwoHop {
                                    intermediate: path.intermediate.clone(),
                                    price_difference,
                                },
                                path: vec![token_a.clone(), path.intermediate.clone(), token_b.clone()],
                                pools: path.pools.clone(),
                                expected_profit: profit,
                                input_amount,
                                output_amount: two_hop_output,
                                gas_cost,
                                slippage_estimate: path.total_fee,
                                confidence: self.calculate_two_hop_confidence(path, price_difference),
                                detected_at: std::time::Instant::now(),
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Detect triangle arbitrage opportunities (A -> B -> C -> A)
    fn detect_triangle_arbitrage(&mut self, state: &StateSnapshot) -> Result<Vec<FastArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // Only check pre-computed triangle paths for hot tokens
        for (base_token, intermediates) in &self.triangle_paths.triangles {
            for (intermediate1, targets) in intermediates {
                for intermediate2 in targets {
                    if let Some(opportunity) = self.check_triangle_arbitrage(
                        base_token,
                        intermediate1,
                        intermediate2,
                        state,
                    )? {
                        opportunities.push(opportunity);
                    }
                }
            }
        }

        Ok(opportunities)
    }

    /// Check triangle arbitrage for specific tokens
    fn check_triangle_arbitrage(
        &self,
        token_a: &TokenId,
        token_b: &TokenId,
        token_c: &TokenId,
        state: &StateSnapshot,
    ) -> Result<Option<FastArbitrageOpportunity>> {
        // Get prices for all three pairs
        let price_ab = self.get_direct_price(token_a, token_b, state)?;
        let price_bc = self.get_direct_price(token_b, token_c, state)?;
        let price_ca = self.get_direct_price(token_c, token_a, state)?;

        // Calculate triangle rate
        let triangle_rate = price_ab * price_bc * price_ca;

        // Check if triangle rate indicates arbitrage (should be != 1.0)
        let rate_deviation = (triangle_rate - 1.0).abs();

        if rate_deviation > self.config.price_deviation_threshold {
            // Calculate potential profit
            let input_amount = U256::from(1_000_000_000_000_000_000u64); // 1 ETH

            // Simulate triangle execution
            let amount_after_ab = self.calculate_output_amount(input_amount, price_ab)?;
            let amount_after_bc = self.calculate_output_amount(amount_after_ab, price_bc)?;
            let final_amount = self.calculate_output_amount(amount_after_bc, price_ca)?;

            if final_amount > input_amount {
                let profit = final_amount - input_amount;
                let gas_cost = 450_000u64; // 3 swaps
                let gas_cost_wei = U256::from(gas_cost * 20_000_000_000u64);

                if profit > gas_cost_wei && profit > self.config.min_three_hop_profit {
                    return Ok(Some(FastArbitrageOpportunity {
                        opportunity_type: FastOpportunityType::Triangle {
                            intermediate1: token_b.clone(),
                            intermediate2: token_c.clone(),
                            total_rate: triangle_rate,
                        },
                        path: vec![token_a.clone(), token_b.clone(), token_c.clone(), token_a.clone()],
                        pools: self.get_triangle_pools(token_a, token_b, token_c, state)?,
                        expected_profit: profit,
                        input_amount,
                        output_amount: final_amount,
                        gas_cost,
                        slippage_estimate: 0.01, // Estimated 1% total slippage
                        confidence: self.calculate_triangle_confidence(triangle_rate),
                        detected_at: std::time::Instant::now(),
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Detect price deviation opportunities
    fn detect_price_deviation_opportunities(&mut self, _state: &StateSnapshot) -> Result<Vec<FastArbitrageOpportunity>> {
        let opportunities = Vec::new();

        // Check current deviations against reference prices
        for ((token_a, token_b), deviation) in &self.price_deviation.current_deviations {
            if deviation.abs() > self.config.price_deviation_threshold {
                if let Some(ref_price) = self.price_deviation.reference_prices.get(&(token_a.clone(), token_b.clone())) {
                    // This indicates a potential arbitrage opportunity
                    // Implementation would create appropriate FastArbitrageOpportunity
                    debug!("Price deviation detected for {}-{}: {:.4}%", token_a, token_b, deviation * 100.0);
                }
            }
        }

        Ok(opportunities)
    }

    /// Helper methods for calculations

    fn get_direct_price(&self, token_a: &TokenId, token_b: &TokenId, state: &StateSnapshot) -> Result<f64> {
        if let Some(price) = state.spot_prices.get(&(token_a.clone(), token_b.clone())) {
            Ok(*price)
        } else {
            // Fallback to cache or calculate from pools
            Ok(1.0) // Placeholder
        }
    }

    fn calculate_output_amount(&self, input: U256, price: f64) -> Result<U256> {
        // Simplified calculation - in practice would consider fees, slippage
        let input_f64 = input.as_limbs()[0] as f64;
        let output_f64 = input_f64 * price * 0.997; // 0.3% fee
        Ok(U256::from(output_f64 as u64))
    }

    fn calculate_two_hop_output(&self, input: U256, path: &TwoHopPath) -> Result<U256> {
        let input_f64 = input.as_limbs()[0] as f64;
        let output_f64 = input_f64 * path.price * (1.0 - path.total_fee);
        Ok(U256::from(output_f64 as u64))
    }

    fn calculate_two_hop_confidence(&self, _path: &TwoHopPath, price_difference: f64) -> f64 {
        // Higher confidence for larger price differences, but cap at reasonable levels
        (price_difference.abs() * 10.0).min(0.95).max(0.1)
    }

    fn calculate_triangle_confidence(&self, triangle_rate: f64) -> f64 {
        let deviation = (triangle_rate - 1.0).abs();
        (deviation * 20.0).min(0.95).max(0.1)
    }

    fn get_triangle_pools(&self, token_a: &TokenId, token_b: &TokenId, token_c: &TokenId, _state: &StateSnapshot) -> Result<Vec<PoolId>> {
        // Simplified - would look up actual pools
        Ok(vec![
            format!("{}_{}_pool", token_a, token_b),
            format!("{}_{}_pool", token_b, token_c),
            format!("{}_{}_pool", token_c, token_a),
        ])
    }

    /// Update methods for caches and trackers

    fn update_price_caches(&mut self, _state: &StateSnapshot) -> Result<()> {
        // Update price caches with latest state
        self.two_hop_cache.cache_stats.updates += 1;
        Ok(())
    }

    fn update_hot_pairs(&mut self, _state: &StateSnapshot) -> Result<()> {
        // Update hot pairs based on recent activity
        Ok(())
    }

    fn update_price_deviations(&mut self, _state: &StateSnapshot) -> Result<()> {
        // Update price deviation tracking
        Ok(())
    }
}

// Implementation for supporting structures

impl TrianglePathMap {
    fn new() -> Self {
        Self {
            triangles: HashMap::new(),
            path_scores: HashMap::new(),
            last_update: std::time::Instant::now(),
        }
    }
}

impl HotPairTracker {
    fn new() -> Self {
        Self {
            update_counts: HashMap::new(),
            last_updates: HashMap::new(),
            hot_pairs: HashSet::new(),
        }
    }
}

impl PriceDeviationTracker {
    fn new() -> Self {
        Self {
            reference_prices: HashMap::new(),
            current_deviations: HashMap::new(),
            deviation_history: HashMap::new(),
        }
    }
}

impl TwoHopPriceCache {
    fn new(_capacity: usize) -> Self {
        Self {
            direct_prices: HashMap::new(),
            two_hop_prices: HashMap::new(),
            cache_stats: CacheStats::default(),
        }
    }
}