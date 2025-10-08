//! Strategy Optimization Module
//!
//! This module contains strategy-level optimizers that work on execution plans.
//! Unlike general optimizers (in optimizers/), these are specifically for MEV strategy optimization.

pub mod z3_strategy_optimizer;

pub use z3_strategy_optimizer::{
    Z3StrategyOptimizer,
    Z3ArbitrageOptimizer, // Legacy alias for backward compatibility
};
