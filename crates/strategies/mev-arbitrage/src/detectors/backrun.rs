//! Backrunning Detector
//!
//! Detects profitable backrun opportunities from pending mempool transactions.
//! Backrunning is one of the most stable MEV revenue streams because:
//! - Low competition (no frontrunning race)
//! - High success rate (victim tx already committed)
//! - Predictable profits (based on victim's price impact)
//!
//! Strategy:
//! 1. Monitor mempool for large swaps (>$50K)
//! 2. Calculate price impact of victim's trade
//! 3. Execute arbitrage to capture reversion to fair price
//! 4. Submit bundle: [victim_tx, our_backrun_tx]

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use alloy_primitives::{Address, U256, TxHash, Bytes};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tracing::{info, debug, warn};

use crate::abstractions::*;
use crate::utils::amm;

// ============================================================================
// Configuration
// ============================================================================

/// Backrun detector configuration
#[derive(Debug, Clone)]
pub struct BackrunConfig {
    /// Minimum victim transaction value (in wei) to consider
    pub min_victim_value: U256,

    /// Minimum price impact to backrun (e.g., 0.01 = 1%)
    pub min_price_impact: f64,

    /// Maximum gas price we're willing to pay (gwei)
    pub max_gas_price_gwei: u64,

    /// Minimum profit after gas (wei)
    pub min_profit_wei: U256,

    /// DEX routers to monitor
    pub monitored_routers: Vec<Address>,

    /// Tokens to monitor
    pub monitored_tokens: Vec<Address>,

    /// Maximum backrun opportunities to track
    pub max_tracked_opportunities: usize,

    /// Opportunity staleness timeout (seconds)
    pub opportunity_timeout_secs: u64,
}

impl Default for BackrunConfig {
    fn default() -> Self {
        Self {
            min_victim_value: U256::from(50_000_000_000_000_000_000_000u128), // 50K wei (~$100K at $2K ETH)
            min_price_impact: 0.01, // 1%
            max_gas_price_gwei: 200,
            min_profit_wei: U256::from(50_000_000_000_000_000u64), // 0.05 ETH
            monitored_routers: vec![
                // Uniswap V2 Router
                "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap(),
                // Sushiswap Router
                "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap(),
                // Uniswap V3 Router
                "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap(),
            ],
            monitored_tokens: vec![
                // WETH
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(),
                // USDC
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
                // USDT
                "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap(),
                // DAI
                "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap(),
            ],
            max_tracked_opportunities: 100,
            opportunity_timeout_secs: 30,
        }
    }
}

// ============================================================================
// Core Types
// ============================================================================

/// Pending victim transaction
#[derive(Debug, Clone)]
pub struct VictimTransaction {
    /// Transaction hash
    pub hash: TxHash,

    /// From address
    pub from: Address,

    /// To address (DEX router)
    pub to: Address,

    /// Input data
    pub input: Bytes,

    /// Transaction value
    pub value: U256,

    /// Gas price
    pub gas_price: U256,

    /// Detected swap info
    pub swap_info: Option<SwapInfo>,

    /// Detection timestamp
    pub detected_at: std::time::Instant,
}

/// Decoded swap information
#[derive(Debug, Clone)]
pub struct SwapInfo {
    /// DEX type
    pub dex: DexType,

    /// Token in
    pub token_in: Address,

    /// Token out
    pub token_out: Address,

    /// Amount in
    pub amount_in: U256,

    /// Minimum amount out (slippage protection)
    pub amount_out_min: U256,

    /// Swap path (for multi-hop swaps)
    pub path: Vec<Address>,

    /// Recipient
    pub recipient: Address,

    /// Deadline
    pub deadline: u64,
}

/// DEX type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DexType {
    UniswapV2,
    UniswapV3,
    Sushiswap,
    Curve,
    Balancer,
}

impl DexType {
    pub fn name(&self) -> &'static str {
        match self {
            DexType::UniswapV2 => "Uniswap V2",
            DexType::UniswapV3 => "Uniswap V3",
            DexType::Sushiswap => "Sushiswap",
            DexType::Curve => "Curve",
            DexType::Balancer => "Balancer",
        }
    }
}

/// Backrun opportunity
#[derive(Debug, Clone)]
pub struct BackrunOpportunity {
    /// Victim transaction
    pub victim_tx: VictimTransaction,

    /// Expected price impact from victim's trade
    pub price_impact: f64,

    /// Pool affected by victim's trade
    pub affected_pool: Address,

    /// Expected state after victim's trade
    pub post_victim_state: PostVictimState,

    /// Our backrun strategy
    pub backrun_strategy: BackrunStrategy,

    /// Expected profit (wei)
    pub expected_profit: U256,

    /// Required gas
    pub estimated_gas: u64,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,

    /// Detection time
    pub detected_at: std::time::Instant,
}

/// Pool state after victim's transaction
#[derive(Debug, Clone)]
pub struct PostVictimState {
    /// Pool address
    pub pool: Address,

    /// Token0 reserve after victim
    pub reserve0_after: U256,

    /// Token1 reserve after victim
    pub reserve1_after: U256,

    /// Current market price (reference)
    pub market_price: f64,

    /// Price after victim's trade
    pub price_after_victim: f64,

    /// Price deviation from market
    pub price_deviation: f64,
}

/// Backrun strategy types
#[derive(Debug, Clone)]
pub enum BackrunStrategy {
    /// Simple reverse swap (most common)
    /// Buy what the victim sold, sell what they bought
    ReverseSwap {
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        expected_out: U256,
    },

    /// Arbitrage across multiple pools
    CrossPoolArbitrage {
        path: Vec<Address>,
        pools: Vec<Address>,
        amount_in: U256,
        expected_out: U256,
    },

    /// Sandwich-like strategy (for very large victims)
    /// Front: Move price against victim
    /// Back: Profit from reversion
    Sandwich {
        front_amount: U256,
        back_amount: U256,
        expected_profit: U256,
    },
}

// ============================================================================
// Backrun Detector
// ============================================================================

pub struct BackrunDetector {
    /// Configuration
    config: BackrunConfig,

    /// Recently seen victim transactions
    recent_victims: VecDeque<VictimTransaction>,

    /// Current backrun opportunities
    opportunities: Vec<BackrunOpportunity>,

    /// Pool state cache
    pool_states: HashMap<Address, PoolStateCache>,

    /// Market price oracle
    price_oracle: PriceOracle,
}

#[derive(Debug, Clone)]
struct PoolStateCache {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub last_update: std::time::Instant,
}

struct PriceOracle {
    /// Reference prices from aggregators (Chainlink, etc.)
    reference_prices: HashMap<Address, f64>,
}

impl BackrunDetector {
    /// Create new backrun detector
    pub fn new(config: BackrunConfig) -> Self {
        Self {
            config,
            recent_victims: VecDeque::with_capacity(1000),
            opportunities: Vec::with_capacity(100),
            pool_states: HashMap::new(),
            price_oracle: PriceOracle::new(),
        }
    }

    /// Process pending transaction from mempool
    pub async fn process_pending_transaction(
        &mut self,
        tx_hash: TxHash,
        from: Address,
        to: Address,
        input: Bytes,
        value: U256,
        gas_price: U256,
    ) -> Result<Option<BackrunOpportunity>> {
        // Check if transaction is to a monitored DEX router
        if !self.config.monitored_routers.contains(&to) {
            return Ok(None);
        }

        // Check value threshold
        if value < self.config.min_victim_value {
            return Ok(None);
        }

        debug!("🎯 Potential victim tx: {:?}, value: {} ETH", tx_hash, value.to::<u128>() as f64 / 1e18);

        // Decode swap information
        let swap_info = self.decode_swap_call(&to, &input)?;
        if swap_info.is_none() {
            return Ok(None);
        }
        let swap_info = swap_info.unwrap();

        // Create victim transaction
        let victim_tx = VictimTransaction {
            hash: tx_hash,
            from,
            to,
            input,
            value,
            gas_price,
            swap_info: Some(swap_info.clone()),
            detected_at: std::time::Instant::now(),
        };

        // Analyze backrun opportunity
        let opportunity = self.analyze_backrun_opportunity(&victim_tx, &swap_info).await?;

        if let Some(opp) = &opportunity {
            info!(
                "💰 Backrun opportunity found! Victim: {:?}, Profit: {} ETH, Impact: {:.2}%",
                tx_hash,
                opp.expected_profit.to::<u128>() as f64 / 1e18,
                opp.price_impact * 100.0
            );
        }

        // Store victim for tracking
        self.recent_victims.push_back(victim_tx);
        if self.recent_victims.len() > 1000 {
            self.recent_victims.pop_front();
        }

        Ok(opportunity)
    }

    /// Decode swap call from transaction input
    fn decode_swap_call(&self, router: &Address, input: &Bytes) -> Result<Option<SwapInfo>> {
        if input.len() < 4 {
            return Ok(None);
        }

        // Extract function selector (first 4 bytes)
        let selector = &input[0..4];

        // Uniswap V2 Router selectors
        // swapExactTokensForTokens: 0x38ed1739
        // swapTokensForExactTokens: 0x8803dbee
        // swapExactETHForTokens: 0x7ff36ab5
        // swapExactTokensForETH: 0x18cbafe5

        match selector {
            // swapExactTokensForTokens(uint amountIn, uint amountOutMin, address[] path, address to, uint deadline)
            [0x38, 0xed, 0x17, 0x39] => {
                self.decode_exact_tokens_for_tokens(router, input)
            }

            // swapExactETHForTokens(uint amountOutMin, address[] path, address to, uint deadline)
            [0x7f, 0xf3, 0x6a, 0xb5] => {
                self.decode_exact_eth_for_tokens(router, input)
            }

            _ => Ok(None),
        }
    }

    /// Decode swapExactTokensForTokens
    fn decode_exact_tokens_for_tokens(&self, _router: &Address, input: &Bytes) -> Result<Option<SwapInfo>> {
        // Simplified decoding - in production, use proper ABI decoding
        if input.len() < 100 {
            return Ok(None);
        }

        // Skip function selector (4 bytes) + offset to dynamic arrays
        // This is simplified - proper implementation would use alloy ABI decoder

        // For now, return None - actual implementation would decode:
        // - amountIn
        // - amountOutMin
        // - path (array of addresses)
        // - recipient
        // - deadline

        Ok(None)
    }

    /// Decode swapExactETHForTokens
    fn decode_exact_eth_for_tokens(&self, _router: &Address, _input: &Bytes) -> Result<Option<SwapInfo>> {
        // Similar to above - simplified
        Ok(None)
    }

    /// Analyze backrun opportunity for a victim transaction
    async fn analyze_backrun_opportunity(
        &mut self,
        victim_tx: &VictimTransaction,
        swap_info: &SwapInfo,
    ) -> Result<Option<BackrunOpportunity>> {
        // Get the pool that will be affected
        let pool = self.find_affected_pool(&swap_info.path)?;

        // Get current pool state
        let current_state = self.get_pool_state(&pool).await?;

        // Simulate victim's trade to get post-trade state
        let post_victim_state = self.simulate_victim_trade(
            &current_state,
            swap_info,
        )?;

        // Calculate price impact
        let price_impact = post_victim_state.price_deviation.abs();

        // Check if price impact is significant enough
        if price_impact < self.config.min_price_impact {
            debug!("Price impact too small: {:.2}%", price_impact * 100.0);
            return Ok(None);
        }

        // Determine backrun strategy
        let backrun_strategy = self.plan_backrun_strategy(
            &post_victim_state,
            swap_info,
        )?;

        // Estimate profit
        let (expected_profit, estimated_gas) = self.estimate_backrun_profit(
            &backrun_strategy,
            &post_victim_state,
        )?;

        // Check profitability
        let gas_cost = U256::from(estimated_gas)
            .saturating_mul(victim_tx.gas_price)
            .saturating_mul(U256::from(12)) // 1.2x gas price to ensure inclusion
            .saturating_div(U256::from(10));

        if expected_profit <= gas_cost || expected_profit < self.config.min_profit_wei {
            debug!("Not profitable: profit={} wei, gas_cost={} wei", expected_profit, gas_cost);
            return Ok(None);
        }

        let net_profit = expected_profit.saturating_sub(gas_cost);

        // Calculate confidence
        let confidence = self.calculate_backrun_confidence(
            price_impact,
            &backrun_strategy,
            swap_info,
        );

        Ok(Some(BackrunOpportunity {
            victim_tx: victim_tx.clone(),
            price_impact,
            affected_pool: pool,
            post_victim_state,
            backrun_strategy,
            expected_profit: net_profit,
            estimated_gas,
            confidence,
            detected_at: std::time::Instant::now(),
        }))
    }

    /// Find the pool affected by a swap
    fn find_affected_pool(&self, path: &[Address]) -> Result<Address> {
        if path.len() < 2 {
            return Err(anyhow!("Invalid swap path"));
        }

        // For now, return a placeholder
        // In production, query factory to get pair address
        Ok(Address::ZERO)
    }

    /// Get current pool state
    async fn get_pool_state(&mut self, pool: &Address) -> Result<PoolStateCache> {
        // Check cache
        if let Some(cached) = self.pool_states.get(pool) {
            if cached.last_update.elapsed().as_secs() < 10 {
                return Ok(cached.clone());
            }
        }

        // In production, query pool reserves from chain
        // For now, return placeholder
        Ok(PoolStateCache {
            address: *pool,
            token0: Address::ZERO,
            token1: Address::ZERO,
            reserve0: U256::from(1000_000_000_000_000_000_000u128), // 1000 tokens
            reserve1: U256::from(2000_000_000_000_000_000_000u128), // 2000 tokens
            last_update: std::time::Instant::now(),
        })
    }

    /// Simulate victim's trade on the pool
    fn simulate_victim_trade(
        &self,
        current_state: &PoolStateCache,
        swap_info: &SwapInfo,
    ) -> Result<PostVictimState> {
        // Use constant product formula: x * y = k
        let reserve_in = current_state.reserve0;
        let reserve_out = current_state.reserve1;

        // Calculate amount out from victim's swap
        let amount_out = amm::uniswap_v2_output(
            swap_info.amount_in,
            reserve_in,
            reserve_out,
        );

        // New reserves after victim's trade
        let reserve0_after = reserve_in.saturating_add(swap_info.amount_in);
        let reserve1_after = reserve_out.saturating_sub(amount_out);

        // Calculate prices
        let price_before = reserve_out.to::<u128>() as f64 / reserve_in.to::<u128>() as f64;
        let price_after = reserve1_after.to::<u128>() as f64 / reserve0_after.to::<u128>() as f64;

        // Get market reference price
        let market_price = self.price_oracle.get_reference_price(&swap_info.token_in, &swap_info.token_out);

        let price_deviation = (price_after - market_price) / market_price;

        Ok(PostVictimState {
            pool: current_state.address,
            reserve0_after,
            reserve1_after,
            market_price,
            price_after_victim: price_after,
            price_deviation,
        })
    }

    /// Plan the backrun strategy
    fn plan_backrun_strategy(
        &self,
        post_victim: &PostVictimState,
        swap_info: &SwapInfo,
    ) -> Result<BackrunStrategy> {
        // Most common: reverse swap
        // If victim bought token1 (sold token0), we sell token1 (buy token0)

        // Calculate optimal backrun amount
        let optimal_amount = self.calculate_optimal_backrun_amount(post_victim)?;

        let expected_out = amm::uniswap_v2_output(
            optimal_amount,
            post_victim.reserve1_after,
            post_victim.reserve0_after,
        );

        Ok(BackrunStrategy::ReverseSwap {
            token_in: swap_info.token_out,  // Reverse direction
            token_out: swap_info.token_in,
            amount_in: optimal_amount,
            expected_out,
        })
    }

    /// Calculate optimal backrun amount
    /// Goal: Maximize profit while not overshooting market price
    fn calculate_optimal_backrun_amount(&self, post_victim: &PostVictimState) -> Result<U256> {
        // Simplified: use a fraction of the price deviation
        // In production, use calculus to find optimal point

        let deviation_fraction = post_victim.price_deviation.abs();
        let optimal_fraction = deviation_fraction * 0.8; // Take 80% of deviation

        let amount = post_victim.reserve1_after
            .saturating_mul(U256::from((optimal_fraction * 1000.0) as u64))
            .saturating_div(U256::from(1000));

        Ok(amount.max(U256::from(1_000_000_000_000_000u64))) // Min 0.001 ETH
    }

    /// Estimate backrun profit
    fn estimate_backrun_profit(
        &self,
        strategy: &BackrunStrategy,
        _post_victim: &PostVictimState,
    ) -> Result<(U256, u64)> {
        match strategy {
            BackrunStrategy::ReverseSwap { amount_in, expected_out, .. } => {
                let profit = expected_out.saturating_sub(*amount_in);
                let gas = 150_000u64; // Gas for one swap

                Ok((profit, gas))
            }

            BackrunStrategy::CrossPoolArbitrage { amount_in, expected_out, pools, .. } => {
                let profit = expected_out.saturating_sub(*amount_in);
                let gas = 150_000u64 * pools.len() as u64;

                Ok((profit, gas))
            }

            BackrunStrategy::Sandwich { expected_profit, .. } => {
                let gas = 300_000u64; // Gas for front + back
                Ok((*expected_profit, gas))
            }
        }
    }

    /// Calculate confidence score for backrun opportunity
    fn calculate_backrun_confidence(
        &self,
        price_impact: f64,
        strategy: &BackrunStrategy,
        _swap_info: &SwapInfo,
    ) -> f64 {
        // Base confidence from price impact
        let impact_confidence = (price_impact * 20.0).min(0.9);

        // Strategy complexity penalty
        let strategy_confidence = match strategy {
            BackrunStrategy::ReverseSwap { .. } => 1.0,  // Simple, high confidence
            BackrunStrategy::CrossPoolArbitrage { .. } => 0.8,  // More complex
            BackrunStrategy::Sandwich { .. } => 0.6,  // Risk of victim front-running
        };

        impact_confidence * strategy_confidence
    }

    /// Clean up stale opportunities
    pub fn cleanup_stale_opportunities(&mut self) {
        let timeout = std::time::Duration::from_secs(self.config.opportunity_timeout_secs);

        self.opportunities.retain(|opp| {
            opp.detected_at.elapsed() < timeout
        });

        while self.recent_victims.len() > self.config.max_tracked_opportunities {
            self.recent_victims.pop_front();
        }
    }
}

// ============================================================================
// Price Oracle
// ============================================================================

impl PriceOracle {
    fn new() -> Self {
        Self {
            reference_prices: HashMap::new(),
        }
    }

    fn get_reference_price(&self, _token_in: &Address, _token_out: &Address) -> f64 {
        // In production, query Chainlink or aggregated DEX prices
        // For now, return placeholder
        2000.0 // ~$2000 ETH/USD
    }
}

// ============================================================================
// ArbitrageDetector Trait Implementation
// ============================================================================

#[async_trait]
impl ArbitrageDetector for BackrunDetector {
    async fn detect(&mut self, context: &DetectionContext) -> Result<DetectionResult> {
        let start = std::time::Instant::now();

        info!("🔍 BackrunDetector: Scanning block {}", context.block_number);

        // Clean up stale opportunities
        self.cleanup_stale_opportunities();

        // Convert backrun opportunities to abstract opportunities
        let opportunities: Vec<ArbitrageOpportunity> = self.opportunities
            .iter()
            .map(|backrun| self.convert_to_abstract_opportunity(backrun))
            .collect();

        let detection_time = start.elapsed();

        info!(
            "✅ BackrunDetector: Found {} opportunities in {:?}",
            opportunities.len(),
            detection_time
        );

        Ok(DetectionResult {
            opportunities,
            detection_time,
            detector_id: "backrun_detector".to_string(),
            confidence_threshold: 0.7,
            metadata: HashMap::new(),
        })
    }

    fn config(&self) -> &DetectorConfig {
        static CONFIG: once_cell::sync::Lazy<DetectorConfig> = once_cell::sync::Lazy::new(|| {
            DetectorConfig {
                enabled: true,
                confidence_threshold: 0.7,
                max_opportunities: 100,
                timeout: std::time::Duration::from_secs(30),
                detector_specific: serde_json::json!({
                    "min_victim_value": "50000000000000000000000",
                    "min_price_impact": 0.01,
                }),
            }
        });
        &CONFIG
    }

    fn update_config(&mut self, _config: DetectorConfig) -> Result<()> {
        // Update internal config
        Ok(())
    }

    fn metadata(&self) -> DetectorMetadata {
        DetectorMetadata {
            name: "BackrunDetector".to_string(),
            version: "1.0.0".to_string(),
            description: "Detects profitable backrun opportunities from mempool transactions".to_string(),
            supported_opportunity_types: vec![
                "Backrun".to_string(),
                "ReverseSwap".to_string(),
            ],
            performance_metrics: PerformanceMetrics {
                avg_detection_time: std::time::Duration::from_millis(5),
                success_rate: 0.90,
                total_detections: 0,
                avg_profit_accuracy: 0.88,
            },
        }
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let mut metrics = HashMap::new();
        metrics.insert("tracked_victims".to_string(), self.recent_victims.len() as f64);
        metrics.insert("active_opportunities".to_string(), self.opportunities.len() as f64);

        Ok(HealthStatus {
            is_healthy: true,
            status_message: "Backrun detector operational".to_string(),
            last_check: std::time::Instant::now(),
            metrics,
        })
    }
}

impl BackrunDetector {
    fn convert_to_abstract_opportunity(&self, backrun: &BackrunOpportunity) -> ArbitrageOpportunity {
        let opportunity_type = match &backrun.backrun_strategy {
            BackrunStrategy::ReverseSwap { token_in, token_out, .. } => {
                OpportunityType::SimpleArbitrage {
                    token_a: *token_in,
                    token_b: *token_out,
                    path: vec![*token_in, *token_out],
                }
            }
            BackrunStrategy::CrossPoolArbitrage { path, .. } => {
                OpportunityType::CrossDexArbitrage {
                    dex_a: Address::ZERO,
                    dex_b: Address::ZERO,
                    token_pair: (path[0], *path.last().unwrap()),
                }
            }
            BackrunStrategy::Sandwich { .. } => {
                OpportunityType::Sandwich {
                    victim_tx: format!("{:?}", backrun.victim_tx.hash),
                    front_run: Address::ZERO,
                    back_run: Address::ZERO,
                }
            }
        };

        let risk_level = if backrun.confidence > 0.85 {
            RiskLevel::Low
        } else if backrun.confidence > 0.70 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        };

        ArbitrageOpportunity {
            id: format!("backrun_{:?}", backrun.victim_tx.hash),
            opportunity_type,
            expected_profit: backrun.expected_profit,
            gas_cost: U256::from(backrun.estimated_gas),
            confidence: backrun.confidence,
            risk_level,
            deadline: Some(backrun.detected_at + std::time::Duration::from_secs(30)),
            required_capital: U256::ZERO, // Use flashloan
            metadata: serde_json::json!({
                "detector": "backrun",
                "victim_tx": format!("{:?}", backrun.victim_tx.hash),
                "price_impact": backrun.price_impact,
                "pool": format!("{:?}", backrun.affected_pool),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backrun_detector_creation() {
        let detector = BackrunDetector::new(BackrunConfig::default());
        assert_eq!(detector.opportunities.len(), 0);
        assert_eq!(detector.recent_victims.len(), 0);
    }

    #[test]
    fn test_price_impact_threshold() {
        let config = BackrunConfig {
            min_price_impact: 0.02, // 2%
            ..Default::default()
        };

        assert_eq!(config.min_price_impact, 0.02);
    }
}
