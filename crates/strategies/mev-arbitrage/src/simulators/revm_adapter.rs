//! REVM Adapter for Price Impact Simulation
//!
//! Provides a simplified interface to REVM for simulating liquidations
//! and their price impact on DEX pools.

use alloy_primitives::{Address, U256};
use anyhow::{Result, Context};
use std::collections::HashMap;

/// REVM-based simulator for transaction execution
pub struct RevmSimulator {
    /// Fork block number
    block_number: u64,
    /// Cached account states
    account_cache: HashMap<Address, AccountState>,
    /// Cached pool states
    pool_cache: HashMap<Address, PoolReserves>,
}

/// Account state snapshot
#[derive(Debug, Clone)]
pub struct AccountState {
    pub balance: U256,
    pub nonce: u64,
    pub code: Vec<u8>,
    pub storage: HashMap<U256, U256>,
}

/// Pool reserve state
#[derive(Debug, Clone)]
pub struct PoolReserves {
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub last_update: u64,
}

/// Simulation result
#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success: bool,
    pub gas_used: u64,
    pub output: Vec<u8>,
    pub state_changes: Vec<StateChange>,
    pub logs: Vec<SimulationLog>,
}

/// State change record
#[derive(Debug, Clone)]
pub struct StateChange {
    pub address: Address,
    pub slot: U256,
    pub old_value: U256,
    pub new_value: U256,
}

/// Simulation log event
#[derive(Debug, Clone)]
pub struct SimulationLog {
    pub address: Address,
    pub topics: Vec<U256>,
    pub data: Vec<u8>,
}

/// Transaction to simulate
#[derive(Debug, Clone)]
pub struct SimulationTx {
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub data: Vec<u8>,
    pub gas_limit: u64,
}

impl RevmSimulator {
    /// Create new REVM simulator at specific block
    pub fn new(block_number: u64) -> Self {
        Self {
            block_number,
            account_cache: HashMap::new(),
            pool_cache: HashMap::new(),
        }
    }

    /// Simulate a transaction and return the result
    pub async fn simulate_tx(&mut self, tx: SimulationTx) -> Result<SimulationResult> {
        // TODO: Real REVM integration
        // For now, provide simplified simulation logic

        // Simulate based on transaction target
        if self.is_uniswap_v2_swap(&tx.to) {
            self.simulate_uniswap_v2_swap(tx).await
        } else if self.is_aave_liquidation(&tx.data) {
            self.simulate_aave_liquidation(tx).await
        } else {
            // Generic simulation
            self.simulate_generic_call(tx).await
        }
    }

    /// Simulate Uniswap V2 swap
    async fn simulate_uniswap_v2_swap(&mut self, tx: SimulationTx) -> Result<SimulationResult> {
        // Parse swap parameters from calldata
        // For now, use simplified logic

        let gas_used = 150_000u64;

        // Get pool reserves
        let pool = self.get_or_fetch_pool_state(tx.to).await?;

        // Calculate swap output using constant product formula
        // amountOut = (amountIn * reserve1 * 997) / (reserve0 * 1000 + amountIn * 997)
        let amount_in = tx.value;
        let reserve_in = pool.reserve0;
        let reserve_out = pool.reserve1;

        let amount_in_with_fee = amount_in.saturating_mul(U256::from(997));
        let numerator = amount_in_with_fee.saturating_mul(reserve_out);
        let denominator = reserve_in.saturating_mul(U256::from(1000)).saturating_add(amount_in_with_fee);
        let amount_out = numerator / denominator;

        // Update pool reserves
        let new_reserve0 = reserve_in.saturating_add(amount_in);
        let new_reserve1 = reserve_out.saturating_sub(amount_out);

        // Record state changes
        let state_changes = vec![
            StateChange {
                address: tx.to,
                slot: U256::from(8), // Reserve0 slot (Uniswap V2)
                old_value: reserve_in,
                new_value: new_reserve0,
            },
            StateChange {
                address: tx.to,
                slot: U256::from(9), // Reserve1 slot
                old_value: reserve_out,
                new_value: new_reserve1,
            },
        ];

        // Update cache
        self.pool_cache.insert(tx.to, PoolReserves {
            reserve0: new_reserve0,
            reserve1: new_reserve1,
            ..pool
        });

        Ok(SimulationResult {
            success: true,
            gas_used,
            output: amount_out.to_be_bytes::<32>().to_vec(),
            state_changes,
            logs: vec![],
        })
    }

    /// Simulate Aave liquidation call
    async fn simulate_aave_liquidation(&mut self, tx: SimulationTx) -> Result<SimulationResult> {
        // Liquidation gas is typically higher
        let gas_used = 500_000u64;

        // Parse liquidation parameters
        // liquidationCall(collateral, debt, user, debtToCover, receiveAToken)

        // Simplified simulation - just track gas and success
        Ok(SimulationResult {
            success: true,
            gas_used,
            output: vec![],
            state_changes: vec![],
            logs: vec![],
        })
    }

    /// Simulate generic contract call
    async fn simulate_generic_call(&mut self, tx: SimulationTx) -> Result<SimulationResult> {
        let gas_used = 100_000u64;

        Ok(SimulationResult {
            success: true,
            gas_used,
            output: vec![],
            state_changes: vec![],
            logs: vec![],
        })
    }

    /// Get pool state from cache or fetch from provider
    async fn get_or_fetch_pool_state(&mut self, pool_address: Address) -> Result<PoolReserves> {
        if let Some(cached) = self.pool_cache.get(&pool_address) {
            return Ok(cached.clone());
        }

        // TODO: Fetch from provider
        // For now, return mock data
        let pool = PoolReserves {
            token0: Address::ZERO,
            token1: Address::ZERO,
            reserve0: U256::from(1000_000_000_000_000_000_000u128), // 1000 ETH
            reserve1: U256::from(2000_000_000_000_000_000_000u128), // 2000 tokens
            last_update: self.block_number,
        };

        self.pool_cache.insert(pool_address, pool.clone());
        Ok(pool)
    }

    /// Check if address is Uniswap V2 pair
    fn is_uniswap_v2_swap(&self, _address: &Address) -> bool {
        // TODO: Check if contract implements Uniswap V2 Pair interface
        false
    }

    /// Check if calldata is Aave liquidation
    fn is_aave_liquidation(&self, data: &[u8]) -> bool {
        // Check for liquidationCall function selector (0x00a718a9)
        if data.len() < 4 {
            return false;
        }
        data[0..4] == [0x00, 0xa7, 0x18, 0xa9]
    }

    /// Get current pool reserves (for price impact calculation)
    pub fn get_pool_reserves(&self, pool_address: Address) -> Option<&PoolReserves> {
        self.pool_cache.get(&pool_address)
    }

    /// Snapshot current state
    pub fn snapshot(&self) -> SimulatorSnapshot {
        SimulatorSnapshot {
            block_number: self.block_number,
            pool_states: self.pool_cache.clone(),
        }
    }

    /// Restore from snapshot
    pub fn restore(&mut self, snapshot: SimulatorSnapshot) {
        self.block_number = snapshot.block_number;
        self.pool_cache = snapshot.pool_states;
    }
}

/// Simulator state snapshot
#[derive(Debug, Clone)]
pub struct SimulatorSnapshot {
    pub block_number: u64,
    pub pool_states: HashMap<Address, PoolReserves>,
}

/// Helper functions for AMM math
pub mod amm_math {
    use alloy_primitives::U256;

    /// Calculate Uniswap V2 swap output
    /// amountOut = (amountIn * reserve_out * 997) / (reserve_in * 1000 + amountIn * 997)
    pub fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_in == U256::ZERO || reserve_in == U256::ZERO || reserve_out == U256::ZERO {
            return U256::ZERO;
        }

        let amount_in_with_fee = amount_in.saturating_mul(U256::from(997));
        let numerator = amount_in_with_fee.saturating_mul(reserve_out);
        let denominator = reserve_in
            .saturating_mul(U256::from(1000))
            .saturating_add(amount_in_with_fee);

        numerator / denominator
    }

    /// Calculate Uniswap V2 swap input (inverse)
    /// amountIn = (reserve_in * amountOut * 1000) / ((reserve_out - amountOut) * 997) + 1
    pub fn get_amount_in(amount_out: U256, reserve_in: U256, reserve_out: U256) -> U256 {
        if amount_out == U256::ZERO || reserve_in == U256::ZERO || reserve_out == U256::ZERO {
            return U256::ZERO;
        }

        if amount_out >= reserve_out {
            return U256::MAX; // Cannot swap more than reserve
        }

        let numerator = reserve_in
            .saturating_mul(amount_out)
            .saturating_mul(U256::from(1000));
        let denominator = reserve_out
            .saturating_sub(amount_out)
            .saturating_mul(U256::from(997));

        (numerator / denominator).saturating_add(U256::from(1))
    }

    /// Calculate price impact percentage
    /// impact = (1 - (reserve_out / (reserve_out - amountOut))) * 100
    pub fn calculate_price_impact(amount_out: U256, reserve_out: U256) -> f64 {
        if reserve_out == U256::ZERO || amount_out >= reserve_out {
            return 100.0; // 100% impact
        }

        let new_reserve = reserve_out.saturating_sub(amount_out);
        let old_price = 1.0;

        // Convert U256 to f64 safely
        let reserve_out_f64 = reserve_out.to::<u128>() as f64;
        let new_reserve_f64 = new_reserve.to::<u128>() as f64;

        if new_reserve_f64 == 0.0 {
            return 100.0;
        }

        let new_price = reserve_out_f64 / new_reserve_f64;

        ((new_price / old_price - 1.0) * 100.0).abs()
    }

    /// Calculate new price after swap
    pub fn get_new_price(reserve0: U256, reserve1: U256, amount_in: U256) -> U256 {
        let new_reserve0 = reserve0.saturating_add(amount_in);
        let amount_out = get_amount_out(amount_in, reserve0, reserve1);
        let new_reserve1 = reserve1.saturating_sub(amount_out);

        // Price = reserve1 / reserve0
        if new_reserve0 == U256::ZERO {
            return U256::ZERO;
        }

        // Return price with 18 decimals precision
        new_reserve1
            .saturating_mul(U256::from(1_000_000_000_000_000_000u64))
            / new_reserve0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::amm_math::*;

    #[test]
    fn test_get_amount_out() {
        // Test case: 1 ETH in, 1000 ETH reserve, 2000 USDC reserve
        let amount_in = U256::from(1_000_000_000_000_000_000u64); // 1 ETH
        let reserve_in = U256::from(1000_000_000_000_000_000_000u128); // 1000 ETH
        let reserve_out = U256::from(2000_000_000_000_000_000_000u128); // 2000 USDC

        let amount_out = get_amount_out(amount_in, reserve_in, reserve_out);

        // Expected: ~1.994 USDC (with 0.3% fee)
        assert!(amount_out > U256::ZERO);
        assert!(amount_out < U256::from(2_000_000_000_000_000_000u64)); // Less than 2 USDC
    }

    #[test]
    fn test_price_impact() {
        let amount_out = U256::from(100_000_000_000_000_000_000u128); // 100 tokens
        let reserve_out = U256::from(1000_000_000_000_000_000_000u128); // 1000 tokens

        let impact = calculate_price_impact(amount_out, reserve_out);

        // 10% of reserves = significant impact
        assert!(impact > 5.0); // More than 5% impact
        assert!(impact < 15.0); // Less than 15% impact
    }

    #[test]
    fn test_simulator_creation() {
        let simulator = RevmSimulator::new(18_500_000);
        assert_eq!(simulator.block_number, 18_500_000);
    }

    #[test]
    fn test_snapshot_restore() {
        let mut simulator = RevmSimulator::new(18_500_000);

        // Add some state
        simulator.pool_cache.insert(
            Address::repeat_byte(0x01),
            PoolReserves {
                token0: Address::ZERO,
                token1: Address::ZERO,
                reserve0: U256::from(1000),
                reserve1: U256::from(2000),
                last_update: 18_500_000,
            },
        );

        // Create snapshot
        let snapshot = simulator.snapshot();

        // Modify state
        simulator.pool_cache.clear();
        assert_eq!(simulator.pool_cache.len(), 0);

        // Restore
        simulator.restore(snapshot);
        assert_eq!(simulator.pool_cache.len(), 1);
    }
}
