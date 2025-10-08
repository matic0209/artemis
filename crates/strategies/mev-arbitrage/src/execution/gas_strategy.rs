//! Gas Strategy Management
//!
//! This module provides sophisticated gas pricing and optimization strategies for MEV execution.
//!
//! ## Features
//!
//! - **Dynamic Gas Pricing**: Adjust gas prices based on network conditions
//! - **Strategy Selection**: Conservative, Aggressive, or Adaptive modes
//! - **Priority Fee Optimization**: EIP-1559 support with priority fee calculation
//! - **Gas Limit Estimation**: Accurate gas estimation for complex transactions
//! - **Profitability Checks**: Ensure gas costs don't exceed expected profits
//!
//! ## Migration Note
//!
//! This module extracts and enhances gas management functionality from:
//! - `legacy::mev_pipeline::GasStrategy`
//! - `legacy::mev_orchestrator` gas estimation logic

use alloy_primitives::U256;
use serde::{Serialize, Deserialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Gas pricing strategy
///
/// Determines how aggressive or conservative the bot should be with gas prices.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GasStrategy {
    /// Conservative: Use base fee + minimal priority fee
    /// Best for low-competition opportunities
    Conservative,

    /// Aggressive: Use high priority fees to win auctions
    /// Best for high-value, time-sensitive opportunities
    Aggressive,

    /// Adaptive: Dynamically adjust based on network conditions and profit margins
    /// Recommended for most scenarios
    Adaptive {
        /// Base multiplier for priority fee (e.g., 1.1 = 10% above base)
        base_multiplier: f64,
        /// Maximum multiplier to use (safety limit)
        max_multiplier: f64,
    },

    /// Custom: User-defined gas price
    Custom {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
    },
}

impl Default for GasStrategy {
    fn default() -> Self {
        GasStrategy::Adaptive {
            base_multiplier: 1.1,
            max_multiplier: 2.0,
        }
    }
}

impl GasStrategy {
    /// Calculate gas price based on strategy and current network conditions
    pub fn calculate_gas_price(
        &self,
        base_fee: U256,
        network_conditions: &NetworkConditions,
        opportunity_value: U256,
    ) -> GasPrice {
        match self {
            GasStrategy::Conservative => {
                let priority_fee = Self::min_priority_fee();
                GasPrice {
                    max_fee_per_gas: base_fee + priority_fee,
                    max_priority_fee_per_gas: priority_fee,
                }
            }

            GasStrategy::Aggressive => {
                // Use high priority fee (90th percentile + buffer)
                let priority_fee = network_conditions.percentile_90_priority_fee
                    .saturating_mul(U256::from(12))
                    .checked_div(U256::from(10))
                    .unwrap_or(U256::from(2_000_000_000u64)); // 2 gwei fallback

                GasPrice {
                    max_fee_per_gas: base_fee.saturating_mul(U256::from(2)) + priority_fee,
                    max_priority_fee_per_gas: priority_fee,
                }
            }

            GasStrategy::Adaptive { base_multiplier, max_multiplier } => {
                // Calculate priority fee based on network congestion and profit margin
                let base_priority = network_conditions.median_priority_fee;

                // Adjust based on congestion
                let congestion_multiplier = network_conditions.congestion_level();

                // Calculate multiplier (capped at max_multiplier)
                let multiplier = (base_multiplier * congestion_multiplier).min(*max_multiplier);

                let priority_fee = Self::apply_multiplier(base_priority, multiplier);

                // Ensure we don't overpay relative to opportunity value
                let max_reasonable_gas = opportunity_value
                    .checked_div(U256::from(3))
                    .unwrap_or(U256::ZERO); // Max 33% of profit for gas

                let capped_priority = priority_fee.min(max_reasonable_gas);

                GasPrice {
                    max_fee_per_gas: base_fee + capped_priority,
                    max_priority_fee_per_gas: capped_priority,
                }
            }

            GasStrategy::Custom { max_fee_per_gas, max_priority_fee_per_gas } => {
                GasPrice {
                    max_fee_per_gas: *max_fee_per_gas,
                    max_priority_fee_per_gas: *max_priority_fee_per_gas,
                }
            }
        }
    }

    /// Minimum viable priority fee (1 gwei)
    fn min_priority_fee() -> U256 {
        U256::from(1_000_000_000u64)
    }

    /// Apply multiplier to U256 value
    fn apply_multiplier(value: U256, multiplier: f64) -> U256 {
        // Convert to f64, apply multiplier, convert back
        // This is imprecise for very large values but acceptable for gas prices
        let value_f64 = value.to_string().parse::<f64>().unwrap_or(0.0);
        let result = value_f64 * multiplier;
        U256::from(result as u64)
    }
}

/// Gas price result
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasPrice {
    /// Maximum fee per gas (EIP-1559)
    pub max_fee_per_gas: U256,
    /// Maximum priority fee per gas (tip to miner)
    pub max_priority_fee_per_gas: U256,
}

impl GasPrice {
    /// Calculate total gas cost for a transaction
    pub fn total_cost(&self, gas_limit: u64) -> U256 {
        self.max_fee_per_gas * U256::from(gas_limit)
    }

    /// Check if execution is profitable after gas costs
    pub fn is_profitable(&self, expected_profit: U256, gas_limit: u64) -> bool {
        let gas_cost = self.total_cost(gas_limit);
        expected_profit > gas_cost
    }

    /// Calculate net profit after gas costs
    pub fn net_profit(&self, expected_profit: U256, gas_limit: u64) -> U256 {
        let gas_cost = self.total_cost(gas_limit);
        expected_profit.saturating_sub(gas_cost)
    }
}

/// Network conditions for gas price calculation
#[derive(Debug, Clone)]
pub struct NetworkConditions {
    /// Current base fee
    pub base_fee: U256,
    /// Median priority fee from recent blocks
    pub median_priority_fee: U256,
    /// 90th percentile priority fee
    pub percentile_90_priority_fee: U256,
    /// Recent base fee history
    pub base_fee_history: VecDeque<U256>,
    /// Network utilization (0.0 to 1.0)
    pub network_utilization: f64,
    /// Last updated timestamp
    pub last_updated: Instant,
}

impl NetworkConditions {
    /// Create new network conditions
    pub fn new(base_fee: U256) -> Self {
        Self {
            base_fee,
            median_priority_fee: U256::from(1_500_000_000u64), // 1.5 gwei default
            percentile_90_priority_fee: U256::from(2_000_000_000u64), // 2 gwei default
            base_fee_history: VecDeque::new(),
            network_utilization: 0.5, // 50% default
            last_updated: Instant::now(),
        }
    }

    /// Update with new block data
    pub fn update(&mut self, base_fee: U256, gas_used: u64, gas_limit: u64) {
        self.base_fee = base_fee;
        self.base_fee_history.push_back(base_fee);

        // Keep only recent history (last 10 blocks)
        if self.base_fee_history.len() > 10 {
            self.base_fee_history.pop_front();
        }

        // Update utilization
        self.network_utilization = if gas_limit > 0 {
            gas_used as f64 / gas_limit as f64
        } else {
            0.5
        };

        self.last_updated = Instant::now();
    }

    /// Calculate congestion level (1.0 = normal, >1.0 = congested)
    pub fn congestion_level(&self) -> f64 {
        // Base on network utilization and base fee trend
        let utilization_factor = if self.network_utilization > 0.9 {
            1.5 // Highly congested
        } else if self.network_utilization > 0.7 {
            1.2 // Moderately congested
        } else {
            1.0 // Normal
        };

        // Check if base fee is rising
        let trend_factor = self.base_fee_trend();

        utilization_factor * trend_factor
    }

    /// Calculate base fee trend (1.0 = stable, >1.0 = rising, <1.0 = falling)
    fn base_fee_trend(&self) -> f64 {
        if self.base_fee_history.len() < 3 {
            return 1.0;
        }

        let recent: Vec<_> = self.base_fee_history.iter().rev().take(3).collect();
        if recent[0] > recent[2] {
            1.1 // Rising
        } else if recent[0] < recent[2] {
            0.9 // Falling
        } else {
            1.0 // Stable
        }
    }

    /// Check if conditions are stale
    pub fn is_stale(&self) -> bool {
        self.last_updated.elapsed() > Duration::from_secs(30)
    }
}

impl Default for NetworkConditions {
    fn default() -> Self {
        Self::new(U256::from(20_000_000_000u64)) // 20 gwei
    }
}

/// Gas estimator for different transaction types
pub struct GasEstimator;

impl GasEstimator {
    /// Estimate gas for a simple swap
    pub fn estimate_swap_gas(num_hops: usize) -> u64 {
        // Base overhead + per-hop cost
        100_000 + (num_hops as u64 * 50_000)
    }

    /// Estimate gas for flash loan arbitrage
    pub fn estimate_flash_loan_gas(num_swaps: usize) -> u64 {
        // Flash loan overhead + swaps + repayment
        150_000 + (num_swaps as u64 * 50_000) + 50_000
    }

    /// Estimate gas for liquidation
    pub fn estimate_liquidation_gas() -> u64 {
        300_000 // Typically more complex
    }

    /// Estimate gas for sandwich attack
    pub fn estimate_sandwich_gas() -> u64 {
        // Two transactions: front-run + back-run
        250_000
    }

    /// Add safety buffer to estimate
    pub fn with_buffer(estimate: u64, buffer_percent: f64) -> u64 {
        let buffer = (estimate as f64 * buffer_percent) as u64;
        estimate + buffer
    }
}

/// Gas optimizer for multi-step transactions
pub struct GasOptimizer {
    /// Maximum gas price willing to pay
    pub max_gas_price: U256,
    /// Minimum profit threshold after gas
    pub min_net_profit: U256,
}

impl GasOptimizer {
    /// Create new gas optimizer
    pub fn new(max_gas_price: U256, min_net_profit: U256) -> Self {
        Self {
            max_gas_price,
            min_net_profit,
        }
    }

    /// Check if transaction should be executed given gas conditions
    pub fn should_execute(
        &self,
        gas_price: &GasPrice,
        gas_limit: u64,
        expected_profit: U256,
    ) -> bool {
        // Check gas price cap
        if gas_price.max_fee_per_gas > self.max_gas_price {
            return false;
        }

        // Check profitability
        let net_profit = gas_price.net_profit(expected_profit, gas_limit);
        net_profit >= self.min_net_profit
    }

    /// Calculate optimal gas limit for maximum profit
    pub fn optimize_gas_limit(
        &self,
        base_estimate: u64,
        gas_price: &GasPrice,
        expected_profit: U256,
    ) -> u64 {
        // Add buffer for safety
        let with_buffer = GasEstimator::with_buffer(base_estimate, 0.2); // 20% buffer

        // Ensure it's still profitable
        if gas_price.is_profitable(expected_profit, with_buffer) {
            with_buffer
        } else {
            base_estimate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conservative_strategy() {
        let strategy = GasStrategy::Conservative;
        let base_fee = U256::from(20_000_000_000u64); // 20 gwei
        let conditions = NetworkConditions::new(base_fee);
        let opportunity_value = U256::from(1_000_000_000_000_000_000u64); // 1 ETH

        let price = strategy.calculate_gas_price(base_fee, &conditions, opportunity_value);

        // Should be base_fee + minimal priority
        assert!(price.max_priority_fee_per_gas == U256::from(1_000_000_000u64));
    }

    #[test]
    fn test_gas_price_profitability() {
        let gas_price = GasPrice {
            max_fee_per_gas: U256::from(50_000_000_000u64), // 50 gwei
            max_priority_fee_per_gas: U256::from(2_000_000_000u64),
        };

        let expected_profit = U256::from(20_000_000_000_000_000u64); // 0.02 ETH
        let gas_limit = 200_000;

        assert!(gas_price.is_profitable(expected_profit, gas_limit));

        let net = gas_price.net_profit(expected_profit, gas_limit);
        assert!(net > U256::ZERO);
    }

    #[test]
    fn test_network_conditions_congestion() {
        let mut conditions = NetworkConditions::new(U256::from(20_000_000_000u64));

        // Simulate high utilization
        conditions.network_utilization = 0.95;

        let congestion = conditions.congestion_level();
        assert!(congestion > 1.0, "High utilization should increase congestion level");
    }

    #[test]
    fn test_gas_estimator() {
        assert_eq!(GasEstimator::estimate_swap_gas(2), 200_000);
        assert_eq!(GasEstimator::estimate_swap_gas(3), 250_000);

        let with_buffer = GasEstimator::with_buffer(200_000, 0.2);
        assert_eq!(with_buffer, 240_000);
    }

    #[test]
    fn test_gas_optimizer_should_execute() {
        let optimizer = GasOptimizer::new(
            U256::from(100_000_000_000u64), // Max 100 gwei
            U256::from(1_000_000_000_000_000u64), // Min 0.001 ETH profit
        );

        let gas_price = GasPrice {
            max_fee_per_gas: U256::from(50_000_000_000u64),
            max_priority_fee_per_gas: U256::from(2_000_000_000u64),
        };

        let expected_profit = U256::from(20_000_000_000_000_000u64); // 0.02 ETH

        assert!(optimizer.should_execute(&gas_price, 200_000, expected_profit));
    }

    #[test]
    fn test_adaptive_strategy_profit_capping() {
        let strategy = GasStrategy::Adaptive {
            base_multiplier: 2.0,
            max_multiplier: 3.0,
        };

        let base_fee = U256::from(20_000_000_000u64);
        let mut conditions = NetworkConditions::new(base_fee);
        conditions.median_priority_fee = U256::from(10_000_000_000u64); // 10 gwei

        // Small opportunity - should cap gas
        let small_opportunity = U256::from(5_000_000_000_000_000u64); // 0.005 ETH

        let price = strategy.calculate_gas_price(base_fee, &conditions, small_opportunity);

        // Priority fee should be capped to not exceed 33% of opportunity value
        let max_reasonable = small_opportunity.checked_div(U256::from(3)).unwrap();
        assert!(price.max_priority_fee_per_gas <= max_reasonable);
    }
}