//! A strategy implementing probabilistic uniswap v3 / v2 arbitrage on MEV share. At a
//! a high level, we listen to the stream of mev share events, and filter for trades
//! that touch a v3 pool that we have a v2 pool for. We then submit a series of backruns
//! of varying sizes, hoping that one of them will be profitable.

/// Alloy-based strategy implementation.
pub mod alloy_impl;

/// This module contains the core strategy implementation.
pub mod strategy;

/// Strategy implementation details.
pub mod strategy_impl;

/// This module contains the core type definitions for the strategy.
pub mod types;

/// Configuration management for the strategy.
pub mod config;

/// Price oracle for real market data.
pub mod price_oracle;

/// Strategy tests.
#[cfg(test)]
mod tests;

// Re-export main strategy components
pub use strategy::{MevShareUniArb, ArbConfig, ArbStats};
pub use alloy_impl::V2PoolInfo;
pub use config::{StrategyConfig, ConfigLoader};
