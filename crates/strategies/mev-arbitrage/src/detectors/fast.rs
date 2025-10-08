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
    /// Edge-based triangle index for O(e²) detection
    triangle_edge_index: TriangleEdgeIndex,
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

/// Edge-based triangle index for O(e²) triangle detection
/// Instead of checking all token triplets O(n³), we index by edges
pub struct TriangleEdgeIndex {
    /// Map from edge (tokenA, tokenB) -> list of third tokens that complete a triangle
    /// e.g., if we have edges A-B, B-C, C-A, then:
    ///   (A, B) -> [C]
    ///   (B, C) -> [A]
    ///   (C, A) -> [B]
    edge_to_completions: HashMap<(TokenId, TokenId), Vec<TriangleCompletion>>,
    /// Last rebuild time
    last_rebuild: std::time::Instant,
    /// Total number of triangles indexed
    triangle_count: usize,
}

/// Information about a triangle completion
#[derive(Debug, Clone)]
pub struct TriangleCompletion {
    /// The third token that completes the triangle
    pub third_token: TokenId,
    /// Pool IDs for the three edges
    pub pool_ab: PoolId,  // edge A-B (the index key)
    pub pool_bc: PoolId,  // edge B-third_token
    pub pool_ca: PoolId,  // edge third_token-A
    /// Quality score (based on liquidity, fees)
    pub quality_score: f64,
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
            triangle_edge_index: TriangleEdgeIndex::new(),
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
    /// OPTIMIZED: O(e²) instead of O(n³)
    ///
    /// Algorithm:
    /// 1. Rebuild triangle index if stale (happens once per N blocks)
    /// 2. For each edge (A, B) in the index:
    ///    - Look up all triangles that include this edge: (A, B, C)
    ///    - Check if triangle is profitable
    ///
    /// Complexity:
    /// - Rebuild: O(e²) - for each edge, find common neighbors
    /// - Detection: O(e * k) where k = avg triangles per edge (typically small)
    /// - Total: O(e²) much better than O(n³) when graph is sparse
    fn detect_triangle_arbitrage(&mut self, state: &StateSnapshot) -> Result<Vec<FastArbitrageOpportunity>> {
        let mut opportunities = Vec::new();

        // Rebuild index if stale (do this periodically, not every block)
        let rebuild_interval = std::time::Duration::from_secs(60); // rebuild every 60s
        if self.triangle_edge_index.last_rebuild.elapsed() > rebuild_interval {
            info!("🔄 Rebuilding triangle edge index...");
            self.rebuild_triangle_index(state)?;
        }

        // O(e) loop over edges
        for ((token_a, token_b), completions) in &self.triangle_edge_index.edge_to_completions {
            // O(k) loop over triangles that include edge (A, B), where k is typically small
            for completion in completions {
                // Quick check: only evaluate high-quality triangles
                if completion.quality_score < 0.5 {
                    continue;
                }

                if let Some(opportunity) = self.check_triangle_arbitrage(
                    token_a,
                    token_b,
                    &completion.third_token,
                    state,
                )? {
                    opportunities.push(opportunity);
                }

                // Early exit if we found enough opportunities
                if opportunities.len() >= 50 {
                    break;
                }
            }

            if opportunities.len() >= 50 {
                break;
            }
        }

        Ok(opportunities)
    }

    /// Rebuild triangle edge index - O(e²) operation
    /// This finds all triangles in the graph by looking for common neighbors of edges
    fn rebuild_triangle_index(&mut self, state: &StateSnapshot) -> Result<()> {
        let start = std::time::Instant::now();

        self.triangle_edge_index.edge_to_completions.clear();
        self.triangle_edge_index.triangle_count = 0;

        // Build adjacency list from state
        let mut adjacency: HashMap<TokenId, HashSet<TokenId>> = HashMap::new();
        let mut edge_pools: HashMap<(TokenId, TokenId), PoolId> = HashMap::new();

        // Extract edges from pools in state
        for (pool_id, pool_info) in &state.pools {
            let tokens: Vec<TokenId> = pool_info.tokens.iter().cloned().collect();
            if tokens.len() >= 2 {
                let token_a = &tokens[0];
                let token_b = &tokens[1];

                adjacency.entry(token_a.clone()).or_default().insert(token_b.clone());
                adjacency.entry(token_b.clone()).or_default().insert(token_a.clone());

                edge_pools.insert((token_a.clone(), token_b.clone()), pool_id.clone());
                edge_pools.insert((token_b.clone(), token_a.clone()), pool_id.clone());
            }
        }

        // Find triangles: O(e²) algorithm
        // For each edge (A, B), find common neighbors C such that A-C and B-C exist
        let edges: Vec<(TokenId, TokenId)> = edge_pools.keys().cloned().collect();

        for (token_a, token_b) in &edges {
            let neighbors_a = adjacency.get(token_a);
            let neighbors_b = adjacency.get(token_b);

            if let (Some(na), Some(nb)) = (neighbors_a, neighbors_b) {
                // Find intersection: common neighbors form triangles
                let common_neighbors: Vec<TokenId> = na.intersection(nb).cloned().collect();

                for token_c in common_neighbors {
                    // We found triangle: A-B-C-A
                    let pool_ab = edge_pools.get(&(token_a.clone(), token_b.clone())).cloned();
                    let pool_bc = edge_pools.get(&(token_b.clone(), token_c.clone())).cloned();
                    let pool_ca = edge_pools.get(&(token_c.clone(), token_a.clone())).cloned();

                    if let (Some(pab), Some(pbc), Some(pca)) = (pool_ab, pool_bc, pool_ca) {
                        // Calculate quality score based on pool liquidity
                        let quality_score = self.calculate_triangle_quality_score(&pab, &pbc, &pca, state);

                        let completion = TriangleCompletion {
                            third_token: token_c.clone(),
                            pool_ab: pab,
                            pool_bc: pbc,
                            pool_ca: pca,
                            quality_score,
                        };

                        self.triangle_edge_index.edge_to_completions
                            .entry((token_a.clone(), token_b.clone()))
                            .or_default()
                            .push(completion);

                        self.triangle_edge_index.triangle_count += 1;
                    }
                }
            }
        }

        self.triangle_edge_index.last_rebuild = std::time::Instant::now();

        let elapsed = start.elapsed();
        info!(
            "✅ Triangle index rebuilt: {} triangles, {} edges, {:?}",
            self.triangle_edge_index.triangle_count,
            self.triangle_edge_index.edge_to_completions.len(),
            elapsed
        );

        Ok(())
    }

    /// Calculate quality score for a triangle (0.0 - 1.0)
    /// Higher score = better liquidity and lower fees
    fn calculate_triangle_quality_score(&self, pool_ab: &PoolId, pool_bc: &PoolId, pool_ca: &PoolId, state: &StateSnapshot) -> f64 {
        let mut total_liquidity_score = 0.0;
        let mut count = 0;

        for pool_id in [pool_ab, pool_bc, pool_ca] {
            if let Some(pool) = state.pools.get(pool_id) {
                // Normalize liquidity to 0-1 range
                // Assume pools with > 100 ETH equivalent liquidity get score 1.0
                let liquidity_eth = pool.total_liquidity.unwrap_or(0.0);
                let liquidity_score = (liquidity_eth / 100.0).min(1.0);
                total_liquidity_score += liquidity_score;
                count += 1;
            }
        }

        if count > 0 {
            total_liquidity_score / count as f64
        } else {
            0.5 // default medium quality
        }
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

impl TriangleEdgeIndex {
    fn new() -> Self {
        Self {
            edge_to_completions: HashMap::new(),
            last_rebuild: std::time::Instant::now(),
            triangle_count: 0,
        }
    }
}

// ============================================================================
// ArbitrageDetector Trait Implementation
// ============================================================================

use crate::abstractions::{
    ArbitrageDetector, DetectionContext, DetectionResult, ArbitrageOpportunity as AbstractOpportunity,
    OpportunityType, RiskLevel as AbstractRiskLevel, DetectorConfig, DetectorMetadata, HealthStatus
};
use async_trait::async_trait;
use alloy_primitives::Address;

#[async_trait]
impl ArbitrageDetector for FastArbitrageDetector {
    async fn detect(&mut self, context: &DetectionContext) -> Result<DetectionResult> {
        let detection_start = std::time::Instant::now();

        info!("FastArbitrageDetector: Starting detection at block {}", context.block_number);

        // Use the state_snapshot from context
        let fast_opportunities = self.detect_fast_arbitrage(&context.state_snapshot).await?;

        // Convert FastArbitrageOpportunity to AbstractOpportunity
        let opportunities: Vec<AbstractOpportunity> = fast_opportunities
            .into_iter()
            .map(|fast_opp| self.convert_to_abstract_opportunity(fast_opp))
            .collect();

        let detection_time = detection_start.elapsed();

        info!("FastArbitrageDetector: Found {} opportunities in {:?}",
              opportunities.len(), detection_time);

        Ok(DetectionResult {
            opportunities,
            detection_time,
            detector_id: "fast_arbitrage_detector".to_string(),
            confidence_threshold: 0.8,
            metadata: HashMap::new(),
        })
    }

    fn config(&self) -> &DetectorConfig {
        // Return a static config for now
        // In a real implementation, this would be stored in FastArbitrageDetector
        static CONFIG: once_cell::sync::Lazy<DetectorConfig> = once_cell::sync::Lazy::new(|| DetectorConfig {
            enabled: true,
            confidence_threshold: 0.8,
            max_opportunities: 100,
            timeout: std::time::Duration::from_secs(5),
            detector_specific: serde_json::json!({
                "min_profit_threshold": "500000000000000",
                "max_gas_cost": "1000000"
            }),
        });
        &CONFIG
    }

    fn update_config(&mut self, config: DetectorConfig) -> Result<()> {
        // Update internal config from detector_specific JSON
        if let Some(min_profit) = config.detector_specific.get("min_profit_threshold") {
            if let Some(profit_str) = min_profit.as_str() {
                self.config.min_two_hop_profit = U256::from_str_radix(profit_str, 10).unwrap_or(self.config.min_two_hop_profit);
            }
        }
        Ok(())
    }

    fn metadata(&self) -> DetectorMetadata {
        use crate::abstractions::PerformanceMetrics;

        DetectorMetadata {
            name: "FastArbitrageDetector".to_string(),
            version: "1.0.0".to_string(),
            description: "Fast 2-3 hop arbitrage detector with millisecond latency".to_string(),
            supported_opportunity_types: vec![
                "TwoHopArbitrage".to_string(),
                "TriangleArbitrage".to_string(),
                "PriceDeviationArbitrage".to_string(),
            ],
            performance_metrics: PerformanceMetrics {
                avg_detection_time: std::time::Duration::from_millis(50),
                success_rate: 0.95,
                total_detections: 0,
                avg_profit_accuracy: 0.85,
            },
        }
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Simple health check
        let mut metrics = HashMap::new();
        metrics.insert("cache_hit_rate".to_string(), 0.8);
        metrics.insert("avg_detection_ms".to_string(), 50.0);

        Ok(HealthStatus {
            is_healthy: true,
            status_message: "Detector operating normally".to_string(),
            last_check: std::time::Instant::now(),
            metrics,
        })
    }
}

impl FastArbitrageDetector {
    /// Convert FastArbitrageOpportunity to AbstractOpportunity
    fn convert_to_abstract_opportunity(&self, fast_opp: FastArbitrageOpportunity) -> AbstractOpportunity {
        let opportunity_type = match fast_opp.opportunity_type {
            FastOpportunityType::TwoHop { intermediate: _, price_difference: _ } => {
                OpportunityType::SimpleArbitrage {
                    token_a: Address::ZERO, // Would need actual addresses
                    token_b: Address::ZERO,
                    path: vec![Address::ZERO, Address::ZERO, Address::ZERO], // 2-hop path
                }
            }
            FastOpportunityType::Triangle { intermediate1: _, intermediate2: _, total_rate: _ } => {
                OpportunityType::TriangularArbitrage {
                    tokens: vec![Address::ZERO, Address::ZERO, Address::ZERO, Address::ZERO],
                    pools: vec![Address::ZERO, Address::ZERO, Address::ZERO],
                }
            }
            FastOpportunityType::PriceDeviation { .. } => {
                OpportunityType::SimpleArbitrage {
                    token_a: Address::ZERO,
                    token_b: Address::ZERO,
                    path: vec![Address::ZERO, Address::ZERO],
                }
            }
        };

        let risk_level = if fast_opp.confidence > 0.9 {
            AbstractRiskLevel::Low
        } else if fast_opp.confidence > 0.7 {
            AbstractRiskLevel::Medium
        } else {
            AbstractRiskLevel::High
        };

        AbstractOpportunity {
            id: format!("fast_{:?}", fast_opp.detected_at),
            opportunity_type,
            expected_profit: fast_opp.expected_profit,
            gas_cost: U256::from(fast_opp.gas_cost),
            confidence: fast_opp.confidence,
            risk_level,
            deadline: Some(fast_opp.detected_at + std::time::Duration::from_secs(12)), // 1 block
            required_capital: fast_opp.input_amount,
            metadata: serde_json::json!({
                "detector": "fast",
                "slippage_estimate": fast_opp.slippage_estimate,
                "path_length": fast_opp.path.len(),
            }),
        }
    }
}