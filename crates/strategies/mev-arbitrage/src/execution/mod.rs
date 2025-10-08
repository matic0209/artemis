//! Execution Module
//!
//! This module contains execution-related implementations:
//! - Gas strategy for optimal gas price management
//! - Flashloan integration for zero-capital arbitrage
//! - Transaction builder for signing and submitting transactions

pub mod gas_strategy;
pub mod flashloan;
pub mod transaction_builder;

// Re-export main execution types
pub use gas_strategy::GasStrategy;
pub use flashloan::{
    FlashloanProvider, FlashloanRequest, FlashloanExecutor, FlashloanAddresses,
    execution_plan_to_flashloan, requires_flashloan, estimate_profit_after_fees,
};
pub use transaction_builder::{
    TransactionBuilder, BuiltTransaction, FlashbotsBundle,
    encode_uniswap_v2_swap, encode_uniswap_v3_swap,
};
