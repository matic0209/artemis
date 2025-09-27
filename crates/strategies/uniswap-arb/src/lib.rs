//! MEV-Share Uniswap arbitrage strategy placeholder build.

#[cfg(feature = "full")]
pub mod alloy_impl;
#[cfg(feature = "full")]
pub mod strategy;
#[cfg(feature = "full")]
pub mod strategy_impl;
#[cfg(feature = "full")]
pub mod types;
#[cfg(feature = "full")]
pub mod config;
#[cfg(feature = "full")]
pub mod price_oracle;
#[cfg(feature = "full")]
mod tests;

#[cfg(feature = "full")]
pub use strategy::{MevShareUniArb, ArbConfig, ArbStats};
#[cfg(feature = "full")]
pub use alloy_impl::V2PoolInfo;
#[cfg(feature = "full")]
pub use config::{StrategyConfig, ConfigLoader};

#[cfg(not(feature = "full"))]
mod stub;
#[cfg(not(feature = "full"))]
pub use stub::*;
