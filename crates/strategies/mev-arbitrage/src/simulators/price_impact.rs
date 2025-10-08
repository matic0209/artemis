//! Price Impact Simulator using REVM
//!
//! Simulates liquidations and other transactions to determine their impact on DEX prices.

use alloy_primitives::{Address, U256};
use std::collections::HashMap;
use anyhow::Result;

use crate::detectors::liquidation::{LiquidationOpportunity, PriceImpact, PoolStateChange, PriceChange};
use crate::abstractions::ArbitrageOpportunity;
use crate::utils::TokenGraph;
use crate::simulators::revm_adapter::{RevmSimulator, SimulationTx, PoolReserves, amm_math};

/// Price impact simulator
pub struct PriceImpactSimulator {
    /// REVM simulator for transaction execution
    revm_sim: RevmSimulator,
    /// Pool states cache
    pool_states: HashMap<Address, PoolState>,
}

#[derive(Debug, Clone)]
pub struct PoolState {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
}

impl PriceImpactSimulator {
    pub fn new() -> Self {
        Self::with_block(18_500_000) // Default to recent block
    }

    pub fn with_block(block_number: u64) -> Self {
        Self {
            revm_sim: RevmSimulator::new(block_number),
            pool_states: HashMap::new(),
        }
    }

    /// 模拟清算交易，返回价格影响
    pub async fn simulate_liquidation(
        &mut self,
        liquidation: &LiquidationOpportunity,
    ) -> Result<PriceImpact> {
        // 1. Snapshot current pool states
        let snapshot = self.revm_sim.snapshot();

        // 2. Simulate Aave liquidationCall transaction
        let liquidation_tx = self.build_liquidation_tx(liquidation)?;
        let _liq_result = self.revm_sim.simulate_tx(liquidation_tx).await?;

        // 3. Simulate selling seized collateral on DEX
        // This is where price impact occurs
        let collateral_sale_impact = self
            .simulate_collateral_sale(
                liquidation.position.collateral_token,
                liquidation.collateral_to_seize,
            )
            .await?;

        // 4. Restore snapshot (we're just simulating, not actually executing)
        self.revm_sim.restore(snapshot);

        Ok(collateral_sale_impact)
    }

    /// Build Aave liquidation transaction
    fn build_liquidation_tx(
        &self,
        liquidation: &LiquidationOpportunity,
    ) -> Result<SimulationTx> {
        // Aave liquidationCall function signature:
        // liquidationCall(address collateralAsset, address debtAsset, address user, uint256 debtToCover, bool receiveAToken)

        // Function selector: 0x00a718a9
        let mut data = vec![0x00, 0xa7, 0x18, 0xa9];

        // Add parameters (simplified - would need proper ABI encoding)
        data.extend_from_slice(liquidation.position.collateral_token.as_slice());
        data.extend_from_slice(liquidation.position.debt_token.as_slice());
        data.extend_from_slice(liquidation.position.user.as_slice());
        // debt_to_repay and receiveAToken would follow...

        Ok(SimulationTx {
            from: Address::ZERO, // Bot address
            to: Address::ZERO,   // Aave pool address
            value: U256::ZERO,
            data,
            gas_limit: 500_000,
        })
    }

    /// Simulate selling collateral on DEX
    async fn simulate_collateral_sale(
        &mut self,
        collateral_token: Address,
        amount: U256,
    ) -> Result<PriceImpact> {
        // Find DEX pool for collateral/WETH
        let pool_address = self.find_pool_for_token(collateral_token)?;

        // Get pool reserves before sale
        let pool = self.revm_sim
            .get_pool_reserves(pool_address)
            .cloned()
            .unwrap_or_else(|| PoolReserves {
                token0: collateral_token,
                token1: Address::ZERO, // WETH
                reserve0: U256::from(1000_000_000_000_000_000_000u128),
                reserve1: U256::from(500_000_000_000_000_000_000u128),
                last_update: 0,
            });

        // Calculate output and price impact using AMM math
        let amount_out = amm_math::get_amount_out(amount, pool.reserve0, pool.reserve1);
        let price_impact = amm_math::calculate_price_impact(amount_out, pool.reserve1);

        // Calculate new reserves
        let new_reserve0 = pool.reserve0.saturating_add(amount);
        let new_reserve1 = pool.reserve1.saturating_sub(amount_out);

        // Calculate old and new prices
        let old_price = if pool.reserve0 > U256::ZERO {
            pool.reserve1
                .saturating_mul(U256::from(1_000_000_000_000_000_000u64))
                / pool.reserve0
        } else {
            U256::ZERO
        };

        let new_price = if new_reserve0 > U256::ZERO {
            new_reserve1
                .saturating_mul(U256::from(1_000_000_000_000_000_000u64))
                / new_reserve0
        } else {
            U256::ZERO
        };

        // Build price impact result
        let mut affected_pools = HashMap::new();
        affected_pools.insert(
            pool_address,
            PoolStateChange {
                pool_address,
                token0: pool.token0,
                token1: pool.token1,
                old_reserve0: pool.reserve0,
                old_reserve1: pool.reserve1,
                new_reserve0,
                new_reserve1,
            },
        );

        let mut price_changes = HashMap::new();
        price_changes.insert(
            collateral_token,
            PriceChange {
                token: collateral_token,
                old_price_in_weth: old_price,
                new_price_in_weth: new_price,
                change_percentage: -price_impact, // Negative because price drops when selling
            },
        );

        Ok(PriceImpact {
            affected_pools,
            price_changes,
            gas_used: 150_000, // Typical swap gas
        })
    }

    /// Find DEX pool for a given token
    fn find_pool_for_token(&self, _token: Address) -> Result<Address> {
        // TODO: Query Uniswap V2 Factory or maintain pool registry
        // For now, return mock pool address
        Ok(Address::repeat_byte(0x01))
    }

    /// Estimate price impact without full REVM simulation (simplified)
    /// DEPRECATED: Use simulate_liquidation with REVM instead
    fn estimate_liquidation_impact(
        &self,
        liquidation: &LiquidationOpportunity,
    ) -> Result<PriceImpact> {
        let mut price_changes = HashMap::new();

        // Estimate price impact based on collateral size vs pool liquidity
        // This is a simplified calculation - real implementation would use REVM

        // Assume 1% price impact for every 10% of pool liquidity
        // Real calculation would simulate exact AMM math
        let estimated_change = 0.5; // 0.5% placeholder

        price_changes.insert(
            liquidation.position.collateral_token,
            PriceChange {
                token: liquidation.position.collateral_token,
                old_price_in_weth: U256::from(1_000_000_000_000_000_000u64), // Placeholder
                new_price_in_weth: U256::from(995_000_000_000_000_000u64),   // -0.5%
                change_percentage: -estimated_change,
            },
        );

        Ok(PriceImpact {
            affected_pools: HashMap::new(),
            price_changes,
            gas_used: 500_000,
        })
    }

    /// 在新价格下查找套利机会
    pub fn find_arbitrage_after_impact(
        &self,
        impact: &PriceImpact,
        graph: &TokenGraph,
    ) -> Result<Vec<ArbitrageOpportunity>> {
        let mut opportunities = vec![];

        // For each significant price change
        for (token, price_change) in &impact.price_changes {
            // Only consider changes > 0.5%
            if price_change.change_percentage.abs() < 0.5 {
                continue;
            }

            // Find triangular arbitrage paths involving this token
            let paths = graph.find_triangular_paths(*token);

            for path in paths {
                // Calculate profit with new prices
                // This is simplified - real implementation would:
                // 1. Use new reserve ratios
                // 2. Simulate each swap
                // 3. Calculate exact profit

                let estimated_profit = self.estimate_path_profit(&path, &impact.price_changes)?;

                if estimated_profit > U256::ZERO {
                    use uuid::Uuid;

                    opportunities.push(ArbitrageOpportunity {
                        id: Uuid::new_v4().to_string(),
                        opportunity_type: crate::abstractions::OpportunityType::TriangularArbitrage {
                            tokens: path.clone(),
                            pools: vec![],
                        },
                        expected_profit: estimated_profit,
                        gas_cost: U256::from(200_000u64).saturating_mul(U256::from(20_000_000_000u64)), // 200k gas * 20 gwei
                        confidence: 0.7,
                        risk_level: crate::abstractions::RiskLevel::Medium,
                        deadline: None,
                        required_capital: U256::from(100_000_000_000_000_000u64), // 0.1 ETH
                        metadata: serde_json::json!({
                            "price_impact_induced": true,
                            "path": path.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>(),
                        }),
                    });
                }
            }
        }

        Ok(opportunities)
    }

    /// Estimate profit for an arbitrage path with new prices
    fn estimate_path_profit(
        &self,
        path: &[Address],
        price_changes: &HashMap<Address, PriceChange>,
    ) -> Result<U256> {
        // Simplified profit estimation
        // Real implementation would simulate each swap with AMM math

        // Check if path contains tokens with price changes
        let has_price_change = path
            .iter()
            .any(|token| price_changes.contains_key(token));

        if has_price_change {
            // Assume 0.1 ETH profit for paths affected by price changes
            // Real calculation would be much more precise
            Ok(U256::from(100_000_000_000_000_000u64)) // 0.1 ETH
        } else {
            Ok(U256::ZERO)
        }
    }

    /// Snapshot current pool states
    fn snapshot_pool_states(&self) -> HashMap<Address, PoolState> {
        self.pool_states.clone()
    }

    /// Update pool state
    pub fn update_pool_state(&mut self, pool: PoolState) {
        self.pool_states.insert(pool.address, pool);
    }
}

impl Default for PriceImpactSimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulator_creation() {
        let simulator = PriceImpactSimulator::new();
        assert_eq!(simulator.pool_states.len(), 0);
    }

    #[test]
    fn test_pool_state_update() {
        let mut simulator = PriceImpactSimulator::new();

        let pool = PoolState {
            address: Address::repeat_byte(0x01),
            token0: Address::repeat_byte(0x02),
            token1: Address::repeat_byte(0x03),
            reserve0: U256::from(1000),
            reserve1: U256::from(2000),
        };

        simulator.update_pool_state(pool.clone());
        assert_eq!(simulator.pool_states.len(), 1);
    }
}
