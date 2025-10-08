//! Multi-Strategy Composer
//!
//! Combines liquidations with arbitrage opportunities to create
//! higher-value atomic bundles (inspired by 0x0e49's approach).

use alloy_primitives::U256;
use anyhow::Result;

use crate::abstractions::{ArbitrageOpportunity, DetectionContext};
use crate::detectors::liquidation::{LiquidationDetector, LiquidationOpportunity};
use crate::detectors::fast::FastArbitrageDetector;
use crate::simulators::PriceImpactSimulator;
use crate::utils::TokenGraph;

/// Composed opportunity combining liquidation + induced arbitrages
#[derive(Debug, Clone)]
pub struct ComposedOpportunity {
    /// Liquidation component
    pub liquidation: LiquidationOpportunity,

    /// Arbitrage opportunities induced by the liquidation
    pub induced_arbitrages: Vec<ArbitrageOpportunity>,

    /// Total expected profit
    pub total_profit: U256,

    /// Total gas cost
    pub total_gas: u64,

    /// Execution order
    pub execution_order: Vec<ExecutionStep>,
}

/// Execution step in a composed opportunity
#[derive(Debug, Clone)]
pub enum ExecutionStep {
    Liquidation {
        index: usize,
        opportunity: LiquidationOpportunity,
    },
    Arbitrage {
        index: usize,
        opportunity: ArbitrageOpportunity,
        depends_on: Vec<usize>, // Indices of steps this depends on
    },
}

/// Multi-strategy composer configuration
#[derive(Debug, Clone)]
pub struct MultiStrategyComposerConfig {
    /// Minimum total profit to compose (in wei)
    pub min_total_profit: U256,

    /// Minimum arbitrage profit to include (in wei)
    pub min_arbitrage_profit: U256,

    /// Maximum number of arbitrage steps per liquidation
    pub max_arbitrages_per_liquidation: usize,
}

impl Default for MultiStrategyComposerConfig {
    fn default() -> Self {
        Self {
            min_total_profit: U256::from(50_000_000_000_000_000u64), // 0.05 ETH
            min_arbitrage_profit: U256::from(10_000_000_000_000_000u64), // 0.01 ETH
            max_arbitrages_per_liquidation: 5,
        }
    }
}

/// Multi-strategy composer
pub struct MultiStrategyComposer {
    liquidation_detector: LiquidationDetector,
    price_simulator: PriceImpactSimulator,
    config: MultiStrategyComposerConfig,
}

impl MultiStrategyComposer {
    pub fn new(
        liquidation_detector: LiquidationDetector,
        config: MultiStrategyComposerConfig,
    ) -> Self {
        Self {
            liquidation_detector,
            price_simulator: PriceImpactSimulator::new(),
            config,
        }
    }

    /// Compose opportunities by combining liquidations with induced arbitrages
    pub async fn compose_opportunities<P>(
        &mut self,
        provider: &P,
        context: &DetectionContext,
        token_graph: &TokenGraph,
    ) -> Result<Vec<ComposedOpportunity>>
    where
        P: alloy_provider::Provider + Clone,
    {
        // 1. Detect liquidation opportunities
        let liquidations = self
            .liquidation_detector
            .detect_underwater_positions(provider, context)
            .await?;

        let mut composed = vec![];

        // 2. For each liquidation, simulate price impact and find induced arbitrages
        for liq in liquidations {
            // Simulate the liquidation's price impact
            let impact = self.price_simulator.simulate_liquidation(&liq).await?;

            // Find arbitrage opportunities created by the price impact
            let arbs = self
                .price_simulator
                .find_arbitrage_after_impact(&impact, token_graph)?;

            // Filter arbitrages by minimum profit
            let profitable_arbs: Vec<_> = arbs
                .into_iter()
                .filter(|arb| arb.expected_profit >= self.config.min_arbitrage_profit)
                .take(self.config.max_arbitrages_per_liquidation)
                .collect();

            if profitable_arbs.is_empty() {
                // No induced arbitrage, skip this liquidation for composition
                // (could still be executed standalone)
                continue;
            }

            // 3. Calculate total profit
            let liquidation_profit = liq.liquidation_reward;
            let arbitrage_profit: U256 = profitable_arbs
                .iter()
                .map(|arb| arb.expected_profit)
                .fold(U256::ZERO, |acc, p| acc.saturating_add(p));
            let total_profit = liquidation_profit.saturating_add(arbitrage_profit);

            // Check if total profit meets minimum threshold
            if total_profit < self.config.min_total_profit {
                continue;
            }

            // 4. Build execution order
            let execution_order = self.build_execution_order(&liq, &profitable_arbs);

            // 5. Calculate total gas
            // Estimate 200k gas per arbitrage (ArbitrageOpportunity doesn't have gas_estimate field)
            let total_gas = impact.gas_used
                + (profitable_arbs.len() as u64 * 200_000);

            composed.push(ComposedOpportunity {
                liquidation: liq,
                induced_arbitrages: profitable_arbs,
                total_profit,
                total_gas,
                execution_order,
            });
        }

        // 6. Sort by total profit (descending)
        composed.sort_by(|a, b| b.total_profit.cmp(&a.total_profit));

        Ok(composed)
    }

    /// Build execution order for a composed opportunity
    fn build_execution_order(
        &self,
        liq: &LiquidationOpportunity,
        arbs: &[ArbitrageOpportunity],
    ) -> Vec<ExecutionStep> {
        let mut steps = vec![];

        // Step 0: Liquidation (must execute first)
        steps.push(ExecutionStep::Liquidation {
            index: 0,
            opportunity: liq.clone(),
        });

        // Steps 1+: Arbitrages (depend on liquidation)
        for (i, arb) in arbs.iter().enumerate() {
            steps.push(ExecutionStep::Arbitrage {
                index: i + 1,
                opportunity: arb.clone(),
                depends_on: vec![0], // Depends on liquidation step
            });
        }

        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detectors::liquidation::{LiquidationDetectorConfig, LiquidationProtocol, AavePosition};
    use alloy_primitives::Address;

    #[test]
    fn test_composer_creation() {
        let liq_detector = LiquidationDetector::new(LiquidationDetectorConfig::default());
        let config = MultiStrategyComposerConfig::default();
        let composer = MultiStrategyComposer::new(liq_detector, config);

        assert_eq!(composer.config.max_arbitrages_per_liquidation, 5);
    }

    #[test]
    fn test_execution_order_building() {
        let liq_detector = LiquidationDetector::new(LiquidationDetectorConfig::default());
        let config = MultiStrategyComposerConfig::default();
        let composer = MultiStrategyComposer::new(liq_detector, config);

        let liq = LiquidationOpportunity {
            protocol: LiquidationProtocol::AaveV2,
            position: AavePosition {
                user: Address::ZERO,
                collateral_token: Address::ZERO,
                collateral_amount: U256::ZERO,
                debt_token: Address::ZERO,
                debt_amount: U256::ZERO,
                health_factor: 0.9,
            },
            liquidation_reward: U256::from(1_000_000_000_000_000_000u64),
            collateral_to_seize: U256::from(1_100_000_000_000_000_000u64),
            debt_to_repay: U256::from(1_000_000_000_000_000_000u64),
            max_close_factor: 0.5,
        };

        let arbs = vec![];
        let steps = composer.build_execution_order(&liq, &arbs);

        assert_eq!(steps.len(), 1); // Just liquidation
        match &steps[0] {
            ExecutionStep::Liquidation { index, .. } => {
                assert_eq!(*index, 0);
            }
            _ => panic!("Expected liquidation step"),
        }
    }
}
