//! Arbitrage Detectors Module
//!
//! This module contains all arbitrage detection implementations:
//! - Fast detector for 2-3 hop simple arbitrage
//! - Enhanced detector for complex multi-hop arbitrage
//! - Symbolic detector for Z3-based detection with multiple strategies
//! - Liquidation detector for Aave V2/V3 and other lending protocols
//! - Backrun detector for profitable mempool backrunning opportunities

pub mod fast;
pub mod enhanced;
pub mod symbolic;
pub mod z3_cache;
pub mod liquidation;
pub mod aave_contracts;
pub mod backrun;

// Re-export main detector types
pub use fast::FastArbitrageDetector;
pub use enhanced::EnhancedArbitrageDetector;
pub use symbolic::SymbolicDetector;
pub use z3_cache::Z3Cache;
pub use liquidation::{LiquidationDetector, LiquidationDetectorConfig, LiquidationOpportunity, LiquidationProtocol};
pub use aave_contracts::{AaveAccountData, IAaveV2Pool, IAaveV3Pool};
pub use backrun::{BackrunDetector, BackrunConfig, BackrunOpportunity, BackrunStrategy};
